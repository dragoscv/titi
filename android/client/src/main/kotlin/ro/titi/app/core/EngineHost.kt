package ro.titi.app.core

import android.content.Context
import android.util.Log
import kotlinx.coroutines.CoroutineExceptionHandler
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExecutorCoroutineDispatcher
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import ro.titi.app.audio.AudioEngine
import ro.titi.client.R
import ro.titi.app.audio.Cues
import ro.titi.app.data.Prefs
import ro.titi.app.transport.LinkStats
import ro.titi.app.transport.Transport
import ro.titi.app.transport.TransportEvents
import uniffi.titi_ffi.FfiAction
import uniffi.titi_ffi.FfiChatItem
import uniffi.titi_ffi.FfiLinkClass
import uniffi.titi_ffi.FfiMessageBody
import uniffi.titi_ffi.FfiPriority
import uniffi.titi_ffi.FfiProfile
import uniffi.titi_ffi.FfiQuoteKind
import uniffi.titi_ffi.FfiReplyState
import uniffi.titi_ffi.FfiUiEvent
import uniffi.titi_ffi.GroupSummary
import uniffi.titi_ffi.MemberSummary
import uniffi.titi_ffi.TitiEngine
import uniffi.titi_ffi.groupHash
import java.io.File
import java.util.concurrent.Executors
import kotlin.random.Random

// ---- UI state ---------------------------------------------------------------

data class Peer(val node: String, val name: String, val hue: Int, val link: FfiLinkClass, val inGroup: Boolean, val bars: Int = 0, val hops: Int = 0)

enum class FloorState { Idle, Pending, Talking, Busy, Queued }

/** Quote shown above a reply. [state] tells whether the original still exists here. */
data class ChatReply(val id: String, val sender: String, val senderName: String, val mine: Boolean, val preview: String, val kind: FfiQuoteKind, val state: FfiReplyState)

data class ChatReaction(val emoji: String, val count: Int, val mine: Boolean)

/** One timeline row (message model v2). [body] null = deleted for everyone (tombstone). */
data class ChatMessage(
    val id: String,
    val group: String,
    val from: String,
    val fromName: String,
    val sentMs: Long,
    val sortMs: Long,
    val body: FfiMessageBody?,
    val mine: Boolean,
    val ackCount: Int = 0,
    val edited: Int = 0,
    val reply: ChatReply? = null,
    val reactions: List<ChatReaction> = emptyList(),
    val mentions: List<String> = emptyList(),
    val mentionAll: Boolean = false,
    val mentionsMe: Boolean = false,
    val forwarded: Int = 0,
) {
    val deleted: Boolean get() = body == null
    val acked: Boolean get() = ackCount > 0
    val text: String? get() = (body as? FfiMessageBody.Text)?.text
    /** Author-only, text-only, inside the core's edit window and edit budget. */
    fun canEdit(nowMs: Long = System.currentTimeMillis()): Boolean = mine && body is FfiMessageBody.Text && nowMs - sentMs < ChatRules.editWindowMs && edited < ChatRules.MAX_EDITS
    fun canDelete(): Boolean = mine && !deleted
}

data class EditRevision(val text: String, val sentMs: Long)

object ChatRules {
    const val MAX_EDITS = 10
    /** Messages forwarded this many times show "Forwarded many times". */
    const val FORWARDED_MANY = 5
    val editWindowMs: Long by lazy { runCatching { uniffi.titi_ffi.editWindowMs().toLong() }.getOrDefault(24 * 3600_000L) }
    val quickReactions: List<String> by lazy { runCatching { uniffi.titi_ffi.quickReactions() }.getOrDefault(listOf("👍", "❤️", "😂", "😮", "😢", "🙏")) }
}

data class Invite(val group: String, val name: String, val host: String, val hostName: String, val members: Int)

data class GroupState(
    val id: String,
    val name: String,
    val fullDuplex: Boolean,
    val isCreator: Boolean,
    val members: List<MemberSummary>,
    val floor: FloorState = FloorState.Idle,
    val talkerName: String? = null,
    val talkerNode: String? = null,
    val talkerPrio: Int = 0,
    val handover: String = "stable",
    val link: FfiLinkClass? = null,
    val profile: FfiProfile = FfiProfile.STD,
    val suspended: Boolean = false,
    val unread: Int = 0,
)

