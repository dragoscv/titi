package ro.titi.tv.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.focusGroup
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.Groups
import androidx.compose.material.icons.rounded.Login
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.MicOff
import androidx.compose.material.icons.rounded.QrCode2
import androidx.compose.material.icons.rounded.Radio
import androidx.compose.material.icons.rounded.Sms
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.PowerSettingsNew
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
import androidx.compose.ui.draw.scale
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusRestorer
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Icon
import androidx.tv.material3.Surface
import androidx.tv.material3.Text
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.flow.merge
import kotlinx.coroutines.flow.map
import ro.titi.app.core.EngineHost
import ro.titi.app.core.FloorState
import ro.titi.app.core.GroupState
import ro.titi.app.core.Invite
import ro.titi.app.core.Toast
import ro.titi.app.core.toHex
import ro.titi.app.data.Prefs
import ro.titi.tv.MainActivity
import ro.titi.tv.R
import uniffi.titi_ffi.FfiMessageBody

private enum class Sheet { None, Invite, Join, Settings }

/** Overscan-safe margins for a 960×540 dp canvas. */
private val SafeH = 48.dp
private val SafeV = 27.dp

@Composable
fun TvRoot(engine: EngineHost, prefs: Prefs, activity: MainActivity) {
    val state by engine.state.collectAsState()
    val settings by prefs.settings.collectAsState(initial = null)
    val micUsable by activity.micUsable.collectAsState()
    val res = androidx.compose.ui.platform.LocalResources.current
    var sheet by remember { mutableStateOf(Sheet.None) }
    var toast by remember { mutableStateOf<String?>(null) }

    LaunchedEffect(Unit) {
        merge(
            engine.toasts.map { t -> when (t) { is Toast.Text -> if (t.arg != null) res.getString(t.res, t.arg) else res.getString(t.res); is Toast.Raw -> t.text } },
            activity.hints.map { res.getString(it) },
        ).collect { toast = it }
    }
    LaunchedEffect(toast) { if (toast != null) { delay(3500); toast = null } }

    val groups = state.groups
    val sel = state.active ?: groups.firstOrNull()
    LaunchedEffect(sel?.id, state.activeGroup) { if (sel != null && sel.id != state.activeGroup) engine.setActiveGroup(sel.id) }

    val railFirst = remember { FocusRequester() }
    val talkReq = remember { FocusRequester() }
    var stageFocused by remember { mutableStateOf(false) }
    LaunchedEffect(settings != null) { if (settings != null) runCatching { railFirst.requestFocus() } }

    val invite = state.invites.firstOrNull()
    BackHandler(enabled = invite != null && sheet == Sheet.None) { invite?.let { engine.declineInvite(it) } }
    BackHandler(enabled = invite == null && sheet == Sheet.None && stageFocused) { runCatching { railFirst.requestFocus() } }

    Box(
        Modifier
            .fillMaxSize()
            .background(Tv.Graphite)
            .onPreviewKeyEvent { e ->
                // an invite banner owns OK until answered
                if (invite != null && sheet == Sheet.None && isOkKey(e.nativeKeyEvent.keyCode)) {
                    if (e.type == KeyEventType.KeyDown && e.nativeKeyEvent.repeatCount == 0) engine.acceptInvite(invite)
                    true
                } else false
            },
    ) {
        Row(Modifier.fillMaxSize().padding(horizontal = SafeH, vertical = SafeV)) {
            Rail(
                name = settings?.name.orEmpty(),
                groups = groups,
                selectedId = sel?.id,
                firstReq = railFirst,
                onFocusGroup = { if (it != state.activeGroup) engine.setActiveGroup(it) },
                onTalkDown = activity::talkDown,
                onTalkUp = activity::talkUp,
                onJoin = { sheet = Sheet.Join },
                onSettings = { sheet = Sheet.Settings },
                onCreate = { engine.createGroup(res.getString(R.string.create_default_name)) },
                onRight = { if (sel != null) runCatching { talkReq.requestFocus() } },
            )
            Spacer(Modifier.width(24.dp))
            Column(Modifier.weight(1f).fillMaxHeight().onFocusChanged { stageFocused = it.hasFocus }) {
                if (sel != null) Stage(engine, sel, state.nodeId, state.muted, micUsable, talkReq, activity, onInvite = { sheet = Sheet.Invite })
                else Empty()
            }
        }

        AnimatedVisibility(invite != null, Modifier.align(Alignment.TopCenter).padding(top = SafeV), enter = slideInVertically { -it } + fadeIn(), exit = slideOutVertically { -it } + fadeOut()) {
            invite?.let { InviteBanner(it) }
        }
        AnimatedVisibility(toast != null, Modifier.align(Alignment.BottomCenter).padding(bottom = SafeV), enter = slideInVertically { it } + fadeIn(), exit = slideOutVertically { it } + fadeOut()) {
            Text(toast.orEmpty(), Modifier.background(Tv.Elevated, CircleShape).padding(horizontal = 20.dp, vertical = 10.dp), color = Tv.Ink, fontSize = 14.sp)
        }
    }

    when (sheet) {
        Sheet.Invite -> sel?.let { g -> SheetDialog(onDismiss = { sheet = Sheet.None }) { InvitePanel(engine, g.id, g.name) } }
        Sheet.Join -> SheetDialog(onDismiss = { sheet = Sheet.None }) {
            JoinPanel(onJoin = { code -> engine.joinByCode(code); toast = res.getString(R.string.join_looking); sheet = Sheet.None })
        }
        Sheet.Settings -> settings?.let { s -> SheetDialog(onDismiss = { sheet = Sheet.None }) { SettingsPanel(prefs, engine, s, onQuit = activity::quit) } }
        Sheet.None -> Unit
    }
}

