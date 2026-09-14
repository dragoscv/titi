package ro.titi.app.transport

import uniffi.titi_ffi.FfiLinkClass

/**
 * A link the engine can send bytes over. Each transport owns one engine link id.
 * Transports are dumb pipes: discovery → [TransportEvents.peerSeen], bytes →
 * [TransportEvents.frame]. All engine logic (dedup, routing, crypto) is in Rust.
 */
interface Transport {
    val linkId: UInt
    val linkClass: FfiLinkClass
    /** Max payload per datagram, or null if stream-like. */
    val mtu: UInt?

    fun start(events: TransportEvents)
    fun stop()

    /** peer == null → broadcast to every connected peer on this link. */
    fun send(peer: String?, bytes: ByteArray)

    /** Approximate stats for the engine's link-cost model. */
    fun stats(): LinkStats = LinkStats()
}

data class LinkStats(val estBps: UInt = 0u, val rttMs: UInt = 0u, val lossPct: UByte = 0u)

interface TransportEvents {
    fun linkUp(t: Transport)
    fun linkDown(t: Transport)
    fun peerSeen(t: Transport, token: String)
    fun peerLost(t: Transport, token: String)
    fun frame(t: Transport, token: String, bytes: ByteArray)
    fun stats(t: Transport, s: LinkStats)
}

/** Fixed link ids so persisted routes are stable across restarts. */
object LinkIds {
    const val LAN: UInt = 1u
    const val HOTSPOT: UInt = 2u
    const val WIFI_AWARE: UInt = 3u
    const val NEARBY: UInt = 4u
    const val BLE_L2CAP: UInt = 5u
    const val BLE_GATT: UInt = 6u
    const val BT_RFCOMM: UInt = 7u
    const val INTERNET: UInt = 8u
}