data class RadioState(
    val nodeId: String = "",
    val groups: List<GroupState> = emptyList(),
    val activeGroup: String? = null,
    val peers: Map<String, Peer> = emptyMap(),
    val invites: List<Invite> = emptyList(),
    val muted: Boolean = false,
    val running: Boolean = false,
    /** Group id where *we* have an SOS active, or null. */
    val sosGroup: String? = null,
) {
    val active: GroupState? get() = groups.firstOrNull { it.id == activeGroup }
}

/** Background-worthy events the host service turns into high-priority notifications. */
sealed interface Alert {
    data class Sos(val group: String, val groupName: String, val fromName: String, val latE7: Long, val lonE7: Long, val cancelled: Boolean) : Alert
    data class InviteOffered(val invite: Invite) : Alert
}

sealed interface Toast {
    data class Text(val res: Int, val arg: String? = null) : Toast
    data class Raw(val text: String) : Toast
}

sealed interface Cue { data object Granted : Cue; data object Released : Cue; data object Denied : Cue; data object Warning : Cue; data object Incoming : Cue; data object Sos : Cue }

// ---- host -------------------------------------------------------------------

/**
 * Owns the Rust engine. Everything that touches [engine] runs on [dispatcher]
 * (a single thread) so the core stays single-threaded and deterministic.
 */
class EngineHost(private val ctx: Context, private val prefs: Prefs) : TransportEvents {
    private val executor = Executors.newSingleThreadExecutor { Thread(it, "titi-engine").apply { priority = Thread.MAX_PRIORITY } }
    private val dispatcher: ExecutorCoroutineDispatcher = executor.asCoroutineDispatcher()
    private val crashGuard = CoroutineExceptionHandler { _, t -> Log.e(TAG, "engine coroutine failed", t) }
    private val scope = CoroutineScope(SupervisorJob() + dispatcher + crashGuard)

    private val settings0 = prefs.settingsNow()
    val engine: TitiEngine = TitiEngine(prefs.identitySeed() ?: ByteArray(0), settings0.name.ifBlank { "Titi" }, settings0.hue.toUShort(), Random.nextLong().toULong())
    val nodeIdHex: String = engine.nodeId().toHex()

    private val _state = MutableStateFlow(RadioState(nodeId = nodeIdHex))
    val state: StateFlow<RadioState> = _state.asStateFlow()
    private val _messages = MutableStateFlow<List<ChatMessage>>(emptyList())
    val messages: StateFlow<List<ChatMessage>> = _messages.asStateFlow()
    private val _toasts = MutableSharedFlow<Toast>(extraBufferCapacity = 8)
    val toasts: SharedFlow<Toast> = _toasts.asSharedFlow()
    private val _cues = MutableSharedFlow<Cue>(extraBufferCapacity = 8)
    val cues: SharedFlow<Cue> = _cues.asSharedFlow()
    private val _alerts = MutableSharedFlow<Alert>(extraBufferCapacity = 8)
    val alerts: SharedFlow<Alert> = _alerts.asSharedFlow()
    /** Mic/mixer level, 50 Hz — kept out of [state] so it doesn't recompose/notify everything. */
    private val _level = MutableStateFlow(-60f)
    val level: StateFlow<Float> = _level.asStateFlow()

    /** Cues play from the host (not the Activity) so they work with the screen off. */
    private val cuePlayer = Cues(ctx)

    val audio = AudioEngine(ctx) { pcm, now -> post { apply(engine.onAudioIn(pcm.toLeBytes(), now.toULong())) } }.also {
        it.onCaptureFailed = { post { apply(engine.pttUp(now())) }; toast(Toast.Text(R.string.error_mic)) }
    }
    private val transports = java.util.concurrent.ConcurrentHashMap<UInt, Transport>()
    private var tickJob: Job? = null
    private var codeJob: Job? = null
    private val names = HashMap<String, String>() // node hex → display name
    private var pendingCode: Pair<String, Long>? = null // code, until
    private var pendingLinkRoom: Pair<ByteArray, Long>? = null // group hash, until
    /** Groups whose newest history page is loaded; declared before `init` (refreshGroups runs there). */
    private val loadedGroups = HashSet<String>()

