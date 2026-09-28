package ro.titi.app.transport

import android.content.Context
import android.net.ConnectivityManager
import android.net.LinkProperties
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.wifi.WifiManager
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import uniffi.titi_ffi.FfiLinkClass
import java.net.DatagramPacket
import java.net.Inet4Address
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.MulticastSocket
import java.net.NetworkInterface
import java.net.SocketTimeoutException
import java.util.concurrent.ConcurrentHashMap

/**
 * Same-LAN transport: UDP unicast + IPv4 multicast on any Wi-Fi network the
 * phone is on (router, LocalOnlyHotspot, another phone's hotspot, Wi-Fi Direct
 * group — they all look like a LAN here). Discovery is the engine's own HELLO
 * multicast, so no NSD/mDNS dependency. Token = "ip:port".
 *
 * The same class is instantiated for [LinkIds.HOTSPOT] when we are the hotspot
 * owner (the engine's cost model treats the class differently).
 */
class LanTransport(
    private val ctx: Context,
    override val linkId: UInt = LinkIds.LAN,
    override val linkClass: FfiLinkClass = FfiLinkClass.LAN,
) : Transport {
    override val mtu: UInt? = 1200u

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var events: TransportEvents? = null
    private var socket: MulticastSocket? = null
    private var rxJob: Job? = null
    private var multicastLock: WifiManager.MulticastLock? = null
    private var network: Network? = null
    private val group = InetAddress.getByName(MCAST_ADDR)
    private val peers = ConcurrentHashMap<String, Long>() // token → last seen
    private var up = false

    private val cm by lazy { ctx.getSystemService(ConnectivityManager::class.java) }
    private val wifi by lazy { ctx.applicationContext.getSystemService(WifiManager::class.java) }

    private val netCallback = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(n: Network) { bind(n) }
        override fun onLinkPropertiesChanged(n: Network, lp: LinkProperties) { if (n == network && socket == null) bind(n) }
        override fun onLost(n: Network) { if (n == network) unbind() }
    }

    override fun start(events: TransportEvents) {
        this.events = events
        // One callback, registered once (re-registering the same object throws on newer
        // Android). No INTERNET requirement so internet-less hotspots match too.
        val req = NetworkRequest.Builder()
            .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
            .removeCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            .build()
        runCatching { cm.registerNetworkCallback(req, netCallback) }.onFailure { Log.w(TAG, "register", it) }
        scope.launch { expiryLoop() }
        scope.launch { for ((pkt, s) in txQueue) runCatching { s.send(pkt) }.onFailure { if (!s.isClosed) Log.w(TAG, "tx ${pkt.socketAddress}", it) } }
    }

    override fun stop() {
        runCatching { cm.unregisterNetworkCallback(netCallback) }
        unbind()
        scope.cancel()
    }

    @Synchronized
    private fun bind(n: Network) {
        if (socket != null && network == n) return
        unbind()
        network = n
        try {
            val iface = interfaceFor(n) ?: run { Log.w(TAG, "no interface for $n"); return }
            val s = MulticastSocket(PORT).apply {
                reuseAddress = true
                soTimeout = 1000
                networkInterface = iface
                timeToLive = 1
                loopbackMode = true // disable loopback
                receiveBufferSize = 256 * 1024
            }
            // unicast replies must leave via Wi-Fi, never cellular/VPN
            runCatching { n.bindSocket(s) }
            s.joinGroup(InetSocketAddress(group, PORT), iface)
            multicastLock = wifi.createMulticastLock("titi-lan").apply { setReferenceCounted(false); acquire() }
            socket = s
            up = true
            events?.linkUp(this)
            rxJob = scope.launch { rxLoop(s) }
            Log.i(TAG, "bound on ${iface.name}")
        } catch (e: Exception) {
            Log.w(TAG, "bind failed", e)
            socket = null
        }
    }

    @Synchronized
    private fun unbind() {
        rxJob?.cancel(); rxJob = null
        socket?.let { runCatching { it.leaveGroup(InetSocketAddress(group, PORT), it.networkInterface) }; it.close() }
        socket = null
        multicastLock?.let { if (it.isHeld) it.release() }
        multicastLock = null
        network = null
        if (up) {
            up = false
            peers.keys.forEach { events?.peerLost(this, it) }
            peers.clear()
            events?.linkDown(this)
        }
    }

    private fun interfaceFor(n: Network): NetworkInterface? {
        val lp = cm.getLinkProperties(n) ?: return null
        return lp.interfaceName?.let { runCatching { NetworkInterface.getByName(it) }.getOrNull() }
            ?: NetworkInterface.getNetworkInterfaces().toList().firstOrNull { ni ->
                ni.inetAddresses.toList().any { a -> a is Inet4Address && lp.linkAddresses.any { it.address == a } }
            }
    }

    private suspend fun rxLoop(s: MulticastSocket) {
        val buf = ByteArray(2048)
        while (scope.isActive && !s.isClosed) {
            val pkt = DatagramPacket(buf, buf.size)
            try {
                s.receive(pkt)
            } catch (_: SocketTimeoutException) {
                continue
            } catch (e: Exception) {
                if (!s.isClosed) Log.w(TAG, "rx", e)
                break
            }
            val token = "${pkt.address.hostAddress}:${pkt.port}"
            val first = peers.put(token, System.currentTimeMillis()) == null
            if (first) events?.peerSeen(this, token)
            events?.frame(this, token, pkt.data.copyOf(pkt.length))
        }
    }

    private suspend fun expiryLoop() {
        while (scope.isActive) {
            kotlinx.coroutines.delay(5_000)
            val now = System.currentTimeMillis()
            for ((tok, seen) in peers) {
                if (now - seen > PEER_EXPIRY_MS) {
                    peers.remove(tok)
                    events?.peerLost(this, tok)
                }
            }
        }
    }

    override fun send(peer: String?, bytes: ByteArray) {
        val s = socket ?: return
        val target = if (peer == null) {
            InetSocketAddress(group, PORT)
        } else {
            val (host, port) = peer.substringBeforeLast(':') to peer.substringAfterLast(':').toIntOrNull()
            if (port == null) return
            InetSocketAddress(host, port)
        }
        // one ordered writer; under pressure drop the oldest (stale voice is worthless)
        txQueue.trySend(DatagramPacket(bytes, bytes.size, target) to s)
    }

    private val txQueue = kotlinx.coroutines.channels.Channel<Pair<DatagramPacket, MulticastSocket>>(64, kotlinx.coroutines.channels.BufferOverflow.DROP_OLDEST)

    override fun stats(): LinkStats = LinkStats(estBps = 5_000_000u, rttMs = 8u, lossPct = 0u)

    companion object {
        private const val TAG = "LanTransport"
        const val PORT = 41414
        const val MCAST_ADDR = "239.77.84.84" // 'M','T','T'
        private const val PEER_EXPIRY_MS = 40_000L
    }
}