// ---- rail ------------------------------------------------------------------

@Composable
private fun Rail(
    name: String,
    groups: List<GroupState>,
    selectedId: String?,
    firstReq: FocusRequester,
    onFocusGroup: (String) -> Unit,
    onTalkDown: () -> Unit,
    onTalkUp: () -> Unit,
    onJoin: () -> Unit,
    onSettings: () -> Unit,
    onCreate: () -> Unit,
    onRight: () -> Unit,
) {
    Column(Modifier.width(260.dp).fillMaxHeight()) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(bottom = 16.dp)) {
            Box(Modifier.size(32.dp).background(Tv.Amber, RoundedCornerShape(11.dp)), contentAlignment = Alignment.Center) {
                Icon(Icons.Rounded.Mic, null, tint = Tv.Graphite, modifier = Modifier.size(18.dp))
            }
            Spacer(Modifier.width(10.dp))
            Column {
                Text(stringResource(R.string.app_name), color = Tv.Ink, fontSize = 20.sp, fontWeight = FontWeight.ExtraBold)
                Text(name, color = Tv.Muted, fontSize = 11.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
            }
        }
        LazyColumn(
            Modifier
                .weight(1f)
                .focusRestorer(firstReq)
                .focusGroup()
                .onPreviewKeyEvent { e ->
                    if (e.type == KeyEventType.KeyDown && e.nativeKeyEvent.keyCode == android.view.KeyEvent.KEYCODE_DPAD_RIGHT) { onRight(); true } else false
                },
            verticalArrangement = Arrangement.spacedBy(8.dp),
            contentPadding = androidx.compose.foundation.layout.PaddingValues(vertical = 4.dp, horizontal = 6.dp),
        ) {
            itemsIndexed(groups, key = { _, g -> g.id }) { i, g ->
                GroupRow(
                    g = g,
                    selected = g.id == selectedId,
                    modifier = (if (i == 0) Modifier.focusRequester(firstReq) else Modifier)
                        .onFocusChanged { if (it.isFocused) onFocusGroup(g.id) }
                        .holdToTalk(onTalkDown, onTalkUp),
                )
            }
            item(key = "join") {
                RailAction(Icons.Rounded.Login, stringResource(R.string.join_group), onJoin, if (groups.isEmpty()) Modifier.focusRequester(firstReq) else Modifier)
            }
            item(key = "create") { RailAction(Icons.Rounded.Add, stringResource(R.string.create_group), onCreate) }
            item(key = "settings") { RailAction(Icons.Rounded.Settings, stringResource(R.string.settings), onSettings) }
        }
        Text(stringResource(R.string.rail_hint), color = Tv.Muted, fontSize = 9.sp, modifier = Modifier.padding(top = 8.dp))
    }
}

