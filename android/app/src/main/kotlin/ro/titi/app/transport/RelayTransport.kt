package ro.titi.app.transport

import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import okio.ByteString.Companion.toByteString
import uniffi.titi_ffi.FfiLinkClass
import java.util.concurrent.TimeUnit
import kotlin.math.min

/**
 * Internet link via the Cloud Run relay (ADR-0005). One WebSocket, one room
 * per group hash; the engine's frames are forwarded opaque. Token = hex node id
 * of the remote peer (relay unicasts by `dst`; broadcast → `peer = null`).
 *
 * Reconnects with backoff and resumes the room with the server's token.
 */
class RelayTransport(
    private val url: () -> String,
    private val nodeId: () -> ByteArray,
    private val displayName: () -> String,
    private val hue: () -> Int,
) : Transport {
    override val linkId: UInt = LinkIds.INTERNET
    override val linkClass: FfiLinkClass = FfiLinkClass.INTERNET
    override val mtu: UInt? = null

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val client = OkHttpClient.Builder()
        .pingInterval(25, TimeUnit.SECONDS)
        .connectTimeout(10, TimeUnit.SECONDS)
        .build()
    private var events: TransportEvents? = null
    private var ws: WebSocket? = null
    private var connectJob: Job? = null
    private var groupHash: ByteArray? = null
    private var resumeToken: ByteArray = ByteArray(0)
    private var attempt = 0
    @Volatile private var enabled = false
    @Volatile private var joined = false
    private val peers = HashSet<String>()

    override fun start(events: TransportEvents) {
        this.events = events
        enabled = true
        scheduleConnect(0)
    }

    override fun stop() {
        enabled = false
        connectJob?.cancel()
        ws?.close(1000, "bye")
        ws = null
        if (joined) { joined = false; events?.linkDown(this) }
        scope.cancel()
    }

    /** Set the group whose room we should sit in (active group). */
    fun setGroupHash(h: ByteArray?) {
        if (h contentEquals groupHash) return
        groupHash = h
        resumeToken = ByteArray(0)
        ws?.let { sendJoin(it) }
    }

    private fun scheduleConnect(delayMs: Long) {
        if (!enabled) return
        connectJob?.cancel()
        connectJob = scope.launch {
            delay(delayMs)
            connect()
        }
    }

    private fun connect() {
        val req = Request.Builder().url(url()).build()
        ws = client.newWebSocket(req, listener)
    }

    private val listener = object : WebSocketListener() {
        override fun onOpen(webSocket: WebSocket, response: Response) {
            attempt = 0
            sendJoin(webSocket)
        }

        override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
            val b = bytes.toByteArray()
            if (b.isEmpty()) return
            when (b[0].toInt()) {
                TAG_SIGNAL -> onSignal(b, 1)
                TAG_ENVELOPE -> {
                    // src is bytes 8..16 of the envelope
                    if (b.size < 1 + 16) return
                    val src = b.copyOfRange(1 + 8, 1 + 16).toHex()
                    if (peers.add(src)) events?.peerSeen(this@RelayTransport, src)
                    events?.frame(this@RelayTransport, src, b.copyOfRange(1, b.size))
                }
            }
        }

        override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
            Log.w(TAG, "ws failure: ${t.message}")
            onClosedInternal()
        }

        override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
            onClosedInternal()
        }
    }

    private fun onClosedInternal() {
        ws = null
        if (joined) {
            joined = false
            peers.forEach { events?.peerLost(this, it) }
            peers.clear()
            events?.linkDown(this)
        }
        if (enabled) {
            attempt++
            val backoff = min(30_000L, 500L * (1L shl min(attempt, 6)))
            scheduleConnect(backoff)
        }
    }

    private fun sendJoin(s: WebSocket) {
        val gh = groupHash ?: return
        val node = MiniProto.Writer()
            .bytes(1, nodeId())
            .string(3, displayName())
            .varint(4, hue().toLong())
        val join = MiniProto.Writer()
            .bytes(1, gh)
            .message(2, node)
            .bytes(3, resumeToken)
        val sig = MiniProto.Writer().message(F_ROOM_JOIN, join).toByteArray()
        s.send(byteArrayOf(TAG_SIGNAL.toByte()).plus(sig).toByteString())
    }

    private fun onSignal(b: ByteArray, off: Int) {
        val r = MiniProto.Reader(b, off)
        while (r.next()) {
            when (r.field) {
                F_ROOM_JOINED -> {
                    val m = r.message()
                    while (m.next()) {
                        when (m.field) {
                            1 -> resumeToken = m.bytes()
                            2 -> {
                                val p = m.message()
                                while (p.next()) { if (p.field == 1) { val id = p.bytes().toHex(); if (peers.add(id)) events?.peerSeen(this, id) } else p.skip() }
                            }
                            else -> m.skip()
                        }
                    }
                    if (!joined) { joined = true; events?.linkUp(this) }
                    events?.stats(this, stats())
                }
                F_PEER_EVENT -> {
                    val m = r.message()
                    var id: String? = null; var joinedFlag = false
                    while (m.next()) {
                        when (m.field) {
                            1 -> { val p = m.message(); while (p.next()) { if (p.field == 1) id = p.bytes().toHex() else p.skip() } }
                            2 -> joinedFlag = m.varint() != 0L
                            else -> m.skip()
                        }
                    }
                    id?.let {
                        if (joinedFlag) { if (peers.add(it)) events?.peerSeen(this, it) }
                        else if (peers.remove(it)) events?.peerLost(this, it)
                    }
                }
                F_ERROR -> {
                    val m = r.message()
                    var code = 0L; var msg = ""
                    while (m.next()) { when (m.field) { 1 -> code = m.varint(); 2 -> msg = m.string(); else -> m.skip() } }
                    Log.w(TAG, "relay error $code: $msg")
                    if (code == 5L) { resumeToken = ByteArray(0); ws?.let { sendJoin(it) } }
                }
                else -> r.skip()
            }
        }
    }

    override fun send(peer: String?, bytes: ByteArray) {
        val s = ws ?: return
        if (!joined) return
        // the relay routes by the envelope's own dst; peer token is informational
        s.send(byteArrayOf(TAG_ENVELOPE.toByte()).plus(bytes).toByteString())
    }

    override fun stats(): LinkStats = LinkStats(estBps = 200_000u, rttMs = 120u, lossPct = 0u)

    private fun ByteArray.toHex() = joinToString("") { "%02x".format(it) }

    companion object {
        private const val TAG = "RelayTransport"
        const val TAG_SIGNAL = 0x00
        const val TAG_ENVELOPE = 0x01
        // Signal oneof field numbers (signal.proto)
        private const val F_ROOM_JOIN = 1
        private const val F_ROOM_JOINED = 2
        private const val F_PEER_EVENT = 4
        private const val F_ERROR = 9
    }
}