    init {
        if (prefs.identitySeed() == null) prefs.saveIdentitySeed(engine.identitySeed())
        runCatching { engine.openChatStore(File(ctx.filesDir, "chat.db").path) }.onFailure { Log.w(TAG, "chat store unavailable, messages stay in memory", it) }
        prefs.groupsBlob()?.let { runCatching { engine.restoreGroups(it, now()) }.onFailure { e -> Log.w(TAG, "restore", e) } }
        refreshGroups()
        scope.launch {
            prefs.settings.collect { s ->
                cuePlayer.pack = s.soundPack; cuePlayer.hapticsOnly = s.hapticsOnly
                val cap = when (s.quality) {
                    ro.titi.app.data.QualityOverride.Auto -> if (s.batterySaver) FfiProfile.LOW else null
                    ro.titi.app.data.QualityOverride.Hq -> FfiProfile.HQ
                    ro.titi.app.data.QualityOverride.Std -> FfiProfile.STD
                    ro.titi.app.data.QualityOverride.Low -> FfiProfile.LOW
                }
                // battery saver: don't spend radio time forwarding other members' voice
                post { engine.setMaxProfile(cap); engine.setRelayCapable(!s.batterySaver) }
            }
        }
        scope.launch { _cues.collect { cuePlayer.play(it) } }
    }

    // ---- lifecycle -------------------------------------------------------

    fun start(transports: List<Transport>) = post {
        if (_state.value.running) return@post
        _state.update { it.copy(running = true) }
        audio.startPlayback()
        for (t in transports) {
            this.transports[t.linkId] = t
            runCatching { t.start(this) }.onFailure { Log.w(TAG, "transport ${t.linkId} start", it) }
        }
        tickJob = scope.launch {
            var n = 0
            while (isActive) {
                runCatching {
                    apply(engine.tick(now()))
                    if (++n % 500 == 0) syncRelayRoom() // rendezvous slots rotate every 10 min
                }.onFailure { Log.e(TAG, "tick", it) }
                delay(20)
            }
        }
        syncRelayRoom()
    }

    /** Runs on the engine thread: releases the floor, then tears down transports and audio. */
    fun stop() = post {
        if (!_state.value.running) return@post
        runCatching { apply(engine.pttUp(now())) }
        tickJob?.cancel(); tickJob = null
        codeJob?.cancel(); codeJob = null
        transports.values.forEach { runCatching { it.stop() } }
        transports.clear()
        audio.release()
        _state.update { it.copy(running = false, peers = emptyMap()) }
    }

    fun post(block: () -> Unit) {
        scope.launch { runCatching(block).onFailure { Log.e(TAG, "engine task", it) } }
    }

    /** Replace one transport at runtime (settings toggles). Runs on the engine thread. */
    fun setTransport(linkId: UInt, t: Transport?) = post {
        transports.remove(linkId)?.let { old -> runCatching { old.stop() }; apply(engine.onLinkDown(linkId, now())) }
        if (t != null && _state.value.running) {
            transports[linkId] = t
            runCatching { t.start(this) }.onFailure { Log.w(TAG, "transport $linkId start", it) }
            syncRelayRoom()
        }
    }
    fun transport(linkId: UInt): Transport? = transports[linkId]

    private fun now(): ULong = System.currentTimeMillis().toULong()

    // ---- transport events (any thread → engine thread) --------------------

    override fun linkUp(t: Transport) = post { apply(engine.onLinkUp(t.linkId, t.linkClass, t.mtu, now())); val s = t.stats(); apply(engine.onLinkStats(t.linkId, s.estBps, s.rttMs, s.lossPct, now())) }
    override fun linkDown(t: Transport) = post { apply(engine.onLinkDown(t.linkId, now())) }
    override fun peerSeen(t: Transport, token: String) = post { apply(engine.onPeerSeen(t.linkId, token, now())) }
    override fun peerLost(t: Transport, token: String) = post { apply(engine.onPeerLost(t.linkId, token, now())) }
    override fun frame(t: Transport, token: String, bytes: ByteArray) = post { apply(engine.onFrame(t.linkId, token, bytes, now())) }
    override fun stats(t: Transport, s: LinkStats) = post { apply(engine.onLinkStats(t.linkId, s.estBps, s.rttMs, s.lossPct, now())) }

    // ---- user intents -----------------------------------------------------

