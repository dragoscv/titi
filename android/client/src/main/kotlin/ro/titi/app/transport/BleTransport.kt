package ro.titi.app.transport

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothGattServer
import android.bluetooth.BluetoothGattServerCallback
import android.bluetooth.BluetoothGattService
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.BluetoothServerSocket
import android.bluetooth.BluetoothSocket
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.ParcelUuid
import android.util.Log
import androidx.core.content.ContextCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import uniffi.titi_ffi.FfiLinkClass
import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.IOException
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap

/**
 * BLE transport (ADR-0002): GATT is used only for discovery + exchanging the
 * L2CAP PSM; voice and control frames flow over an insecure L2CAP CoC socket
 * (API 29+), which is the only cross-platform radio that carries Opus without
 * Wi-Fi. Frames are 2-byte length prefixed. Token = remote MAC.
 *
 * Glare avoidance: the side with the lexicographically smaller node id
 * initiates the connection; the other side only listens. Peers are keyed by
 * the remote NODE id (from the advert / a hello over the socket), never by
 * MAC: Android rotates the resolvable private address every few minutes, so a
 * MAC-keyed table would see the same phone as a new peer each rotation.
 *
 * GATT data channel (DK-09): Windows has no L2CAP CoC API, so a desktop
 * connects as a GATT central and uses [CH_RX] (write, central → us) and
 * [CH_TX] (notify, us → central). Both carry the socket's stream framing —
 * u16 BE length + frame — split into ATT-sized chunks; the central's first
 * frame is its 8-byte node id. Such peers join this same link.
 */
