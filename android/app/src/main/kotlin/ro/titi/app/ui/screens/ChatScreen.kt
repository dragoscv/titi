package ro.titi.app.ui.screens

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import androidx.core.net.toUri
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.Send
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Emergency
import androidx.compose.material.icons.rounded.LocationOn
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import ro.titi.app.R
import ro.titi.app.audio.VoiceNoteRecorder
import ro.titi.app.core.ChatMessage
import ro.titi.app.core.ChatRules
import ro.titi.app.core.ChatText
import ro.titi.app.core.EditRevision
import ro.titi.app.core.EngineHost
import ro.titi.app.core.RadioState
import ro.titi.app.core.toHex
import ro.titi.app.ui.components.Avatar
import ro.titi.app.ui.theme.TelemetryStyle
import ro.titi.app.ui.theme.titi
import uniffi.titi_ffi.FfiMessageBody
import java.text.DateFormat
import java.util.Date
import ro.titi.client.R as CR

/** A mention picked in the composer; dropped on send if its "@name" was erased. */
private data class Mention(val name: String, val node: String?)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChatScreen(engine: EngineHost, state: RadioState, gid: String, onBack: () -> Unit) {
    val g = state.groups.firstOrNull { it.id == gid } ?: run { LaunchedEffect(Unit) { onBack() }; return }
    val all by engine.messages.collectAsState()
    val msgs = remember(all, gid) { all.filter { it.group == gid } }
    var text by remember { mutableStateOf("") }
    var draftLoaded by remember { mutableStateOf(false) }
    var replyTo by remember { mutableStateOf<ChatMessage?>(null) }
    var editing by remember { mutableStateOf<ChatMessage?>(null) }
    val mentions = remember { mutableStateListOf<Mention>() }
    var actionsFor by remember { mutableStateOf<ChatMessage?>(null) }
    var moreEmojiFor by remember { mutableStateOf<ChatMessage?>(null) }
    var deleteFor by remember { mutableStateOf<ChatMessage?>(null) }
    var forwardFor by remember { mutableStateOf<ChatMessage?>(null) }
    var historyFor by remember { mutableStateOf<Pair<ChatMessage, List<EditRevision>>?>(null) }
    var searching by remember { mutableStateOf(false) }
    var query by remember { mutableStateOf("") }
    var results by remember { mutableStateOf<List<ChatMessage>>(emptyList()) }
    var highlight by remember { mutableStateOf<String?>(null) }
    val list = rememberLazyListState()
    val ctx = LocalContext.current
    val recorder = remember { VoiceNoteRecorder(ctx) }
    var recording by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    androidx.compose.runtime.DisposableEffect(recorder) { onDispose { recorder.stop() } }
    LaunchedEffect(msgs.lastOrNull()?.id) { if (msgs.isNotEmpty() && highlight == null) list.animateScrollToItem(msgs.size - 1) }
    LaunchedEffect(gid) { engine.setActiveGroup(gid) }
    // draft: restore once per chat, then persist debounced (not while editing a sent message)
    LaunchedEffect(gid) { draftLoaded = false; text = engine.draft(gid).orEmpty(); draftLoaded = true }
    LaunchedEffect(text, draftLoaded, editing) { if (draftLoaded && editing == null) { delay(400); engine.setDraft(gid, text) } }
    LaunchedEffect(query, searching) {
        if (!searching || query.isBlank()) { results = emptyList(); return@LaunchedEffect }
        delay(250); results = engine.search(gid, query.trim(), 50)
    }
    // reached the top of a full page: fetch the previous page from the store
    val full = msgs.size >= 200
    val atTop by remember(full) { androidx.compose.runtime.derivedStateOf { full && list.firstVisibleItemIndex == 0 } }
    LaunchedEffect(atTop) { if (atTop) engine.loadOlder(gid) }
    LaunchedEffect(highlight) { if (highlight != null) { delay(2500); highlight = null } }