    fun pttDown(prio: FfiPriority = FfiPriority.NORMAL) = post { Log.d(TAG, "ptt down"); apply(engine.pttDown(prio, now())) }
    fun pttUp() = post { Log.d(TAG, "ptt up"); apply(engine.pttUp(now())) }
    fun setMuted(m: Boolean) { audio.muted = m; _state.update { it.copy(muted = m) } }

    fun createGroup(name: String) = post { runCatching { apply(engine.createGroup(name, now())) }.onFailure { err(it) }; refreshGroups(); syncRelayRoom() }
    fun leaveGroup(id: String) = post { runCatching { apply(engine.leaveGroup(id.hexToBytes(), now())) }.onFailure { err(it) }; refreshGroups(); syncRelayRoom() }
    /** Creator only: deletes the group for every member. */
    fun dissolveGroup(id: String) = post { runCatching { apply(engine.dissolveGroup(id.hexToBytes(), now())) }.onFailure { err(it) }; refreshGroups(); syncRelayRoom() }
    fun setActiveGroup(id: String) = post { runCatching { engine.setActiveGroup(id.hexToBytes()) }; refreshGroups(); syncRelayRoom(); _state.update { s -> s.copy(groups = s.groups.map { g -> if (g.id == id) g.copy(unread = 0) else g }) } }
    fun setFullDuplex(id: String, on: Boolean) = post { runCatching { apply(engine.setFullDuplex(id.hexToBytes(), on, now())) }.onFailure { err(it) }; refreshGroups() }
    fun invitePeer(group: String, node: String) = post { runCatching { apply(engine.invitePeer(group.hexToBytes(), node.hexToBytes(), now())) }.onFailure { err(it) } }
    fun acceptInvite(inv: Invite) = post {
        _state.update { s -> s.copy(invites = s.invites.filterNot { it.group == inv.group }) }
        runCatching { apply(engine.acceptInvite(inv.group.hexToBytes(), inv.host.hexToBytes(), now())) }.onFailure { err(it) }
    }
    fun declineInvite(inv: Invite) = post {
        _state.update { s -> s.copy(invites = s.invites.filterNot { it.group == inv.group }) }
        runCatching { apply(engine.declineInvite(inv.group.hexToBytes(), inv.host.hexToBytes(), now())) }
    }
    fun joinByCode(code: String) = post {
        // sit in the code's relay rendezvous rooms for 2 min and retry until a member appears
        pendingCode = code to (System.currentTimeMillis() + 120_000)
        // done = a group we did not have before the join exists (checking the *active* group's
        // member count ended the join instantly whenever an existing group was active)
        val before = _state.value.groups.map { it.id }.toSet()
        syncRelayRoom()
        apply(engine.joinByCode(code, now()))
        codeJob?.cancel()
        codeJob = scope.launch {
            while (isActive) {
                delay(3000)
                val p = pendingCode ?: break
                if (System.currentTimeMillis() > p.second) { pendingCode = null; syncRelayRoom(); break }
                if (_state.value.groups.any { it.id !in before }) { pendingCode = null; syncRelayRoom(); break }
                runCatching { apply(engine.joinByCode(p.first, now())) }.onFailure { Log.w(TAG, "joinByCode", it) }
            }
        }
    }
    /** Invite link: sit in that group's relay room (group_hash(uuid)) and retry every 3 s, like the web host. */
    fun joinByLink(url: String) = post {
        val before = _state.value.groups.map { it.id }.toSet()
        pendingLinkRoom = linkUuid(url)?.let { u -> runCatching { groupHash(u) }.getOrNull() }?.let { it to (System.currentTimeMillis() + 120_000) }
        syncRelayRoom()
        apply(engine.joinByLink(url, now()))
        if (pendingLinkRoom == null) return@post
        codeJob?.cancel()
        codeJob = scope.launch {
            while (isActive) {
                delay(3000)
                val p = pendingLinkRoom ?: break
                if (System.currentTimeMillis() > p.second || _state.value.groups.any { it.id !in before }) { pendingLinkRoom = null; syncRelayRoom(); break }
                runCatching { apply(engine.joinByLink(url, now())) }.onFailure { Log.w(TAG, "joinByLink", it) }
            }
        }
    }