@Composable
private fun railColors(selected: Boolean) = ClickableSurfaceDefaults.colors(
    containerColor = if (selected) Tv.Elevated else Tv.Surface,
    contentColor = Tv.Ink,
    focusedContainerColor = Tv.Elevated,
    focusedContentColor = Tv.Ink,
    pressedContainerColor = Tv.Elevated,
)

@Composable
private fun GroupRow(g: GroupState, selected: Boolean, modifier: Modifier) {
    val live = g.floor == FloorState.Busy || g.floor == FloorState.Talking
    Surface(
        onClick = {},
        modifier = modifier.fillMaxWidth(),
        shape = tileShape(),
        colors = railColors(selected),
        scale = ClickableSurfaceDefaults.scale(focusedScale = 1.04f),
        border = amberFocusBorder(),
    ) {
        Row(Modifier.padding(horizontal = 12.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier.size(32.dp).background(if (live) Tv.Teal else Tv.Amber.copy(alpha = 0.2f), RoundedCornerShape(10.dp)),
                contentAlignment = Alignment.Center,
            ) { Text(g.name.firstOrNull()?.uppercase() ?: "?", color = if (live) Tv.Graphite else Tv.Amber, fontSize = 15.sp, fontWeight = FontWeight.Bold) }
            Spacer(Modifier.width(10.dp))
            Column(Modifier.weight(1f)) {
                Text(g.name, fontSize = 14.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis, color = Tv.Ink)
                Text(
                    if (live && g.talkerName != null) stringResource(R.string.is_talking, g.talkerName!!) else pluralStringResource(R.plurals.members_count, g.members.size, g.members.size),
                    fontSize = 10.sp, color = if (live) Tv.Teal else Tv.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis,
                )
            }
            if (g.unread > 0) Text("${g.unread}", Modifier.background(Tv.Amber, CircleShape).padding(horizontal = 6.dp, vertical = 2.dp), color = Tv.Graphite, fontSize = 10.sp, fontWeight = FontWeight.Bold)
        }
    }
}

