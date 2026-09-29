package ro.titi.wear.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.background
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Emergency
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.LocationOn
import androidx.compose.material.icons.rounded.Logout
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.PowerSettingsNew
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.wear.compose.foundation.lazy.TransformingLazyColumn
import androidx.wear.compose.foundation.lazy.TransformingLazyColumnDefaults
import androidx.wear.compose.foundation.lazy.TransformingLazyColumnItemScope
import androidx.wear.compose.foundation.lazy.items
import androidx.wear.compose.foundation.lazy.rememberTransformingLazyColumnState
import androidx.wear.compose.foundation.rotary.RotaryScrollableDefaults
import androidx.wear.compose.material3.AlertDialog
import androidx.wear.compose.material3.AlertDialogDefaults
import androidx.wear.compose.material3.Button
import androidx.wear.compose.material3.ButtonDefaults
import androidx.wear.compose.material3.Card
import androidx.wear.compose.material3.CardDefaults
import androidx.wear.compose.material3.EdgeButton
import androidx.wear.compose.material3.EdgeButtonSize
import androidx.wear.compose.material3.FilledTonalButton
import androidx.wear.compose.material3.Icon
import androidx.wear.compose.material3.ListHeader
import androidx.wear.compose.material3.ListHeaderDefaults
import androidx.wear.compose.material3.MaterialTheme
import androidx.wear.compose.material3.ScreenScaffold
import androidx.wear.compose.material3.SurfaceTransformation
import androidx.wear.compose.material3.SwitchButton
import androidx.wear.compose.material3.Text
import androidx.wear.compose.material3.lazy.rememberTransformationSpec
import androidx.wear.compose.material3.lazy.transformedHeight
import kotlinx.coroutines.launch
import ro.titi.app.core.ChatMessage
import ro.titi.app.core.EngineHost
import ro.titi.app.core.FloorState
import ro.titi.app.core.RadioState
import ro.titi.app.data.Prefs
import ro.titi.app.data.Settings
import ro.titi.wear.R
import uniffi.titi_ffi.FfiMessageBody

// ---- Groups (page 1) ---------------------------------------------------------