    private fun linkUuid(url: String): ByteArray? {
        val seg = url.substringAfter("/j/", "").substringBefore('/')
        if (seg.isEmpty()) return null
        val flags = android.util.Base64.URL_SAFE or android.util.Base64.NO_PADDING or android.util.Base64.NO_WRAP
        return runCatching { android.util.Base64.decode(seg, flags) }.getOrNull()?.takeIf { it.size == 16 }
    }
    /** Text with an optional quote ([replyTo] message id) and mentions (node ids). */
    fun sendText(group: String, text: String, replyTo: String? = null, mentions: List<String> = emptyList(), mentionAll: Boolean = false) = post {
        runCatching { apply(engine.sendTextV2(group.hexToBytes(), text, replyTo?.hexToBytes(), mentions.map { it.hexToBytes() }, mentionAll, now())) }.onFailure { err(it) }
    }
    /** One reaction per member; empty [emoji] removes ours. */
    fun react(group: String, target: String, emoji: String) = post {
        runCatching { apply(engine.react(group.hexToBytes(), target.hexToBytes(), emoji, now())) }.onFailure { err(it) }
    }
    /** Tap on a reaction chip: same emoji as ours removes it, otherwise sets it. */
    fun toggleReaction(m: ChatMessage, emoji: String) = react(m.group, m.id, if (m.reactions.any { it.mine && it.emoji == emoji }) "" else emoji)
    fun editMessage(group: String, target: String, text: String) = post {
        runCatching { apply(engine.editMessage(group.hexToBytes(), target.hexToBytes(), text, now())) }.onFailure { err(it) }
    }
    fun deleteMessage(group: String, target: String) = post {
        runCatching { apply(engine.deleteMessage(group.hexToBytes(), target.hexToBytes(), now())) }.onFailure { err(it) }
    }
    fun forward(fromGroup: String, target: String, toGroup: String) = post {
        runCatching { apply(engine.forward(fromGroup.hexToBytes(), target.hexToBytes(), toGroup.hexToBytes(), now())) }.onFailure { err(it) }
    }
    fun setDraft(group: String, text: String) = post { runCatching { engine.setDraft(group.hexToBytes(), text) }.onFailure { Log.w(TAG, "setDraft", it) } }
    suspend fun draft(group: String): String? = withContext(dispatcher) { runCatching { engine.draft(group.hexToBytes()) }.getOrNull() }
    /** Diacritic-blind prefix search, newest first; [group] null = all groups. */
    suspend fun search(group: String?, query: String, limit: Int = 50): List<ChatMessage> = withContext(dispatcher) {
        runCatching { engine.search(group?.hexToBytes(), query, limit.toUInt()).map { it.toMessage() } }.onFailure { Log.w(TAG, "search", it) }.getOrDefault(emptyList())
    }
    suspend fun editHistory(target: String): List<EditRevision> = withContext(dispatcher) {
        runCatching { engine.editHistory(target.hexToBytes()).map { EditRevision(it.text, it.sentMs.toLong()) } }.onFailure { Log.w(TAG, "editHistory", it) }.getOrDefault(emptyList())
    }
    /** Loads the page before the oldest loaded row; returns how many new rows arrived. */
    suspend fun loadOlder(group: String, limit: Int = 200): Int = withContext(dispatcher) {
        val oldest = _messages.value.filter { it.group == group }.minOfOrNull { it.sortMs } ?: return@withContext 0
        val page = runCatching { engine.history(group.hexToBytes(), oldest.toULong(), limit.toUInt()) }.onFailure { Log.w(TAG, "history", it) }.getOrDefault(emptyList())
        val known = _messages.value.mapTo(HashSet()) { it.id }
        val fresh = page.map { it.toMessage() }.filter { it.id !in known }
        if (fresh.isNotEmpty()) upsert(fresh)
        fresh.size
    }
    fun sendLocation(group: String, lat: Double, lon: Double, acc: Float, breadcrumb: Boolean) = post {
        runCatching { apply(engine.sendLocation(group.hexToBytes(), lat, lon, acc, breadcrumb, now())) }.onFailure { err(it) }
    }
    fun sendSos(group: String, lat: Double, lon: Double, note: String, cancelled: Boolean) = post {
        runCatching { apply(engine.sendSos(group.hexToBytes(), lat, lon, note, cancelled, now())) }.onFailure { err(it) }
        _state.update { it.copy(sosGroup = if (cancelled) null else group) }
    }