@SuppressLint("MissingPermission")
class BleTransport(
    private val ctx: Context,
    private val nodeId: () -> ByteArray,
) : Transport {
    override val linkId: UInt = LinkIds.BLE_L2CAP
    override val linkClass: FfiLinkClass = FfiLinkClass.BLE_L2CAP
    override val mtu: UInt? = 1000u

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var events: TransportEvents? = null
    private val bt: BluetoothManager? = ctx.getSystemService(BluetoothManager::class.java)
    private val adapter: BluetoothAdapter? get() = bt?.adapter

    private var server: BluetoothServerSocket? = null
    private var gattServer: BluetoothGattServer? = null
    private var psm: Int = 0
    private val peers = ConcurrentHashMap<String, Peer>() // token (node hex) → peer
    private val gattPeers = ConcurrentHashMap<String, GattPeer>() // central MAC → peer
    private val gattRx = ConcurrentHashMap<String, java.io.ByteArrayOutputStream>() // central MAC → partial stream
    private val gattMtu = ConcurrentHashMap<String, Int>() // central MAC → ATT MTU
    private val seen = ConcurrentHashMap<String, Long>() // node hex → last advert
    private val connecting = ConcurrentHashMap.newKeySet<String>() // node hex
    private var up = false

    private interface Peer {
        val token: String
        fun enqueue(b: ByteArray)
        fun close()
    }

    private inner class L2capPeer(override val token: String, val sock: BluetoothSocket) : Peer {
        val out = DataOutputStream(sock.outputStream.buffered(4096))
        val inp = DataInputStream(sock.inputStream.buffered(4096))
        /** One writer per peer: preserves frame order; a slow link drops stale frames. */
        private val q = kotlinx.coroutines.channels.Channel<ByteArray>(32, kotlinx.coroutines.channels.BufferOverflow.DROP_OLDEST)
        private val writer = scope.launch {
            try { for (b in q) write(b) } catch (_: IOException) { close() }
        }
        override fun enqueue(b: ByteArray) { q.trySend(b) }
        @Synchronized fun write(b: ByteArray) {
            out.writeShort(b.size); out.write(b); out.flush()
        }
        override fun close() { q.close(); writer.cancel(); runCatching { sock.close() } }
    }

    /** A GATT central (Windows desktop); frames leave as TX notifications, one in flight at a time. */
    private inner class GattPeer(override val token: String, val dev: BluetoothDevice) : Peer {
        private val q = kotlinx.coroutines.channels.Channel<ByteArray>(32, kotlinx.coroutines.channels.BufferOverflow.DROP_OLDEST)
        val sent = kotlinx.coroutines.channels.Channel<Unit>(kotlinx.coroutines.channels.Channel.CONFLATED)
        private val writer = scope.launch {
            for (b in q) {
                val framed = ByteArray(2 + b.size)
                framed[0] = (b.size shr 8).toByte(); framed[1] = b.size.toByte()
                b.copyInto(framed, 2)
                var off = 0
                var busy = 0
                while (off < framed.size && busy < 50) {
                    val n = minOf((gattMtu[dev.address] ?: 23) - 3, framed.size - off)
                    if (!notify(dev, framed.copyOfRange(off, off + n))) { busy++; delay(4); continue }
                    kotlinx.coroutines.withTimeoutOrNull(1_000) { sent.receive() } ?: break
                    off += n
                }
            }
        }
        override fun enqueue(b: ByteArray) { q.trySend(b) }
        override fun close() { q.close(); writer.cancel() }
    }

    fun hasPermissions(): Boolean {
        val need = if (Build.VERSION.SDK_INT >= 31) listOf(Manifest.permission.BLUETOOTH_SCAN, Manifest.permission.BLUETOOTH_ADVERTISE, Manifest.permission.BLUETOOTH_CONNECT)
        else listOf(Manifest.permission.ACCESS_FINE_LOCATION)
        return need.all { ContextCompat.checkSelfPermission(ctx, it) == PackageManager.PERMISSION_GRANTED }
    }

    override fun start(events: TransportEvents) {
        this.events = events
        val a = adapter ?: run { Log.i(TAG, "no adapter"); return }
        if (!a.isEnabled || !hasPermissions() || Build.VERSION.SDK_INT < 29) {
            Log.i(TAG, "BLE unavailable (enabled=${a.isEnabled}, perms=${hasPermissions()})")
            return
        }
        try {
            server = a.listenUsingInsecureL2capChannel()
            psm = server!!.psm
            Log.i(TAG, "l2cap listening psm=$psm")
        } catch (e: IOException) {
            Log.w(TAG, "l2cap listen failed", e); return
        }
        startGattServer()
        startAdvertising()
        startScan()
        up = true
        events.linkUp(this)
        scope.launch { acceptLoop() }
        scope.launch { housekeeping() }
    }

    override fun stop() {
        runCatching { adapter?.bluetoothLeScanner?.stopScan(scanCb) }
        runCatching { adapter?.bluetoothLeAdvertiser?.stopAdvertising(advCb) }
        runCatching { gattServer?.close() }
        runCatching { server?.close() }
        peers.values.forEach { it.close() }
        peers.clear()
        gattPeers.clear()
        if (up) { up = false; events?.linkDown(this) }
        scope.cancel()
    }

    // ---- GATT: expose PSM + node id, data channel for centrals -------------

    private fun startGattServer() {
        val gs = bt?.openGattServer(ctx, object : BluetoothGattServerCallback() {
            override fun onCharacteristicReadRequest(device: BluetoothDevice, requestId: Int, offset: Int, ch: BluetoothGattCharacteristic) {
                val value = when (ch.uuid) {
                    CH_PSM -> byteArrayOf((psm shr 8).toByte(), psm.toByte())
                    CH_NODE -> nodeId()
                    else -> ByteArray(0)
                }
                gattServer?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, 0, value)
            }
            override fun onCharacteristicWriteRequest(device: BluetoothDevice, requestId: Int, ch: BluetoothGattCharacteristic, preparedWrite: Boolean, responseNeeded: Boolean, offset: Int, value: ByteArray?) {
                if (ch.uuid == CH_RX && value != null) onGattData(device, value)
                if (responseNeeded) gattServer?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, 0, null)
            }
            override fun onDescriptorWriteRequest(device: BluetoothDevice, requestId: Int, d: BluetoothGattDescriptor, preparedWrite: Boolean, responseNeeded: Boolean, offset: Int, value: ByteArray?) {
                if (responseNeeded) gattServer?.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, 0, null)
            }
            override fun onMtuChanged(device: BluetoothDevice, mtu: Int) { gattMtu[device.address] = mtu }
            override fun onNotificationSent(device: BluetoothDevice, status: Int) { gattPeers[device.address]?.sent?.trySend(Unit) }
            override fun onConnectionStateChange(device: BluetoothDevice, status: Int, newState: Int) {
                if (newState != BluetoothProfile.STATE_DISCONNECTED) return
                gattRx.remove(device.address); gattMtu.remove(device.address)
                val p = gattPeers.remove(device.address) ?: return
                p.close()
                Log.i(TAG, "gatt central gone ${p.token}")
                if (peers.remove(p.token, p)) events?.peerLost(this@BleTransport, p.token)
            }
        }) ?: return
        val svc = BluetoothGattService(SERVICE_UUID, BluetoothGattService.SERVICE_TYPE_PRIMARY)
        svc.addCharacteristic(BluetoothGattCharacteristic(CH_PSM, BluetoothGattCharacteristic.PROPERTY_READ, BluetoothGattCharacteristic.PERMISSION_READ))
        svc.addCharacteristic(BluetoothGattCharacteristic(CH_NODE, BluetoothGattCharacteristic.PROPERTY_READ, BluetoothGattCharacteristic.PERMISSION_READ))
        svc.addCharacteristic(BluetoothGattCharacteristic(CH_RX, BluetoothGattCharacteristic.PROPERTY_WRITE or BluetoothGattCharacteristic.PROPERTY_WRITE_NO_RESPONSE, BluetoothGattCharacteristic.PERMISSION_WRITE))
        val tx = BluetoothGattCharacteristic(CH_TX, BluetoothGattCharacteristic.PROPERTY_NOTIFY, BluetoothGattCharacteristic.PERMISSION_READ)
        tx.addDescriptor(BluetoothGattDescriptor(CCCD, BluetoothGattDescriptor.PERMISSION_READ or BluetoothGattDescriptor.PERMISSION_WRITE))
        svc.addCharacteristic(tx)
        gs.addService(svc)
        gattServer = gs
    }

    /** Reassembles the central's length-prefixed stream; its first frame is the 8-byte node id. */
    private fun onGattData(dev: BluetoothDevice, chunk: ByteArray) {
        val mac = dev.address
        val buf = gattRx.getOrPut(mac) { java.io.ByteArrayOutputStream() }
        val frames = ArrayList<ByteArray>()
        synchronized(buf) {
            buf.write(chunk)
            var b = buf.toByteArray()
            while (b.size >= 2) {
                val len = ((b[0].toInt() and 0xFF) shl 8) or (b[1].toInt() and 0xFF)
                if (len == 0 || len > 4096) { b = ByteArray(0); break }
                if (b.size < 2 + len) break
                frames += b.copyOfRange(2, 2 + len)
                b = b.copyOfRange(2 + len, b.size)
            }
            buf.reset(); buf.write(b)
        }
        for (f in frames) {
            val p = gattPeers[mac]
            if (p == null) {
                if (f.size != 8) continue
                val np = GattPeer(f.toHex(), dev)
                gattPeers[mac] = np
                peers.put(np.token, np)?.close()
                Log.i(TAG, "gatt central attached ${np.token} (${peers.size})")
                events?.peerSeen(this, np.token)
            } else {
                events?.frame(this, p.token, f)
            }
        }
    }

    @Suppress("DEPRECATION")
    private fun notify(dev: BluetoothDevice, value: ByteArray): Boolean {
        val gs = gattServer ?: return false
        val ch = gs.getService(SERVICE_UUID)?.getCharacteristic(CH_TX) ?: return false
        return if (Build.VERSION.SDK_INT >= 33) {
            gs.notifyCharacteristicChanged(dev, ch, false, value) == android.bluetooth.BluetoothStatusCodes.SUCCESS
        } else {
            ch.value = value
            gs.notifyCharacteristicChanged(dev, ch, false)
        }
    }

    private val advCb = object : AdvertiseCallback() {
        override fun onStartFailure(errorCode: Int) { Log.w(TAG, "advertise failed $errorCode") }
    }

    private fun startAdvertising() {
        val adv = adapter?.bluetoothLeAdvertiser ?: return
        val settings = AdvertiseSettings.Builder()
            .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY)
            .setTxPowerLevel(AdvertiseSettings.ADVERTISE_TX_POWER_HIGH)
            .setConnectable(true)
            .build()
        // 128-bit UUID (18 B) + flags (3 B) already fill the 31 B legacy payload;
        // the node id goes in the scan response (ADVERTISE_FAILED_DATA_TOO_LARGE otherwise).
        val data = AdvertiseData.Builder()
            .addServiceUuid(ParcelUuid(SERVICE_UUID))
            .setIncludeDeviceName(false)
            .setIncludeTxPowerLevel(false)
            .build()
        val scanResp = AdvertiseData.Builder()
            .addServiceData(ParcelUuid(SERVICE_UUID), nodeId())
            .build()
        adv.startAdvertising(settings, data, scanResp, advCb)
    }

    // ---- scan + connect ----------------------------------------------------

    private val scanCb = object : ScanCallback() {
        override fun onScanResult(callbackType: Int, r: ScanResult) {
            val mac = r.device.address
            val remoteNode = r.scanRecord?.getServiceData(ParcelUuid(SERVICE_UUID))
            if (remoteNode == null || remoteNode.size != 8) return // scan response not received yet
            val token = remoteNode.toHex()
            seen[token] = System.currentTimeMillis()
            if (peers.containsKey(token)) return
            // glare avoidance: smaller node id initiates
            if (compare(nodeId(), remoteNode) >= 0) return
            if (!connecting.add(token)) return
            Log.d(TAG, "scan hit $mac node=$token rssi=${r.rssi}")
            scope.launch { connectTo(r.device, token) }
        }
        override fun onScanFailed(errorCode: Int) { Log.w(TAG, "scan failed $errorCode") }
    }

    private fun startScan() {
        val scanner = adapter?.bluetoothLeScanner ?: return
        val filter = ScanFilter.Builder().setServiceUuid(ParcelUuid(SERVICE_UUID)).build()
        val settings = ScanSettings.Builder()
            .setScanMode(ScanSettings.SCAN_MODE_BALANCED)
            .setReportDelay(0)
            .build()
        scanner.startScan(listOf(filter), settings, scanCb)
    }

    private suspend fun connectTo(dev: BluetoothDevice, token: String) {
        val mac = dev.address
        try {
            val remotePsm = readPsm(dev) ?: run { Log.w(TAG, "no psm from $mac"); return }
            Log.i(TAG, "connecting l2cap $mac psm=$remotePsm")
            val sock = dev.createInsecureL2capChannel(remotePsm)
            sock.connect()
            Log.i(TAG, "l2cap connected $mac")
            // tell the acceptor who we are: 8-byte node id preamble
            sock.outputStream.write(nodeId()); sock.outputStream.flush()
            attach(token, sock)
        } catch (e: Exception) {
            Log.w(TAG, "connect $mac: ${e.message}")
        } finally {
            connecting.remove(token)
        }
    }

    /** Connect GATT, read the PSM characteristic, disconnect. */
    private suspend fun readPsm(dev: BluetoothDevice): Int? {
        val result = kotlinx.coroutines.CompletableDeferred<Int?>()
        val gatt = dev.connectGatt(ctx, false, object : BluetoothGattCallback() {
            override fun onConnectionStateChange(g: BluetoothGatt, status: Int, newState: Int) {
                if (newState == BluetoothProfile.STATE_CONNECTED) g.discoverServices()
                else if (newState == BluetoothProfile.STATE_DISCONNECTED) { result.complete(null); g.close() }
            }
            override fun onServicesDiscovered(g: BluetoothGatt, status: Int) {
                val ch = g.getService(SERVICE_UUID)?.getCharacteristic(CH_PSM)
                if (ch == null) { result.complete(null); g.disconnect(); return }
                g.readCharacteristic(ch)
            }
            @Deprecated("pre-33 callback")
            override fun onCharacteristicRead(g: BluetoothGatt, ch: BluetoothGattCharacteristic, status: Int) {
                val v = ch.value
                result.complete(if (status == BluetoothGatt.GATT_SUCCESS && v != null && v.size == 2) ((v[0].toInt() and 0xFF) shl 8) or (v[1].toInt() and 0xFF) else null)
                g.disconnect()
            }
            override fun onCharacteristicRead(g: BluetoothGatt, ch: BluetoothGattCharacteristic, v: ByteArray, status: Int) {
                result.complete(if (status == BluetoothGatt.GATT_SUCCESS && v.size == 2) ((v[0].toInt() and 0xFF) shl 8) or (v[1].toInt() and 0xFF) else null)
                g.disconnect()
            }
        }, BluetoothDevice.TRANSPORT_LE)
        return try {
            kotlinx.coroutines.withTimeoutOrNull(8_000) { result.await() }
        } finally {
            runCatching { gatt.close() }
        }
    }

    private suspend fun acceptLoop() {
        val s = server ?: return
        while (scope.isActive) {
            val sock = try { s.accept() } catch (e: IOException) { break }
            scope.launch {
                try {
                    val id = ByteArray(8)
                    DataInputStream(sock.inputStream).readFully(id)
                    attach(id.toHex(), sock)
                } catch (e: IOException) {
                    runCatching { sock.close() }
                }
            }
        }
    }

    private fun attach(token: String, sock: BluetoothSocket) {
        val p = L2capPeer(token, sock)
        peers.put(token, p)?.close()
        Log.i(TAG, "peer attached $token (${peers.size})")
        events?.peerSeen(this, token)
        scope.launch { readLoop(p) }
    }

    private suspend fun readLoop(p: L2capPeer) {
        try {
            while (scope.isActive) {
                val len = p.inp.readUnsignedShort()
                if (len == 0 || len > 4096) break
                val buf = ByteArray(len)
                p.inp.readFully(buf)
                events?.frame(this, p.token, buf)
            }
        } catch (_: IOException) {
        } finally {
            p.close()
            if (peers.remove(p.token, p)) events?.peerLost(this, p.token)
        }
    }

    private suspend fun housekeeping() {
        while (scope.isActive) {
            delay(15_000)
            // restart scan periodically: some stacks silently stop delivering results
            runCatching { adapter?.bluetoothLeScanner?.stopScan(scanCb) }
            startScan()
            seen.entries.removeIf { System.currentTimeMillis() - it.value > 60_000 }
        }
    }

    override fun send(peer: String?, bytes: ByteArray) {
        val targets = if (peer == null) peers.values.toList() else listOfNotNull(peers[peer])
        for (p in targets) p.enqueue(bytes)
    }

    override fun stats(): LinkStats = LinkStats(estBps = 300_000u, rttMs = 40u, lossPct = 1u)

    private fun ByteArray.toHex() = joinToString("") { "%02x".format(it) }

    private fun compare(a: ByteArray, b: ByteArray): Int {
        for (i in 0 until minOf(a.size, b.size)) {
            val d = (a[i].toInt() and 0xFF) - (b[i].toInt() and 0xFF)
            if (d != 0) return d
        }
        return a.size - b.size
    }

    companion object {
        private const val TAG = "BleTransport"
        val SERVICE_UUID: UUID = UUID.fromString("74697469-0001-4000-8000-746974690001")
        val CH_PSM: UUID = UUID.fromString("74697469-0002-4000-8000-746974690001")
        val CH_NODE: UUID = UUID.fromString("74697469-0003-4000-8000-746974690001")
        val CH_RX: UUID = UUID.fromString("74697469-0004-4000-8000-746974690001")
        val CH_TX: UUID = UUID.fromString("74697469-0005-4000-8000-746974690001")
        private val CCCD: UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")
    }
}
