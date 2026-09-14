package ro.titi.app.transport

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
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
 * initiates the connection; the other side only listens.
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
    private val peers = ConcurrentHashMap<String, Peer>() // MAC → peer
    private val seen = ConcurrentHashMap<String, Long>() // MAC → last advert
    private val connecting = ConcurrentHashMap.newKeySet<String>()
    private var up = false

    private inner class Peer(val mac: String, val sock: BluetoothSocket) {
        val out = DataOutputStream(sock.outputStream.buffered(4096))
        val inp = DataInputStream(sock.inputStream.buffered(4096))
        @Synchronized fun write(b: ByteArray) {
            out.writeShort(b.size); out.write(b); out.flush()
        }
        fun close() { runCatching { sock.close() } }
    }

    fun hasPermissions(): Boolean {
        val need = if (Build.VERSION.SDK_INT >= 31) listOf(Manifest.permission.BLUETOOTH_SCAN, Manifest.permission.BLUETOOTH_ADVERTISE, Manifest.permission.BLUETOOTH_CONNECT)
        else listOf(Manifest.permission.ACCESS_FINE_LOCATION)
        return need.all { ContextCompat.checkSelfPermission(ctx, it) == PackageManager.PERMISSION_GRANTED }
    }

    override fun start(events: TransportEvents) {
        this.events = events
        val a = adapter ?: return
        if (!a.isEnabled || !hasPermissions() || Build.VERSION.SDK_INT < 29) {
            Log.i(TAG, "BLE unavailable (enabled=${a.isEnabled}, perms=${hasPermissions()})")
            return
        }
        try {
            server = a.listenUsingInsecureL2capChannel()
            psm = server!!.psm
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
        if (up) { up = false; events?.linkDown(this) }
        scope.cancel()
    }

    // ---- GATT: expose PSM + node id ---------------------------------------

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
        }) ?: return
        val svc = BluetoothGattService(SERVICE_UUID, BluetoothGattService.SERVICE_TYPE_PRIMARY)
        svc.addCharacteristic(BluetoothGattCharacteristic(CH_PSM, BluetoothGattCharacteristic.PROPERTY_READ, BluetoothGattCharacteristic.PERMISSION_READ))
        svc.addCharacteristic(BluetoothGattCharacteristic(CH_NODE, BluetoothGattCharacteristic.PROPERTY_READ, BluetoothGattCharacteristic.PERMISSION_READ))
        gs.addService(svc)
        gattServer = gs
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
            seen[mac] = System.currentTimeMillis()
            if (peers.containsKey(mac) || !connecting.add(mac)) return
            val remoteNode = r.scanRecord?.getServiceData(ParcelUuid(SERVICE_UUID))
            // glare avoidance: smaller node id initiates
            if (remoteNode == null || compare(nodeId(), remoteNode) < 0) {
                scope.launch { connectTo(r.device) }
            } else {
                connecting.remove(mac)
            }
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

    private suspend fun connectTo(dev: BluetoothDevice) {
        val mac = dev.address
        try {
            val remotePsm = readPsm(dev) ?: return
            val sock = dev.createInsecureL2capChannel(remotePsm)
            sock.connect()
            attach(mac, sock)
        } catch (e: Exception) {
            Log.w(TAG, "connect $mac: ${e.message}")
        } finally {
            connecting.remove(mac)
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
            attach(sock.remoteDevice.address, sock)
        }
    }

    private fun attach(mac: String, sock: BluetoothSocket) {
        val p = Peer(mac, sock)
        peers.put(mac, p)?.close()
        events?.peerSeen(this, mac)
        scope.launch { readLoop(p) }
    }

    private suspend fun readLoop(p: Peer) {
        try {
            while (scope.isActive) {
                val len = p.inp.readUnsignedShort()
                if (len == 0 || len > 4096) break
                val buf = ByteArray(len)
                p.inp.readFully(buf)
                events?.frame(this, p.mac, buf)
            }
        } catch (_: IOException) {
        } finally {
            p.close()
            if (peers.remove(p.mac, p)) events?.peerLost(this, p.mac)
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
        for (p in targets) {
            scope.launch {
                try { p.write(bytes) } catch (e: IOException) { p.close() }
            }
        }
    }

    override fun stats(): LinkStats = LinkStats(estBps = 300_000u, rttMs = 40u, lossPct = 1u)

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
    }
}