    /** SOS = alert message with location + emergency-priority floor; [cancelSos] undoes both. */
    fun raiseSos(group: String, lat: Double, lon: Double) {
        sendSos(group, lat, lon, "", cancelled = false)
        pttDown(FfiPriority.EMERGENCY)
    }
    fun cancelSos() {
        val g = _state.value.sosGroup ?: return
        pttUp()
        sendSos(g, 0.0, 0.0, "", cancelled = true)
    }
    fun sendVoiceNote(group: String, profile: FfiProfile, durationMs: Int, packets: ByteArray) = post {
        runCatching { apply(engine.sendVoiceNote(group.hexToBytes(), profile, durationMs.toUInt(), packets, now())) }.onFailure { err(it) }
    }
    fun setDisplayName(name: String, hue: Int) = post { engine.setDisplayName(name, hue.toUShort()) }

    /** "code|secondsLeft" for the active group, or null. */
    fun currentCode(group: String): Pair<String, Int>? =
        runCatching { engine.currentCode(group.hexToBytes(), now()) }.getOrNull()?.split('|')?.let { it[0] to (it.getOrNull(1)?.toIntOrNull() ?: 0) }
    fun deepLink(group: String, validMs: Long = 10 * 60_000L): String? = runCatching { engine.deepLink(group.hexToBytes(), now(), validMs.toULong()) }.getOrNull()

    // ---- action dispatch --------------------------------------------------

    private fun apply(actions: List<FfiAction>) {
        for (a in actions) {
            when (a) {
                is FfiAction.Send -> transports[a.link]?.send(a.peer, a.bytes)
                is FfiAction.Play -> { playCount++; if (playCount % 50 == 1) Log.d(TAG, "play #$playCount ${a.pcmLe.size}B"); audio.play(a.pcmLe.toShorts()) }
                is FfiAction.Capture -> if (a.active) audio.startCapture() else audio.stopCapture()
                is FfiAction.Persist -> if (a.key == "groups") prefs.saveGroupsBlob(a.value)
                is FfiAction.WakeAt -> Unit // ticking at 20 ms covers it; used by iOS/wasm
                is FfiAction.Ui -> { if (a.event !is FfiUiEvent.Level && a.event !is FfiUiEvent.PeerDiscovered) Log.d(TAG, "ui ${a.event.javaClass.simpleName}"); onUi(a.event) }
            }
        }
    }
    private var playCount = 0