    val me = state.nodeId
    val others = g.members.filter { it.node.toHex() != me }
    val mentionQuery = text.substringAfterLast(' ').substringAfterLast('\n').takeIf { it.startsWith("@") }?.drop(1)

    fun jumpTo(id: String) {
        searching = false; query = ""
        scope.launch {
            var idx = engine.messages.value.filter { it.group == gid }.indexOfFirst { it.id == id }
            var pages = 0
            while (idx < 0 && pages++ < 10 && engine.loadOlder(gid) > 0) idx = engine.messages.value.filter { it.group == gid }.indexOfFirst { it.id == id }
            if (idx >= 0) { highlight = id; list.animateScrollToItem(idx) }
        }
    }
    fun send() {
        val t = text.trim()
        if (t.isEmpty()) return
        val ed = editing
        if (ed != null) {
            if (t != ed.text) engine.editMessage(gid, ed.id, t)
            editing = null
        } else {
            val used = mentions.filter { "@${it.name}" in t }
            engine.sendText(gid, t, replyTo?.id, used.mapNotNull { it.node }.distinct(), mentionAll = used.any { it.node == null })
            replyTo = null
        }
        mentions.clear(); text = ""
        engine.setDraft(gid, "")
    }

    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).safeDrawingPadding().imePadding()) {
        Row(Modifier.fillMaxWidth().padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton({ if (searching) { searching = false; query = "" } else onBack() }) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.back)) }
            if (searching) {
                OutlinedTextField(query, { query = it }, Modifier.weight(1f), placeholder = { Text(stringResource(R.string.chat_search_hint)) }, singleLine = true, shape = RoundedCornerShape(22.dp))
                IconButton({ searching = false; query = "" }) { Icon(Icons.Rounded.Close, stringResource(R.string.chat_search_close)) }
            } else {
                Text(g.name, style = MaterialTheme.typography.titleLarge, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                IconButton({ searching = true }) { Icon(Icons.Rounded.Search, stringResource(R.string.chat_search)) }
            }
        }
        if (searching) {
            LazyColumn(Modifier.weight(1f), contentPadding = PaddingValues(vertical = 8.dp)) {
                if (query.isNotBlank() && results.isEmpty()) item { Text(stringResource(R.string.chat_search_empty), Modifier.padding(24.dp), color = MaterialTheme.colorScheme.onSurfaceVariant) }
                items(results, key = { "s" + it.id }) { m ->
                    ListItem(
                        headlineContent = { Text(m.text ?: bodyLabel(m), maxLines = 2, overflow = TextOverflow.Ellipsis) },
                        overlineContent = { Text(if (m.mine) stringResource(CR.string.chat_you) else m.fromName) },
                        supportingContent = { Text(DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(Date(m.sentMs)), style = TelemetryStyle) },
                        modifier = Modifier.clickable { jumpTo(m.id) },
                    )
                }
            }
        } else {
            LazyColumn(Modifier.weight(1f), state = list, contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(msgs, key = { it.id }) { m ->
                    Bubble(
                        m, hueOf(state, m.from), highlighted = m.id == highlight,
                        onLongPress = { actionsFor = m },
                        onToggleReaction = { e -> engine.toggleReaction(m, e) },
                        onShowHistory = { scope.launch { historyFor = m to engine.editHistory(m.id) } },
                        onQuote = { m.reply?.let { jumpTo(it.id) } },
                    )
                }
            }
        }
        // composer chips: replying to / editing
        val chip = editing?.let { stringResource(R.string.chat_editing) to (it.text ?: "") }
            ?: replyTo?.let { stringResource(R.string.chat_replying_to, if (it.mine) stringResource(CR.string.chat_you) else it.fromName) to (it.text ?: bodyLabel(it)) }
        if (chip != null) {
            Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp).clip(RoundedCornerShape(12.dp)).background(MaterialTheme.colorScheme.surfaceContainerHigh).padding(start = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f).padding(vertical = 6.dp)) {
                    Text(chip.first, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
                    Text(chip.second, style = MaterialTheme.typography.bodySmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                IconButton({ if (editing != null) { editing = null; text = "" } else replyTo = null }) { Icon(Icons.Rounded.Close, stringResource(R.string.chat_cancel_compose)) }
            }
        }
        // '@' mention picker
        if (mentionQuery != null && editing == null) {
            val q = mentionQuery.lowercase()
            val allToken = stringResource(R.string.chat_mention_all_token)
            val options = buildList {
                if (g.isCreator && allToken.startsWith(q)) add(Mention(allToken, null))
                others.filter { it.name.lowercase().startsWith(q) }.forEach { add(Mention(it.name, it.node.toHex())) }
            }
            if (options.isNotEmpty()) {
                Surface(Modifier.fillMaxWidth().padding(horizontal = 12.dp).heightIn(max = 200.dp), shape = RoundedCornerShape(12.dp), tonalElevation = 3.dp) {
                    LazyColumn {
                        items(options, key = { it.node ?: "all" }) { o ->
                            val allDesc: (@Composable () -> Unit)? = if (o.node == null) { { Text(stringResource(R.string.chat_mention_all_desc)) } } else null
                            ListItem(
                                headlineContent = { Text("@" + o.name) },
                                supportingContent = allDesc,
                                modifier = Modifier.clickable {
                                    text = text.dropLast(mentionQuery.length + 1) + "@" + o.name + " "
                                    mentions += o
                                },
                            )
                        }
                    }
                }
            }
        }
        Row(Modifier.fillMaxWidth().padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(text, { text = it }, Modifier.weight(1f), placeholder = { Text(stringResource(R.string.chat_hint)) }, shape = RoundedCornerShape(22.dp), maxLines = 4)
            Spacer(Modifier.width(8.dp))
            if (text.isBlank() && editing == null) {
                Box(
                    Modifier.size(52.dp).clip(RoundedCornerShape(18.dp)).background(if (recording) MaterialTheme.titi.transmit else MaterialTheme.colorScheme.surfaceContainerHigh)
                        .pointerInput(Unit) {
                            awaitEachGesture {
                                awaitFirstDown()
                                if (!recorder.start()) {
                                    android.widget.Toast.makeText(ctx, CR.string.error_mic, android.widget.Toast.LENGTH_SHORT).show()
                                    return@awaitEachGesture
                                }
                                recording = true
                                var cancelled = false
                                try {
                                    // hold until the finger lifts (finger drift must not end the recording)
                                    while (true) {
                                        val ev = awaitPointerEvent()
                                        if (ev.changes.all { !it.pressed }) break
                                    }
                                } catch (e: kotlinx.coroutines.CancellationException) { cancelled = true; throw e } finally {
                                    recording = false
                                    val r = recorder.stop()
                                    if (!cancelled && r != null) scope.launch {
                                        recorder.encode(r.first)?.let { engine.sendVoiceNote(gid, uniffi.titi_ffi.FfiProfile.STD, r.second, it) }
                                    }
                                }
                            }
                        },
                    contentAlignment = Alignment.Center,
                ) { Icon(Icons.Rounded.Mic, stringResource(R.string.chat_voice_note), tint = if (recording) MaterialTheme.colorScheme.background else MaterialTheme.colorScheme.onSurface) }
            } else {
                IconButton(::send, Modifier.size(52.dp)) { Icon(Icons.AutoMirrored.Rounded.Send, stringResource(if (editing != null) R.string.chat_save_edit else R.string.chat_send), tint = MaterialTheme.colorScheme.primary) }
            }
        }
    }

    // ---- long-press action sheet ----
    actionsFor?.let { m ->
        ModalBottomSheet(onDismissRequest = { actionsFor = null }) {
            if (!m.deleted) {
                Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                    ChatRules.quickReactions.forEach { e ->
                        val mine = m.reactions.any { it.mine && it.emoji == e }
                        val cd = stringResource(CR.string.cd_react_with, e)
                        Box(
                            Modifier.size(44.dp).clip(CircleShape).background(if (mine) MaterialTheme.colorScheme.primary.copy(alpha = 0.25f) else MaterialTheme.colorScheme.surfaceContainerHigh)
                                .clickable { engine.toggleReaction(m, e); actionsFor = null }.semantics { contentDescription = cd },
                            contentAlignment = Alignment.Center,
                        ) { Text(e, style = MaterialTheme.typography.titleLarge) }
                    }
                    TextButton({ moreEmojiFor = m; actionsFor = null }) { Text(stringResource(R.string.chat_react_more)) }
                }
                HorizontalDivider()
                ActionRow(stringResource(CR.string.chat_action_reply)) { replyTo = m; editing = null; actionsFor = null }
                if (m.canEdit()) ActionRow(stringResource(R.string.chat_action_edit)) { editing = m; replyTo = null; text = m.text.orEmpty(); actionsFor = null }
                if (m.canDelete()) ActionRow(stringResource(R.string.chat_action_delete)) { deleteFor = m; actionsFor = null }
                if (state.groups.size > 1) ActionRow(stringResource(R.string.chat_action_forward)) { forwardFor = m; actionsFor = null }
                m.text?.let { t ->
                    ActionRow(stringResource(R.string.copy)) {
                        ctx.getSystemService(ClipboardManager::class.java)?.setPrimaryClip(ClipData.newPlainText(g.name, t))
                        actionsFor = null
                    }
                }
            }
            Spacer(Modifier.size(24.dp))
        }
    }
    moreEmojiFor?.let { m ->
        var emoji by remember(m.id) { mutableStateOf("") }
        AlertDialog(
            onDismissRequest = { moreEmojiFor = null },
            title = { Text(stringResource(R.string.chat_react_more_title)) },
            text = { OutlinedTextField(emoji, { if (it.length <= 16) emoji = it }, singleLine = true, placeholder = { Text(stringResource(R.string.chat_react_more_hint)) }) },
            confirmButton = { TextButton({ if (emoji.isNotBlank()) engine.react(gid, m.id, emoji.trim()); moreEmojiFor = null }, enabled = emoji.isNotBlank()) { Text(stringResource(CR.string.chat_action_react)) } },
            dismissButton = { TextButton({ moreEmojiFor = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
    deleteFor?.let { m ->
        AlertDialog(
            onDismissRequest = { deleteFor = null },
            title = { Text(stringResource(R.string.chat_delete_title)) },
            text = { Text(stringResource(R.string.chat_delete_body)) },
            confirmButton = { TextButton({ engine.deleteMessage(gid, m.id); deleteFor = null }) { Text(stringResource(R.string.chat_action_delete), color = MaterialTheme.titi.emergency) } },
            dismissButton = { TextButton({ deleteFor = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
    forwardFor?.let { m ->
        AlertDialog(
            onDismissRequest = { forwardFor = null },
            title = { Text(stringResource(R.string.chat_forward_title)) },
            text = {
                LazyColumn(Modifier.heightIn(max = 320.dp)) {
                    items(state.groups.filter { it.id != gid }, key = { it.id }) { t ->
                        ListItem(headlineContent = { Text(t.name) }, modifier = Modifier.clickable { engine.forward(gid, m.id, t.id); forwardFor = null })
                    }
                }
            },
            confirmButton = {},
            dismissButton = { TextButton({ forwardFor = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
    historyFor?.let { (_, revs) ->
        AlertDialog(
            onDismissRequest = { historyFor = null },
            title = { Text(stringResource(R.string.chat_edit_history_title)) },
            text = {
                LazyColumn(Modifier.heightIn(max = 360.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    items(revs) { r ->
                        Column {
                            Text(DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT).format(Date(r.sentMs)), style = TelemetryStyle, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            Text(r.text, style = MaterialTheme.typography.bodyMedium)
                        }
                    }
                }
            },
            confirmButton = { TextButton({ historyFor = null }) { Text(stringResource(R.string.close)) } },
        )
    }
}

@Composable
private fun ActionRow(label: String, onClick: () -> Unit) {
    ListItem(headlineContent = { Text(label) }, modifier = Modifier.clickable(onClick = onClick))
}

private fun hueOf(state: RadioState, node: String): Int = state.peers[node]?.hue ?: state.active?.members?.firstOrNull { it.node.toHex() == node }?.hue?.toInt() ?: 200

@Composable
private fun bodyLabel(m: ChatMessage): String = when (val b = m.body) {
    null -> stringResource(CR.string.chat_deleted)
    is FfiMessageBody.Text -> b.text
    is FfiMessageBody.VoiceNote -> stringResource(CR.string.chat_quote_voice)
    is FfiMessageBody.Location -> stringResource(CR.string.chat_quote_location)
    is FfiMessageBody.Sos -> stringResource(CR.string.chat_quote_sos)
}

@OptIn(ExperimentalFoundationApi::class, ExperimentalLayoutApi::class)
@Composable
private fun Bubble(
    m: ChatMessage,
    hue: Int,
    highlighted: Boolean,
    onLongPress: () -> Unit,
    onToggleReaction: (String) -> Unit,
    onShowHistory: () -> Unit,
    onQuote: () -> Unit,
) {
    val ctx = LocalContext.current
    val res = LocalResources.current
    val time = remember(m.sentMs) { DateFormat.getTimeInstance(DateFormat.SHORT).format(Date(m.sentMs)) }
    val accent = MaterialTheme.colorScheme.primary
    Row(Modifier.fillMaxWidth(), horizontalArrangement = if (m.mine) Arrangement.End else Arrangement.Start, verticalAlignment = Alignment.Bottom) {
        if (!m.mine) { Avatar(m.fromName, hue, size = 28.dp); Spacer(Modifier.width(8.dp)) }
        Column(horizontalAlignment = if (m.mine) Alignment.End else Alignment.Start) {
            val bg = when {
                highlighted -> accent.copy(alpha = 0.40f)
                m.deleted -> MaterialTheme.colorScheme.surfaceContainerLow
                m.mine -> accent.copy(alpha = 0.22f)
                else -> MaterialTheme.colorScheme.surfaceContainer
            }
            val shape = RoundedCornerShape(18.dp)
            Column(
                Modifier.widthIn(max = 300.dp).clip(shape).background(bg)
                    .then(if (m.mentionsMe && !m.deleted) Modifier.border(2.dp, accent, shape) else Modifier)
                    .combinedClickable(onClick = {
                        val b = m.body
                        when (b) {
                            is FfiMessageBody.VoiceNote -> ro.titi.app.audio.VoiceNotePlayer.play(b.opusPackets, b.profile)
                            is FfiMessageBody.Location -> openMap(ctx, b.latE7, b.lonE7)
                            is FfiMessageBody.Sos -> openMap(ctx, b.latE7, b.lonE7)
                            else -> Unit
                        }
                    }, onLongClick = onLongPress, onLongClickLabel = stringResource(R.string.cd_message_actions))
                    .padding(horizontal = 14.dp, vertical = 10.dp),
            ) {
                if (!m.mine) Text(m.fromName, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                if (m.mentionsMe && !m.deleted) Text(stringResource(CR.string.chat_mentions_you), style = MaterialTheme.typography.labelSmall, color = accent)
                ChatText.forwarded(res, m.forwarded)?.let { if (!m.deleted) Text(it, style = MaterialTheme.typography.labelSmall, fontStyle = FontStyle.Italic, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                m.reply?.takeIf { !m.deleted }?.let { r ->
                    val who = ChatText.replySender(res, r)
                    val preview = ChatText.replyPreview(res, r)
                    val cd = stringResource(CR.string.cd_reply_quote, who, preview)
                    Row(
                        Modifier.padding(vertical = 4.dp).clip(RoundedCornerShape(10.dp)).background(MaterialTheme.colorScheme.surfaceContainerHighest.copy(alpha = 0.7f))
                            .clickable(onClick = onQuote).semantics { contentDescription = cd }.padding(start = 0.dp),
                    ) {
                        Box(Modifier.width(3.dp).heightIn(min = 36.dp).background(accent))
                        Column(Modifier.padding(horizontal = 8.dp, vertical = 4.dp)) {
                            Text(who, style = MaterialTheme.typography.labelSmall, color = accent)
                            Text(
                                preview, style = MaterialTheme.typography.bodySmall, maxLines = 2, overflow = TextOverflow.Ellipsis,
                                fontStyle = if (r.state == uniffi.titi_ffi.FfiReplyState.PRESENT) FontStyle.Normal else FontStyle.Italic,
                            )
                        }
                    }
                }
                when (val b = m.body) {
                    null -> Text(stringResource(CR.string.chat_deleted), style = MaterialTheme.typography.bodyMedium, fontStyle = FontStyle.Italic, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    is FfiMessageBody.Text -> Text(b.text, style = MaterialTheme.typography.bodyLarge)
                    is FfiMessageBody.VoiceNote -> Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Rounded.PlayArrow, null, tint = accent); Spacer(Modifier.width(6.dp))
                        Text("${b.durationMs / 1000u}s", style = MaterialTheme.typography.bodyLarge)
                    }
                    is FfiMessageBody.Location -> Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Rounded.LocationOn, null, tint = MaterialTheme.titi.receive); Spacer(Modifier.width(6.dp))
                        Text(stringResource(R.string.chat_location, m.fromName), style = MaterialTheme.typography.bodyMedium)
                    }
                    is FfiMessageBody.Sos -> Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Rounded.Emergency, null, tint = MaterialTheme.titi.emergency); Spacer(Modifier.width(6.dp))
                        Text(if (b.cancelled) stringResource(R.string.chat_sos_cancelled, m.fromName) else stringResource(R.string.chat_sos, m.fromName), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.titi.emergency)
                    }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                    if (m.edited > 0 && !m.deleted) {
                        val cd = stringResource(R.string.cd_edit_history)
                        Text(
                            stringResource(CR.string.chat_edited), style = TelemetryStyle, fontStyle = FontStyle.Italic, color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.clickable(onClick = onShowHistory).semantics { contentDescription = cd },
                        )
                    }
                    Text(time, style = TelemetryStyle, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    if (m.mine && !m.deleted) Text(if (m.acked) "✓✓" else "✓", style = TelemetryStyle, color = if (m.acked) MaterialTheme.titi.receive else MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            if (m.reactions.isNotEmpty() && !m.deleted) {
                FlowRow(Modifier.widthIn(max = 300.dp).padding(top = 2.dp), horizontalArrangement = Arrangement.spacedBy(4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    m.reactions.forEach { r ->
                        val cd = ChatText.reaction(res, r)
                        Row(
                            Modifier.clip(CircleShape)
                                .background(if (r.mine) accent.copy(alpha = 0.28f) else MaterialTheme.colorScheme.surfaceContainerHigh)
                                .then(if (r.mine) Modifier.border(1.dp, accent, CircleShape) else Modifier)
                                .clickable { onToggleReaction(r.emoji) }
                                .semantics { contentDescription = cd }
                                .padding(horizontal = 8.dp, vertical = 3.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(r.emoji, style = MaterialTheme.typography.bodyMedium)
                            if (r.count > 1) { Spacer(Modifier.width(3.dp)); Text("${r.count}", style = TelemetryStyle) }
                        }
                    }
                }
            }
        }
    }
}

private fun openMap(ctx: android.content.Context, latE7: Long, lonE7: Long) {
    val lat = latE7 / 1e7; val lon = lonE7 / 1e7
    runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, "geo:$lat,$lon?q=$lat,$lon".toUri())) }
}
