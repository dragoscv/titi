package ro.titi.app.ui.screens

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.Send
import androidx.compose.material.icons.rounded.Emergency
import androidx.compose.material.icons.rounded.LocationOn
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import ro.titi.app.R
import ro.titi.app.audio.VoiceNoteRecorder
import ro.titi.app.core.ChatMessage
import ro.titi.app.core.EngineHost
import ro.titi.app.core.RadioState
import ro.titi.app.ui.components.Avatar
import ro.titi.app.ui.theme.TelemetryStyle
import ro.titi.app.ui.theme.titi
import uniffi.titi_ffi.FfiMessageBody
import java.text.DateFormat
import kotlinx.coroutines.launch
import java.util.Date

@Composable
fun ChatScreen(engine: EngineHost, state: RadioState, gid: String, onBack: () -> Unit) {
    val g = state.groups.firstOrNull { it.id == gid } ?: run { LaunchedEffect(Unit) { onBack() }; return }
    val all by engine.messages.collectAsState()
    val msgs = all.filter { it.group == gid }
    var text by remember { mutableStateOf("") }
    val list = rememberLazyListState()
    val ctx = LocalContext.current
    val recorder = remember { VoiceNoteRecorder(ctx) }
    var recording by remember { mutableStateOf(false) }
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    androidx.compose.runtime.DisposableEffect(recorder) { onDispose { recorder.stop() } }
    LaunchedEffect(msgs.size) { if (msgs.isNotEmpty()) list.animateScrollToItem(msgs.size - 1) }
    LaunchedEffect(gid) { engine.setActiveGroup(gid) }

    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).safeDrawingPadding().imePadding()) {
        Row(Modifier.fillMaxWidth().padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.back)) }
            Text(g.name, style = MaterialTheme.typography.titleLarge)
        }
        LazyColumn(Modifier.weight(1f), state = list, contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(msgs, key = { it.id }) { m -> Bubble(m, hueOf(state, m.from)) }
        }
        Row(Modifier.fillMaxWidth().padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(text, { text = it }, Modifier.weight(1f), placeholder = { Text(stringResource(R.string.chat_hint)) }, shape = RoundedCornerShape(22.dp), maxLines = 4)
            Spacer(Modifier.width(8.dp))
            if (text.isBlank()) {
                Box(
                    Modifier.size(52.dp).clip(RoundedCornerShape(18.dp)).background(if (recording) MaterialTheme.titi.transmit else MaterialTheme.colorScheme.surfaceContainerHigh)
                        .pointerInput(Unit) {
                            awaitEachGesture {
                                awaitFirstDown()
                                if (!recorder.start()) {
                                    android.widget.Toast.makeText(ctx, ro.titi.client.R.string.error_mic, android.widget.Toast.LENGTH_SHORT).show()
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
                                    val res = recorder.stop()
                                    if (!cancelled && res != null) scope.launch {
                                        recorder.encode(res.first)?.let { engine.sendVoiceNote(gid, uniffi.titi_ffi.FfiProfile.STD, res.second, it) }
                                    }
                                }
                            }
                        },
                    contentAlignment = Alignment.Center,
                ) { Icon(Icons.Rounded.Mic, stringResource(R.string.chat_voice_note), tint = if (recording) MaterialTheme.colorScheme.background else MaterialTheme.colorScheme.onSurface) }
            } else {
                IconButton({ engine.sendText(gid, text.trim()); text = "" }, Modifier.size(52.dp)) { Icon(Icons.AutoMirrored.Rounded.Send, stringResource(R.string.chat_send), tint = MaterialTheme.colorScheme.primary) }
            }
        }
    }
}

private fun hueOf(state: RadioState, node: String): Int = state.peers[node]?.hue ?: state.active?.members?.firstOrNull { it.node.joinToString("") { b -> "%02x".format(b) } == node }?.hue?.toInt() ?: 200

@Composable
private fun Bubble(m: ChatMessage, hue: Int) {
    val ctx = LocalContext.current
    val time = remember(m.sentMs) { DateFormat.getTimeInstance(DateFormat.SHORT).format(Date(m.sentMs)) }
    Row(Modifier.fillMaxWidth(), horizontalArrangement = if (m.mine) Arrangement.End else Arrangement.Start, verticalAlignment = Alignment.Bottom) {
        if (!m.mine) { Avatar(m.fromName, hue, size = 28.dp); Spacer(Modifier.width(8.dp)) }
        Column(
            Modifier.widthIn(max = 300.dp).clip(RoundedCornerShape(18.dp))
                .background(if (m.mine) MaterialTheme.colorScheme.primary.copy(alpha = 0.22f) else MaterialTheme.colorScheme.surfaceContainer)
                .padding(horizontal = 14.dp, vertical = 10.dp),
        ) {
            if (!m.mine) Text(m.fromName, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            when (val b = m.body) {
                is FfiMessageBody.Text -> Text(b.text, style = MaterialTheme.typography.bodyLarge)
                is FfiMessageBody.VoiceNote -> Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.clickable { ro.titi.app.audio.VoiceNotePlayer.play(b.opusPackets, b.profile) }) {
                    Icon(Icons.Rounded.PlayArrow, null, tint = MaterialTheme.colorScheme.primary); Spacer(Modifier.width(6.dp))
                    Text("${b.durationMs / 1000u}s", style = MaterialTheme.typography.bodyLarge)
                }
                is FfiMessageBody.Location -> Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.clickable { openMap(ctx, b.latE7, b.lonE7) }) {
                    Icon(Icons.Rounded.LocationOn, null, tint = MaterialTheme.titi.receive); Spacer(Modifier.width(6.dp))
                    Text(stringResource(R.string.chat_location, m.fromName), style = MaterialTheme.typography.bodyMedium)
                }
                is FfiMessageBody.Sos -> Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.clickable { openMap(ctx, b.latE7, b.lonE7) }) {
                    Icon(Icons.Rounded.Emergency, null, tint = MaterialTheme.titi.emergency); Spacer(Modifier.width(6.dp))
                    Text(if (b.cancelled) stringResource(R.string.chat_sos_cancelled, m.fromName) else stringResource(R.string.chat_sos, m.fromName), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.titi.emergency)
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(time, style = TelemetryStyle, color = MaterialTheme.colorScheme.onSurfaceVariant)
                if (m.mine) Text(if (m.acked) "✓✓" else "✓", style = TelemetryStyle, color = if (m.acked) MaterialTheme.titi.receive else MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

private fun openMap(ctx: android.content.Context, latE7: Long, lonE7: Long) {
    val lat = latE7 / 1e7; val lon = lonE7 / 1e7
    runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse("geo:$lat,$lon?q=$lat,$lon"))) }
}