@Composable
private fun RailAction(icon: ImageVector, label: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
    Surface(
        onClick = onClick,
        modifier = modifier.fillMaxWidth(),
        shape = tileShape(),
        colors = ClickableSurfaceDefaults.colors(containerColor = Tv.Surface, contentColor = Tv.Muted, focusedContainerColor = Tv.Elevated, focusedContentColor = Tv.Ink),
        scale = ClickableSurfaceDefaults.scale(focusedScale = 1.04f),
        border = amberFocusBorder(),
    ) {
        Row(Modifier.padding(horizontal = 12.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(icon, null, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(10.dp))
            Text(label, fontSize = 13.sp)
        }
    }
}

// ---- stage -----------------------------------------------------------------

@Composable
private fun Stage(
    engine: EngineHost,
    g: GroupState,
    myNode: String,
    muted: Boolean,
    micUsable: Boolean,
    talkReq: FocusRequester,
    activity: MainActivity,
    onInvite: () -> Unit,
) {
    val level by engine.level.collectAsState()
    val messages by engine.messages.collectAsState()
    val recent = remember(messages, g.id) { messages.filter { it.group == g.id }.takeLast(4) }
    val me = g.floor == FloorState.Talking
    val busy = g.floor == FloorState.Busy || me
    val talker = g.members.firstOrNull { it.node.toHex() == g.talkerNode }

    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom) {
        Column(Modifier.weight(1f)) {
            Text(stringResource(R.string.channel).uppercase(), color = Tv.Muted, fontSize = 10.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 2.sp)
            Text(g.name, color = Tv.Ink, fontSize = 32.sp, fontWeight = FontWeight.ExtraBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Row(Modifier.background(Tv.Surface, CircleShape).padding(horizontal = 12.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Rounded.Groups, null, tint = Tv.Muted, modifier = Modifier.size(14.dp))
            Spacer(Modifier.width(6.dp))
            Text("${g.members.size} · ${g.link?.name?.lowercase() ?: stringResource(R.string.connecting)}", color = Tv.Muted, fontSize = 12.sp)
        }
    }

    Box(
        Modifier.padding(top = 12.dp).fillMaxWidth().fillMaxHeight(0.76f).clip(RoundedCornerShape(24.dp)).background(Tv.Surface),
        contentAlignment = Alignment.Center,
    ) {
        AnimatedContent(
            targetState = if (busy) "t" + (g.talkerNode ?: if (me) "me" else "") else "idle",
            transitionSpec = { (fadeIn() + scaleIn(initialScale = 0.9f)) togetherWith (fadeOut() + scaleOut(targetScale = 1.05f)) },
            label = "stage",
        ) { key ->
            if (key != "idle") {
                val lv = ((level + 50f) / 50f).coerceIn(0f, 1f)
                val halo by animateFloatAsState(1f + lv * 0.35f, spring(dampingRatio = 0.55f, stiffness = Spring.StiffnessMedium), label = "halo")
                val color = if (me) Tv.Amber else hueColor(talker?.hue?.toInt() ?: 170)
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Box(contentAlignment = Alignment.Center) {
                        Box(Modifier.size(158.dp).scale(halo).background(color.copy(alpha = 0.18f), CircleShape))
                        Avatar(if (me) stringResource(R.string.you) else g.talkerName ?: "?", if (me) 40 else talker?.hue?.toInt() ?: 170, 130.dp)
                    }
                    Spacer(Modifier.height(24.dp))
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Rounded.GraphicEq, null, tint = if (me) Tv.Amber else Tv.Teal, modifier = Modifier.size(28.dp))
                        Spacer(Modifier.width(8.dp))
                        Text(
                            if (me) stringResource(R.string.you_are_talking) else stringResource(R.string.is_talking, g.talkerName ?: "?"),
                            color = if (me) Tv.Amber else Tv.Teal, fontSize = 28.sp, fontWeight = FontWeight.Bold,
                        )
                    }
                }
            } else {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Row(horizontalArrangement = Arrangement.spacedBy((-12).dp)) {
                        g.members.take(6).forEach { m ->
                            val mine = m.node.toHex() == myNode
                            Avatar(if (mine) stringResource(R.string.tv_short) else m.name, m.hue.toInt(), 70.dp, ring = Tv.Surface)
                        }
                    }
                    Spacer(Modifier.height(24.dp))
                    Text(
                        when {
                            !g.fullDuplex -> stringResource(R.string.channel_free)
                            muted -> stringResource(R.string.open_mic_muted)
                            else -> stringResource(R.string.open_mic_live)
                        },
                        color = Tv.Muted, fontSize = 22.sp, fontWeight = FontWeight.SemiBold,
                    )
                }
            }
        }
        if (recent.isNotEmpty()) {
            Column(Modifier.align(Alignment.BottomStart).padding(16.dp).width(320.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                recent.forEach { m ->
                    Row(Modifier.background(Tv.Elevated, RoundedCornerShape(11.dp)).padding(horizontal = 10.dp, vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Rounded.Sms, null, tint = Tv.Muted, modifier = Modifier.size(12.dp))
                        Spacer(Modifier.width(6.dp))
                        Text(m.fromName, color = Tv.Ink, fontSize = 12.sp, fontWeight = FontWeight.Bold)
                        Spacer(Modifier.width(6.dp))
                        Text(bodyText(m.body), color = Tv.Ink, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
    }

    Row(Modifier.padding(top = 14.dp).fillMaxWidth().focusRestorer(talkReq).focusGroup(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        val talkLabel = when {
            !micUsable -> stringResource(R.string.action_listening)
            g.fullDuplex -> stringResource(R.string.action_open_mic)
            else -> stringResource(R.string.action_hold_to_talk)
        }
        ActionTile(
            icon = if (micUsable) Icons.Rounded.Mic else Icons.Rounded.MicOff,
            label = talkLabel,
            primary = true,
            active = me,
            modifier = Modifier.weight(1.4f).focusRequester(talkReq).holdToTalk(activity::talkDown, activity::talkUp),
            onClick = {},
        )
        ActionTile(Icons.Rounded.QrCode2, stringResource(R.string.action_invite), modifier = Modifier.weight(1f), onClick = onInvite)
        ActionTile(
            Icons.Rounded.Radio,
            if (g.fullDuplex) stringResource(R.string.action_push_to_talk) else stringResource(R.string.action_open_mic),
            modifier = Modifier.weight(1f),
            onClick = { engine.setFullDuplex(g.id, !g.fullDuplex) },
        )
    }
}

@Composable
private fun bodyText(b: FfiMessageBody): String = when (b) {
    is FfiMessageBody.Text -> b.text
    is FfiMessageBody.Sos -> "SOS"
    is FfiMessageBody.VoiceNote -> "🎙 ${b.durationMs.toInt() / 1000}s"
    is FfiMessageBody.Location -> "📍"
}

@Composable
private fun ActionTile(icon: ImageVector, label: String, modifier: Modifier, primary: Boolean = false, active: Boolean = false, onClick: () -> Unit) {
    val bg = when {
        primary && active -> Tv.Amber
        primary -> Tv.Amber.copy(alpha = 0.2f)
        else -> Tv.Elevated
    }
    val fg = when {
        primary && active -> Tv.Graphite
        primary -> Tv.Amber
        else -> Tv.Ink
    }
    Surface(
        onClick = onClick,
        modifier = modifier.height(52.dp),
        shape = tileShape(14.dp),
        colors = ClickableSurfaceDefaults.colors(containerColor = bg, contentColor = fg, focusedContainerColor = bg, focusedContentColor = fg, pressedContainerColor = bg, pressedContentColor = fg),
        scale = ClickableSurfaceDefaults.scale(focusedScale = 1.05f),
        border = amberFocusBorder(14.dp),
    ) {
        Row(Modifier.fillMaxSize().padding(horizontal = 12.dp), horizontalArrangement = Arrangement.Center, verticalAlignment = Alignment.CenterVertically) {
            Icon(icon, null, modifier = Modifier.size(18.dp))
            Spacer(Modifier.width(8.dp))
            Text(label, fontSize = 14.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
    }
}

@Composable
private fun Empty() {
    Column(
        Modifier.fillMaxSize().clip(RoundedCornerShape(24.dp)).background(Tv.Surface).padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Icon(Icons.Rounded.Radio, null, tint = Tv.Amber, modifier = Modifier.size(48.dp))
        Spacer(Modifier.height(16.dp))
        Text(stringResource(R.string.empty_title), color = Tv.Ink, fontSize = 28.sp, fontWeight = FontWeight.ExtraBold, textAlign = androidx.compose.ui.text.style.TextAlign.Center)
        Spacer(Modifier.height(8.dp))
        Text(stringResource(R.string.empty_body), color = Tv.Muted, fontSize = 15.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center, modifier = Modifier.width(460.dp))
    }
}

// ---- overlays --------------------------------------------------------------

@Composable
private fun InviteBanner(inv: Invite) {
    Row(Modifier.background(Tv.Elevated, RoundedCornerShape(16.dp)).padding(horizontal = 20.dp, vertical = 14.dp), verticalAlignment = Alignment.CenterVertically) {
        Avatar(inv.hostName, 200, 36.dp)
        Spacer(Modifier.width(12.dp))
        Column {
            Text(stringResource(R.string.invite_banner_title, inv.hostName), color = Tv.Ink, fontSize = 16.sp, fontWeight = FontWeight.Bold)
            Text(stringResource(R.string.invite_banner_body, inv.name), color = Tv.Muted, fontSize = 13.sp)
        }
    }
}

@Composable
private fun SheetDialog(onDismiss: () -> Unit, content: @Composable () -> Unit) {
    Dialog(onDismissRequest = onDismiss) {
        Column(Modifier.width(450.dp).background(Tv.Surface, RoundedCornerShape(20.dp)).padding(28.dp)) {
            content()
            Text(stringResource(R.string.close_hint), color = Tv.Muted, fontSize = 11.sp, modifier = Modifier.align(Alignment.CenterHorizontally).padding(top = 20.dp))
        }
    }
}

@Composable
private fun InvitePanel(engine: EngineHost, gid: String, name: String) {
    var code by remember { mutableStateOf<Pair<String, Int>?>(null) }
    var link by remember { mutableStateOf<String?>(null) }
    val focus = remember { FocusRequester() }
    LaunchedEffect(gid) {
        link = engine.deepLink(gid)?.replace("titi://j/", "https://titi.app/j/")
        while (true) { code = engine.currentCode(gid); delay(1000) }
    }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.focusRequester(focus).focusGroup()) {
        val l = link
        if (l != null) Image(rememberQr(l), stringResource(R.string.cd_qr), Modifier.size(170.dp).background(androidx.compose.ui.graphics.Color.White, RoundedCornerShape(14.dp)).padding(6.dp))
        else Box(Modifier.size(170.dp).background(Tv.Elevated, RoundedCornerShape(14.dp)))
        Spacer(Modifier.width(24.dp))
        Column {
            Text(stringResource(R.string.invite_join, name), color = Tv.Muted, fontSize = 13.sp)
            Text(code?.first?.replace("-", " ") ?: "…", color = Tv.Amber, fontSize = 26.sp, fontFamily = FontFamily.Monospace, fontWeight = FontWeight.Medium, modifier = Modifier.padding(vertical = 6.dp))
            val left = code?.second ?: 0
            Text(stringResource(R.string.invite_hint, left / 60, left % 60), color = Tv.Muted, fontSize = 12.sp)
        }
    }
}



/** Every TV setting, D-pad only: OK toggles / cycles. */
@Composable
private fun SettingsPanel(prefs: Prefs, engine: EngineHost, s: ro.titi.app.data.Settings, onQuit: () -> Unit) {
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    fun set(f: (ro.titi.app.data.Settings) -> ro.titi.app.data.Settings) { scope.launch { prefs.update(f) } }
    val first = remember { FocusRequester() }
    LaunchedEffect(Unit) { runCatching { first.requestFocus() } }
    val on = stringResource(R.string.on)
    val off = stringResource(R.string.off)
    val qualities = listOf(R.string.quality_auto, R.string.quality_hq, R.string.quality_std, R.string.quality_low)
    val sounds = listOf(R.string.sound_bird, R.string.sound_radio, R.string.sound_minimal)
    Text(stringResource(R.string.settings), color = Tv.Ink, fontSize = 22.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(bottom = 12.dp))
    Column(Modifier.focusGroup(), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        SettingRow(stringResource(R.string.settings_name), s.name, Modifier.focusRequester(first)) {
            // cycle through room names (no keyboard needed); the device name stays the default
            val rooms = listOf(s.name, "Living room TV", "Bedroom TV", "Kitchen TV", "Office TV").distinct()
            val next = rooms[(rooms.indexOf(s.name) + 1) % rooms.size]
            set { it.copy(name = next) }; engine.setDisplayName(next, s.hue)
        }
        SettingRow(stringResource(R.string.settings_keep_running), if (s.keepRunning) on else off) { set { it.copy(keepRunning = !it.keepRunning) } }
        SettingRow(stringResource(R.string.settings_internet), if (s.useInternet) on else off) { set { it.copy(useInternet = !it.useInternet) } }
        SettingRow(stringResource(R.string.settings_ble), if (s.useBle) on else off) { set { it.copy(useBle = !it.useBle) } }
        SettingRow(stringResource(R.string.settings_quality), stringResource(qualities[s.quality.ordinal])) {
            set { it.copy(quality = ro.titi.app.data.QualityOverride.entries[(it.quality.ordinal + 1) % 4]) }
        }
        SettingRow(stringResource(R.string.settings_sounds), stringResource(sounds[s.soundPack.ordinal])) {
            set { it.copy(soundPack = ro.titi.app.data.SoundPack.entries[(it.soundPack.ordinal + 1) % 3]) }
        }
        SettingRow(stringResource(R.string.settings_quit), "", icon = Icons.Rounded.PowerSettingsNew, onClick = onQuit)
    }
}

@Composable
private fun SettingRow(label: String, value: String, modifier: Modifier = Modifier, icon: ImageVector? = null, onClick: () -> Unit) {
    Surface(
        onClick = onClick,
        modifier = modifier.fillMaxWidth(),
        shape = tileShape(12.dp),
        colors = ClickableSurfaceDefaults.colors(containerColor = Tv.Elevated, contentColor = Tv.Ink, focusedContainerColor = Tv.Amber, focusedContentColor = Tv.Graphite),
        border = amberFocusBorder(12.dp),
    ) {
        Row(Modifier.padding(horizontal = 16.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            if (icon != null) { Icon(icon, null, Modifier.size(18.dp)); Spacer(Modifier.width(10.dp)) }
            Text(label, fontSize = 14.sp, modifier = Modifier.weight(1f))
            Text(value, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
        }
    }
}

@Composable
private fun JoinPanel(onJoin: (String) -> Unit) {
    var text by remember { mutableStateOf("") }
    val focus = remember { FocusRequester() }
    var fieldFocused by remember { mutableStateOf(false) }
    val norm = text.trim().lowercase().replace(Regex("\\s+"), "-")
    // the core owns the code format (word count, checksum digits) — never re-implement it in UI
    val ok = remember(norm) { norm.isNotEmpty() && runCatching { uniffi.titi_ffi.parseInviteCode(norm) }.getOrDefault(false) }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Text(stringResource(R.string.join_title), color = Tv.Ink, fontSize = 22.sp, fontWeight = FontWeight.Bold)
    Text(stringResource(R.string.join_body), color = Tv.Muted, fontSize = 13.sp, modifier = Modifier.padding(top = 4.dp))
    BasicTextField(
        value = text,
        onValueChange = { text = it },
        singleLine = true,
        textStyle = TextStyle(color = Tv.Ink, fontSize = 22.sp, fontFamily = FontFamily.Monospace),
        cursorBrush = SolidColor(Tv.Amber),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { if (ok) onJoin(norm) }),
        modifier = Modifier
            .padding(top = 16.dp)
            .fillMaxWidth()
            .focusRequester(focus)
            .onFocusChanged { fieldFocused = it.isFocused }
            .background(Tv.Graphite, RoundedCornerShape(12.dp))
            .border(2.dp, if (fieldFocused) Tv.Amber else Tv.Outline, RoundedCornerShape(12.dp))
            .padding(horizontal = 16.dp, vertical = 12.dp),
        decorationBox = { inner ->
            if (text.isEmpty()) Text(stringResource(R.string.join_placeholder), color = Tv.Muted, fontSize = 22.sp, fontFamily = FontFamily.Monospace)
            inner()
        },
    )
    Row(Modifier.padding(top = 12.dp).fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(
            if (ok) stringResource(R.string.join_ok) else stringResource(R.string.join_invalid),
            color = if (ok) Tv.Teal else Tv.Muted, fontSize = 12.sp, modifier = Modifier.weight(1f),
        )
        Surface(
            onClick = { if (ok) onJoin(norm) },
            enabled = ok,
            shape = tileShape(12.dp),
            colors = ClickableSurfaceDefaults.colors(containerColor = Tv.Amber.copy(alpha = 0.2f), contentColor = Tv.Amber, focusedContainerColor = Tv.Amber, focusedContentColor = Tv.Graphite),
            border = amberFocusBorder(12.dp),
        ) { Text(stringResource(R.string.join_ok), Modifier.padding(horizontal = 18.dp, vertical = 10.dp), fontSize = 14.sp, fontWeight = FontWeight.SemiBold) }
    }
}