    private fun onUi(e: FfiUiEvent) {
        when (e) {
            is FfiUiEvent.PeerDiscovered -> {
                val id = e.node.toHex(); names[id] = e.name
                _state.update { s -> s.copy(peers = s.peers + (id to (s.peers[id]?.copy(name = e.name, hue = e.hue.toInt(), inGroup = e.inGroup) ?: Peer(id, e.name, e.hue.toInt(), e.link, e.inGroup)))) }
            }
            is FfiUiEvent.PeerLost -> _state.update { s -> s.copy(peers = s.peers - e.node.toHex()) }
            is FfiUiEvent.PeerLink -> {
                val id = e.node.toHex()
                _state.update { s -> s.copy(peers = s.peers[id]?.let { p -> s.peers + (id to p.copy(link = e.link, bars = e.bars.toInt(), hops = e.hops.toInt())) } ?: s.peers) }
            }
            FfiUiEvent.FloorGranted -> { updateActive { it.copy(floor = FloorState.Talking, talkerName = null, talkerNode = nodeIdHex) }; cue(Cue.Granted) }
            is FfiUiEvent.FloorDenied -> { updateActive { it.copy(floor = FloorState.Busy) }; cue(Cue.Denied) }
            is FfiUiEvent.FloorTaken -> {
                val gid = e.group.toHex(); val holder = e.holder.toHex()
                names[holder] = e.name
                updateGroup(gid) { it.copy(floor = if (holder == nodeIdHex) FloorState.Talking else FloorState.Busy, talkerName = e.name, talkerNode = holder, talkerPrio = e.prio.toInt()) }
                if (holder != nodeIdHex) cue(if (e.prio.toInt() >= 2) Cue.Sos else Cue.Incoming)
            }
            is FfiUiEvent.FloorIdle -> {
                val gid = e.group.toHex()
                val wasMine = _state.value.groups.firstOrNull { it.id == gid }?.talkerNode == nodeIdHex
                updateGroup(gid) { it.copy(floor = FloorState.Idle, talkerName = null, talkerNode = null, talkerPrio = 0) }
                if (wasMine) cue(Cue.Released)
            }
            FfiUiEvent.TalkWarning -> { cue(Cue.Warning); toast(Toast.Text(R.string.group_talk_warning)) }
            FfiUiEvent.TalkTimeout -> toast(Toast.Text(R.string.group_talk_timeout))
            is FfiUiEvent.InviteOffered -> {
                val inv = Invite(e.group.toHex(), e.name, e.host.toHex(), e.hostName, e.members.toInt())
                _state.update { s -> s.copy(invites = s.invites.filterNot { it.group == inv.group } + inv) }
                cue(Cue.Incoming)
                _alerts.tryEmit(Alert.InviteOffered(inv))
            }
            is FfiUiEvent.Joined -> { refreshGroups(); syncRelayRoom(); toast(Toast.Text(R.string.joined, e.name)) }
            is FfiUiEvent.JoinFailed -> { Log.w(TAG, "join failed: ${e.reason}"); toast(Toast.Text(R.string.join_failed_generic)) }
            is FfiUiEvent.MemberJoined -> { names[e.node.toHex()] = e.name; refreshGroups() }
            is FfiUiEvent.MemberLeft -> refreshGroups()
            is FfiUiEvent.GroupDissolved -> { refreshGroups(); syncRelayRoom(); toast(Toast.Text(R.string.group_dissolved, e.name)) }
            // the timeline row itself arrives as ChatUpdated; Message only drives unread + alerts
            is FfiUiEvent.Message -> {
                val gid = e.group.toHex(); val from = e.from.toHex()
                val mine = from == nodeIdHex
                if (!mine && _state.value.activeGroup != gid) updateGroup(gid) { it.copy(unread = it.unread + 1) }
                val body = e.body
                if (body is FfiMessageBody.Sos && !mine) {
                    if (!body.cancelled) cue(Cue.Sos)
                    val gName = _state.value.groups.firstOrNull { it.id == gid }?.name ?: ""
                    _alerts.tryEmit(Alert.Sos(gid, gName, names[from] ?: from.take(6), body.latE7, body.lonE7, body.cancelled))
                }
            }
            is FfiUiEvent.MessageAcked -> { val id = e.msgUuid.toHex(); _messages.update { l -> l.map { if (it.id == id && it.ackCount == 0) it.copy(ackCount = 1) else it } } }
            is FfiUiEvent.ChatUpdated -> upsert(listOf(e.item.toMessage()))
            is FfiUiEvent.Handover -> updateGroup(e.group.toHex()) { it.copy(handover = e.state, link = e.link, profile = e.profile) }
            is FfiUiEvent.Suspended -> updateGroup(e.group.toHex()) { it.copy(suspended = true) }
            is FfiUiEvent.Resumed -> updateGroup(e.group.toHex()) { it.copy(suspended = false) }
            is FfiUiEvent.ModeChanged -> updateGroup(e.group.toHex()) { it.copy(fullDuplex = e.fullDuplex) }
            is FfiUiEvent.Level -> _level.value = e.dbfs
            is FfiUiEvent.Error -> { Log.w(TAG, "core error: ${e.message}"); toast(Toast.Text(R.string.error_generic)) }
        }
    }

    private fun refreshGroups() {
        val gs: List<GroupSummary> = engine.groups()
        val prev = _state.value.groups.associateBy { it.id }
        val list = gs.map { g ->
            val id = g.id.toHex()
            val members = runCatching { engine.members(g.id) }.getOrDefault(emptyList())
            members.forEach { names[it.node.toHex()] = it.name }
            (prev[id] ?: GroupState(id, g.name, g.fullDuplex, g.isCreator, members)).copy(name = g.name, fullDuplex = g.fullDuplex, isCreator = g.isCreator, members = members)
        }
        _state.update { it.copy(groups = list, activeGroup = gs.firstOrNull { g -> g.isActive }?.id?.toHex()) }
        syncHistory(list.map { it.id }.toSet())
    }

    // ---- chat timeline ----------------------------------------------------

    /** Loads the newest page for groups seen for the first time; drops rows of groups we left. */
    private fun syncHistory(ids: Set<String>) {
        loadedGroups.retainAll(ids)
        if (_messages.value.any { it.group !in ids }) _messages.update { l -> l.filter { it.group in ids } }
        for (id in ids) {
            if (!loadedGroups.add(id)) continue
            val page = runCatching { engine.history(id.hexToBytes(), null, HISTORY_PAGE.toUInt()) }.onFailure { Log.w(TAG, "history", it) }.getOrDefault(emptyList())
            if (page.isNotEmpty()) upsert(page.map { it.toMessage() })
        }
    }