@Composable
fun GroupsScreen(engine: EngineHost, state: RadioState, onOpen: () -> Unit, onJoin: () -> Unit) {
    val list = rememberTransformingLazyColumnState()
    val spec = rememberTransformationSpec()
    ScreenScaffold(
        scrollState = list,
        edgeButton = {
            EdgeButton(onClick = onJoin, buttonSize = EdgeButtonSize.Medium) {
                Icon(Icons.Rounded.Add, null); Text(stringResource(R.string.join_code))
            }
        },
    ) { pad ->
        TransformingLazyColumn(
            state = list, contentPadding = pad,
            flingBehavior = TransformingLazyColumnDefaults.snapFlingBehavior(list),
            rotaryScrollableBehavior = RotaryScrollableDefaults.snapBehavior(list),
        ) {
            item {
                ListHeader(
                    Modifier.fillMaxWidth().transformedHeight(this, spec).minimumVerticalContentPadding(ListHeaderDefaults.minimumTopListContentPadding),
                    transformation = SurfaceTransformation(spec),
                ) { Text(stringResource(R.string.groups)) }
            }
            if (state.groups.isEmpty()) {
                item { Text(stringResource(R.string.no_groups_hint), Modifier.padding(12.dp).transformedHeight(this, spec), textAlign = TextAlign.Center, style = MaterialTheme.typography.bodySmall) }
            }
            items(state.groups, key = { it.id }) { g ->
                val active = g.id == state.activeGroup
                Button(
                    onClick = { engine.setActiveGroup(g.id); onOpen() },
                    modifier = Modifier.fillMaxWidth().transformedHeight(this, spec).minimumVerticalContentPadding(ButtonDefaults.minimumVerticalListContentPadding),
                    transformation = SurfaceTransformation(spec),
                    colors = if (active) ButtonDefaults.buttonColors() else ButtonDefaults.filledTonalButtonColors(),
                    icon = { Icon(if (g.floor == FloorState.Busy) Icons.Rounded.GraphicEq else Icons.Rounded.Mic, null) },
                    label = { Text(g.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                    secondaryLabel = {
                        Text(
                            when {
                                g.floor == FloorState.Busy && g.talkerName != null -> g.talkerName!!
                                g.unread > 0 -> "● ${g.unread}"
                                else -> pluralStringResource(R.plurals.members_count, g.members.size, g.members.size)
                            },
                            maxLines = 1,
                        )
                    },
                )
            }
        }
    }
}

// ---- Join by code: 3 word pickers + 2 digits -----------------------------------

@Composable
fun JoinScreen(engine: EngineHost, onDone: () -> Unit) {
    var text by remember { mutableStateOf("") }
    val label = stringResource(R.string.join_code)
    val list = rememberTransformingLazyColumnState()
    val spec = rememberTransformationSpec()
    // Wear's system keyboard / voice input via RemoteInput; the core parses loosely
    // (any separator, diacritics, digits attached), so voice "tigru râu acid 42" works.
    val launcher = androidx.activity.compose.rememberLauncherForActivityResult(androidx.activity.result.contract.ActivityResultContracts.StartActivityForResult()) { r ->
        val results = r.data?.let { android.app.RemoteInput.getResultsFromIntent(it) }
        text = results?.getCharSequence(KEY_CODE)?.toString()?.trim() ?: text
    }
    fun ask() {
        val ri = android.app.RemoteInput.Builder(KEY_CODE).setLabel(label).build()
        val intent = androidx.wear.input.RemoteInputIntentHelper.createActionRemoteInputIntent()
        androidx.wear.input.RemoteInputIntentHelper.putRemoteInputsExtra(intent, listOf(ri))
        runCatching { launcher.launch(intent) }
    }
    ScreenScaffold(
        scrollState = list,
        edgeButton = {
            EdgeButton(onClick = { if (text.isNotBlank()) { engine.joinByCode(text); onDone() } else ask() }, buttonSize = EdgeButtonSize.Medium) {
                Text(stringResource(if (text.isBlank()) R.string.join_code else R.string.join))
            }
        },
    ) { pad ->
        TransformingLazyColumn(state = list, contentPadding = pad) {
            item { ListHeader(Modifier.transformedHeight(this, spec)) { Text(stringResource(R.string.join_code)) } }
            item {
                Card(
                    onClick = ::ask,
                    modifier = Modifier.fillMaxWidth().transformedHeight(this, spec).minimumVerticalContentPadding(CardDefaults.minimumVerticalListContentPadding),
                    transformation = SurfaceTransformation(spec),
                ) {
                    Text(
                        if (text.isBlank()) "tigru-rau-acid-42" else text,
                        style = MaterialTheme.typography.titleMedium,
                        color = if (text.isBlank()) MaterialTheme.colorScheme.onSurfaceVariant else Titi.Amber,
                        textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
        }
    }
}

private const val KEY_CODE = "code"

// ---- Chat (page 2): read, quick replies, voice notes ------------------------------

@Composable
fun ChatScreen(engine: EngineHost, state: RadioState, onRecord: () -> Unit) {
    val g = state.active ?: return
    val all by engine.messages.collectAsState()
    val msgs = remember(all, g.id) { all.filter { it.group == g.id }.takeLast(30).reversed() }
    val list = rememberTransformingLazyColumnState()
    val spec = rememberTransformationSpec()
    val replies = listOf(R.string.reply_ok, R.string.reply_on_my_way, R.string.reply_wait, R.string.reply_where)
    val res = androidx.compose.ui.platform.LocalResources.current
    val haptics = LocalHapticFeedback.current
    ScreenScaffold(
        scrollState = list,
        edgeButton = {
            EdgeButton(onClick = onRecord, buttonSize = EdgeButtonSize.Small) { Icon(Icons.Rounded.Mic, stringResource(R.string.record_voice_note)) }
        },
    ) { pad ->
        TransformingLazyColumn(state = list, contentPadding = pad) {
            item { ListHeader(Modifier.transformedHeight(this, spec)) { Text(stringResource(R.string.chat)) } }
            items(replies) { r ->
                FilledTonalButton(
                    onClick = { engine.sendText(g.id, res.getString(r)); haptics.performHapticFeedback(HapticFeedbackType.Confirm) },
                    modifier = Modifier.fillMaxWidth().transformedHeight(this, spec).minimumVerticalContentPadding(ButtonDefaults.minimumVerticalListContentPadding),
                    transformation = SurfaceTransformation(spec),
                    label = { Text(stringResource(r), maxLines = 1) },
                )
            }
            if (msgs.isEmpty()) item { Text(stringResource(R.string.chat_empty), Modifier.padding(8.dp).transformedHeight(this, spec), style = MaterialTheme.typography.bodySmall) }
            items(msgs, key = { it.id }) { m -> Bubble(m, Modifier.transformedHeight(this, spec), SurfaceTransformation(spec)) }
        }
    }
}

@Composable
private fun TransformingLazyColumnItemScope.Bubble(m: ChatMessage, modifier: Modifier, transformation: SurfaceTransformation) {
    val body = m.body
    val text = when (body) {
        is FfiMessageBody.Text -> body.text
        is FfiMessageBody.VoiceNote -> stringResource(R.string.chat_voice_note, body.durationMs.toInt() / 1000)
        is FfiMessageBody.Location -> stringResource(R.string.chat_location)
        is FfiMessageBody.Sos -> if (body.cancelled) stringResource(R.string.sos_cancelled, m.fromName) else stringResource(R.string.sos_from, m.fromName)
    }
    Card(
        onClick = { if (body is FfiMessageBody.VoiceNote) ro.titi.app.audio.VoiceNotePlayer.play(body.opusPackets, body.profile) },
        modifier = modifier.fillMaxWidth().minimumVerticalContentPadding(CardDefaults.minimumVerticalListContentPadding),
        transformation = transformation,
        colors = if (body is FfiMessageBody.Sos && !body.cancelled) CardDefaults.cardColors(containerColor = Titi.Emergency.copy(alpha = 0.3f)) else if (m.mine) CardDefaults.cardColors(containerColor = Titi.Amber.copy(alpha = 0.18f)) else CardDefaults.cardColors(),
    ) {
        Column {
            if (!m.mine) Text(m.fromName, style = MaterialTheme.typography.labelSmall, color = Titi.Teal)
            Text(text, style = MaterialTheme.typography.bodyMedium)
        }
    }
}

// ---- Voice note recorder (full screen hold) ----------------------------------------

@Composable
fun RecordScreen(engine: EngineHost, state: RadioState, onDone: () -> Unit) {
    val g = state.active ?: run { onDone(); return }
    val ctx = LocalContext.current
    val recorder = remember { ro.titi.app.audio.VoiceNoteRecorder(ctx) }
    val scope = rememberCoroutineScope()
    var recording by remember { mutableStateOf(false) }
    androidx.compose.runtime.DisposableEffect(Unit) { onDispose { recorder.stop() } }
    ScreenScaffold {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            TalkButton(
                floor = if (recording) FloorState.Talking else FloorState.Idle, talker = null, muted = false, fullDuplex = false, levelDbfs = if (recording) -20f else -60f,
                onDown = { recording = recorder.start() },
                onUp = {
                    recording = false
                    recorder.stop()?.let { (pcm, ms) ->
                        scope.launch { recorder.encode(pcm)?.let { engine.sendVoiceNote(g.id, uniffi.titi_ffi.FfiProfile.STD, ms, it) }; onDone() }
                    } ?: onDone()
                },
                onToggleMute = {},
            )
            Text(stringResource(if (recording) R.string.recording else R.string.record_voice_note), Modifier.align(Alignment.BottomCenter).padding(bottom = 28.dp), style = MaterialTheme.typography.labelSmall)
        }
    }
}

// ---- Group actions (page 3): SOS, location, members, settings ------------------------

@Composable
fun ActionsScreen(engine: EngineHost, state: RadioState, prefs: Prefs, settings: Settings, onSettings: () -> Unit, onQuit: () -> Unit) {
    val g = state.active
    val list = rememberTransformingLazyColumnState()
    val spec = rememberTransformationSpec()
    var confirmSos by remember { mutableStateOf(false) }
    var confirmLeave by remember { mutableStateOf(false) }
    val ctx = LocalContext.current
    ScreenScaffold(scrollState = list) { pad ->
        TransformingLazyColumn(state = list, contentPadding = pad) {
            item { ListHeader(Modifier.transformedHeight(this, spec)) { Text(g?.name ?: stringResource(R.string.app_name)) } }
            if (g != null) {
                item {
                    val sosActive = state.sosGroup == g.id
                    Button(
                        onClick = { if (sosActive) engine.cancelSos() else confirmSos = true },
                        modifier = Modifier.fillMaxWidth().transformedHeight(this, spec).minimumVerticalContentPadding(ButtonDefaults.minimumVerticalListContentPadding),
                        transformation = SurfaceTransformation(spec),
                        colors = ButtonDefaults.buttonColors(containerColor = Titi.Emergency, contentColor = Color.White),
                        icon = { Icon(Icons.Rounded.Emergency, null) },
                        label = { Text(stringResource(if (sosActive) R.string.sos_cancel else R.string.sos)) },
                        secondaryLabel = if (sosActive) ({ Text(stringResource(R.string.sos_active)) }) else null,
                    )
                }
                item {
                    FilledTonalButton(
                        onClick = { ro.titi.app.util.Location.lastKnown(ctx) { lat, lon, acc -> engine.sendLocation(g.id, lat, lon, acc, breadcrumb = false) } },
                        modifier = Modifier.fillMaxWidth().transformedHeight(this, spec).minimumVerticalContentPadding(ButtonDefaults.minimumVerticalListContentPadding),
                        transformation = SurfaceTransformation(spec),
                        icon = { Icon(Icons.Rounded.LocationOn, null) },
                        label = { Text(stringResource(R.string.chat_location)) },
                    )
                }
                item { ListHeader(Modifier.transformedHeight(this, spec)) { Text(stringResource(R.string.members)) } }
                items(g.members, key = { it.node.joinToString("") { b -> "%02x".format(b) } }) { m ->
                    val hex = m.node.joinToString("") { b -> "%02x".format(b) }
                    val online = hex == state.nodeId || state.peers[hex] != null
                    Row(
                        Modifier.fillMaxWidth().transformedHeight(this, spec).padding(horizontal = 14.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        Box(Modifier.size(10.dp).clip(CircleShape).background(if (online) Titi.Teal else MaterialTheme.colorScheme.outline))
                        Text(if (hex == state.nodeId) stringResource(R.string.you) else m.name, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
                item {
                    FilledTonalButton(
                        onClick = { confirmLeave = true },
                        modifier = Modifier.fillMaxWidth().transformedHeight(this, spec).minimumVerticalContentPadding(ButtonDefaults.minimumVerticalListContentPadding),
                        transformation = SurfaceTransformation(spec),
                        icon = { Icon(Icons.Rounded.Logout, null) },
                        label = { Text(stringResource(R.string.leave)) },
                    )
                }
            }
            item {
                FilledTonalButton(
                    onClick = onSettings,
                    modifier = Modifier.fillMaxWidth().transformedHeight(this, spec).minimumVerticalContentPadding(ButtonDefaults.minimumVerticalListContentPadding),
                    transformation = SurfaceTransformation(spec),
                    icon = { Icon(Icons.Rounded.Settings, null) },
                    label = { Text(stringResource(R.string.settings)) },
                )
            }
            item { QuitButton(Modifier.transformedHeight(this, spec), SurfaceTransformation(spec), onQuit) }
        }
    }
    if (g != null) {
        AlertDialog(
            visible = confirmSos,
            onDismissRequest = { confirmSos = false },
            icon = { Icon(Icons.Rounded.Emergency, null, tint = Titi.Emergency) },
            title = { Text(stringResource(R.string.sos)) },
            text = { Text(stringResource(R.string.sos_confirm, g.name)) },
            confirmButton = {
                AlertDialogDefaults.ConfirmButton(onClick = {
                    confirmSos = false
                    ro.titi.app.util.Location.lastKnown(ctx, allowNull = true) { lat, lon, _ -> engine.raiseSos(g.id, lat, lon) }
                })
            },
            dismissButton = { AlertDialogDefaults.DismissButton(onClick = { confirmSos = false }) },
        )
        AlertDialog(
            visible = confirmLeave,
            onDismissRequest = { confirmLeave = false },
            title = { Text(stringResource(R.string.leave_confirm, g.name)) },
            confirmButton = { AlertDialogDefaults.ConfirmButton(onClick = { confirmLeave = false; engine.leaveGroup(g.id) }) },
            dismissButton = { AlertDialogDefaults.DismissButton(onClick = { confirmLeave = false }) },
        )
    }
}

@Composable
fun SettingsScreen(prefs: Prefs, settings: Settings, onQuit: () -> Unit) {
    val list = rememberTransformingLazyColumnState()
    val spec = rememberTransformationSpec()
    val scope = rememberCoroutineScope()
    fun set(f: (Settings) -> Settings) { scope.launch { prefs.update(f) } }
    ScreenScaffold(scrollState = list) { pad ->
        TransformingLazyColumn(state = list, contentPadding = pad) {
            item { ListHeader(Modifier.transformedHeight(this, spec)) { Text(stringResource(R.string.settings)) } }
            item { Toggle(stringResource(R.string.settings_keep_running), stringResource(R.string.settings_keep_running_desc), settings.keepRunning, Modifier.transformedHeight(this, spec), SurfaceTransformation(spec)) { v -> set { it.copy(keepRunning = v) } } }
            item {
                val next = ro.titi.app.data.QualityOverride.entries.let { e -> e[(settings.quality.ordinal + 1) % e.size] }
                FilledTonalButton(
                    onClick = { set { it.copy(quality = next) } },
                    modifier = Modifier.fillMaxWidth().transformedHeight(this, spec).minimumVerticalContentPadding(ButtonDefaults.minimumVerticalListContentPadding),
                    transformation = SurfaceTransformation(spec),
                    label = { Text(stringResource(R.string.settings_quality)) },
                    secondaryLabel = { Text(stringResource(when (settings.quality) { ro.titi.app.data.QualityOverride.Auto -> R.string.quality_auto; ro.titi.app.data.QualityOverride.Hq -> R.string.quality_hq; ro.titi.app.data.QualityOverride.Std -> R.string.quality_std; ro.titi.app.data.QualityOverride.Low -> R.string.quality_low })) },
                )
            }
            item { Toggle(stringResource(R.string.settings_battery), null, settings.batterySaver, Modifier.transformedHeight(this, spec), SurfaceTransformation(spec)) { v -> set { it.copy(batterySaver = v) } } }
            item { Toggle(stringResource(R.string.use_wifi), stringResource(R.string.use_wifi_desc), settings.useWifiVoice, Modifier.transformedHeight(this, spec), SurfaceTransformation(spec)) { v -> set { it.copy(useWifiVoice = v) } } }
            item { Toggle(stringResource(R.string.use_internet), null, settings.useInternet, Modifier.transformedHeight(this, spec), SurfaceTransformation(spec)) { v -> set { it.copy(useInternet = v) } } }
            item { Toggle(stringResource(R.string.use_ble), null, settings.useBle, Modifier.transformedHeight(this, spec), SurfaceTransformation(spec)) { v -> set { it.copy(useBle = v) } } }
            item { Toggle(stringResource(R.string.haptics_only), null, settings.hapticsOnly, Modifier.transformedHeight(this, spec), SurfaceTransformation(spec)) { v -> set { it.copy(hapticsOnly = v) } } }
            item { QuitButton(Modifier.transformedHeight(this, spec), SurfaceTransformation(spec), onQuit) }
        }
    }
}

@Composable
private fun TransformingLazyColumnItemScope.QuitButton(modifier: Modifier, transformation: SurfaceTransformation, onQuit: () -> Unit) {
    FilledTonalButton(
        onClick = onQuit,
        modifier = modifier.fillMaxWidth().minimumVerticalContentPadding(ButtonDefaults.minimumVerticalListContentPadding),
        transformation = transformation,
        icon = { Icon(Icons.Rounded.PowerSettingsNew, null) },
        label = { Text(stringResource(R.string.settings_quit)) },
    )
}

@Composable
private fun TransformingLazyColumnItemScope.Toggle(label: String, secondary: String?, checked: Boolean, modifier: Modifier, transformation: SurfaceTransformation, onChange: (Boolean) -> Unit) {
    SwitchButton(
        checked = checked, onCheckedChange = onChange,
        modifier = modifier.fillMaxWidth().minimumVerticalContentPadding(ButtonDefaults.minimumVerticalListContentPadding),
        transformation = transformation,
        label = { Text(label) },
        secondaryLabel = secondary?.let { { Text(it, maxLines = 2) } },
    )
}