    /** Replace rows by id (or add them), keeping the list sorted by sortMs. */
    private fun upsert(rows: List<ChatMessage>) {
        _messages.update { l ->
            val byId = LinkedHashMap<String, ChatMessage>(l.size + rows.size)
            l.forEach { byId[it.id] = it }
            rows.forEach { byId[it.id] = it }
            byId.values.sortedWith(compareBy<ChatMessage> { it.sortMs }.thenBy { it.id })
        }
    }

    private fun nameOf(node: String): String = if (node == nodeIdHex) settings0.name.ifBlank { "Titi" } else names[node] ?: node.take(6)

    private fun FfiChatItem.toMessage(): ChatMessage {
        val sender = this.from.toHex()
        return ChatMessage(
            id = msgUuid.toHex(),
            group = group.toHex(),
            from = sender,
            fromName = nameOf(sender),
            sentMs = sentMs.toLong(),
            sortMs = sortMs.toLong(),
            body = if (deleted) null else this.body,
            mine = sender == nodeIdHex,
            ackCount = acked.toInt(),
            edited = edited.toInt(),
            reply = reply?.let { r -> val s = r.sender.toHex(); ChatReply(r.msgUuid.toHex(), s, nameOf(s), s == nodeIdHex, r.preview, r.kind, r.state) },
            reactions = reactions.map { ChatReaction(it.emoji, it.count.toInt(), it.mine) },
            mentions = mentions.map { it.toHex() },
            mentionAll = mentionAll,
            mentionsMe = mentionsMe,
            forwarded = forwarded.toInt(),
        )
    }

    /** Relay rooms: every group (voice) + their rendezvous rooms + a pending code's rooms. */
    private fun syncRelayRoom() {
        val relay = transports[ro.titi.app.transport.LinkIds.INTERNET] as? ro.titi.app.transport.RelayTransport ?: return
        val rooms = mutableListOf<Pair<ByteArray, Boolean>>()
        for (g in _state.value.groups) {
            val gid = g.id.hexToBytes()
            runCatching { groupHash(gid) }.getOrNull()?.let { rooms += it to false }
            runCatching { engine.rendezvousForGroup(gid, now()) }.getOrDefault(emptyList()).forEach { rooms += it to true }
        }
        pendingCode?.let { (code, _) -> uniffi.titi_ffi.rendezvousForCode(code, now()).forEach { rooms += it to true } }
        pendingLinkRoom?.let { (h, _) -> rooms += h to true }
        relay.setRooms(rooms)
    }

    private inline fun updateActive(f: (GroupState) -> GroupState) { _state.value.activeGroup?.let { updateGroup(it, f) } }
    private inline fun updateGroup(id: String, f: (GroupState) -> GroupState) = _state.update { s -> s.copy(groups = s.groups.map { if (it.id == id) f(it) else it }) }
    private fun toast(t: Toast) { _toasts.tryEmit(t) }
    private fun cue(c: Cue) { _cues.tryEmit(c) }
    private fun err(t: Throwable) { Log.w(TAG, "engine", t); toast(Toast.Text(R.string.error_generic)) }

    companion object { private const val TAG = "EngineHost"; private const val HISTORY_PAGE = 200 }
}

// ---- byte helpers -------------------------------------------------------------

fun ByteArray.toHex(): String = joinToString("") { "%02x".format(it) }
fun String.hexToBytes(): ByteArray = ByteArray(length / 2) { i -> ((Character.digit(this[i * 2], 16) shl 4) or Character.digit(this[i * 2 + 1], 16)).toByte() }
fun ShortArray.toLeBytes(): ByteArray { val b = ByteArray(size * 2); for (i in indices) { val v = this[i].toInt(); b[i * 2] = v.toByte(); b[i * 2 + 1] = (v shr 8).toByte() }; return b }
fun ByteArray.toShorts(): ShortArray = ShortArray(size / 2) { i -> ((this[i * 2].toInt() and 0xFF) or (this[i * 2 + 1].toInt() shl 8)).toShort() }
