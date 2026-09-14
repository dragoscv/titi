package ro.titi.app.ui.screens

import android.content.Intent
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.Chat
import androidx.compose.material.icons.rounded.Emergency
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.PersonAdd
import androidx.compose.material.icons.rounded.Share
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import ro.titi.app.R
import ro.titi.app.core.EngineHost
import ro.titi.app.core.FloorState
import ro.titi.app.core.RadioState
import ro.titi.app.ui.components.Avatar
import ro.titi.app.ui.components.LinkChip
import ro.titi.app.ui.components.TalkButton
import ro.titi.app.ui.rememberQr
import ro.titi.app.ui.theme.CodeStyle
import ro.titi.app.ui.theme.TelemetryStyle
import ro.titi.app.ui.theme.titi

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun GroupScreen(engine: EngineHost, state: RadioState, id: String, onBack: () -> Unit, onChat: () -> Unit) {
    val g = state.groups.firstOrNull { it.id == id } ?: run { LaunchedEffect(Unit) { onBack() }; return }
    LaunchedEffect(id) { if (state.activeGroup != id) engine.setActiveGroup(id) }
    var showInvite by remember { mutableStateOf(false) }
    var showMenu by remember { mutableStateOf(false) }
    var confirmLeave by remember { mutableStateOf(false) }
    var confirmSos by remember { mutableStateOf(false) }
    val ctx = LocalContext.current

    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).safeDrawingPadding()) {
        // top bar
        Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.back)) }
            Column(Modifier.weight(1f)) {
                Text(g.name, style = MaterialTheme.typography.titleLarge)
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(if (g.members.size == 1) stringResource(R.string.home_member_one) else stringResource(R.string.home_members, g.members.size), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    LinkChip(g.link, bars = state.peers.values.filter { it.inGroup }.maxOfOrNull { it.bars } ?: 0, hops = state.peers.values.filter { it.inGroup }.maxOfOrNull { it.hops } ?: 0)
                }
            }
            BadgedBox(badge = { if (g.unread > 0) Badge { Text(g.unread.toString()) } }) {
                IconButton(onChat) { Icon(Icons.AutoMirrored.Rounded.Chat, stringResource(R.string.group_chat)) }
            }
            IconButton({ showInvite = true }) { Icon(Icons.Rounded.PersonAdd, stringResource(R.string.home_invite)) }
            Box {
                IconButton({ showMenu = true }) { Icon(Icons.Rounded.MoreVert, null) }
                DropdownMenu(showMenu, { showMenu = false }) {
                    DropdownMenuItem({ Text(stringResource(R.string.group_share_location)) }, { showMenu = false; shareLocation(ctx, engine, g.id) })
                    DropdownMenuItem({ Text(stringResource(R.string.group_leave), color = MaterialTheme.colorScheme.error) }, { showMenu = false; confirmLeave = true })
                }
            }
        }

        // status banner (handover / suspended)
        AnimatedVisibility(g.suspended || g.handover != "stable", enter = slideInVertically() + fadeIn(), exit = slideOutVertically() + fadeOut()) {
            val text = when {
                g.suspended -> stringResource(R.string.handover_suspended)
                g.handover == "switching" -> stringResource(R.string.handover_switching, g.link?.name ?: "")
                else -> stringResource(R.string.handover_degraded)
            }
            Text(text, Modifier.fillMaxWidth().padding(horizontal = 20.dp).clip(RoundedCornerShape(12.dp)).background(MaterialTheme.colorScheme.surfaceContainerHigh).padding(10.dp), style = MaterialTheme.typography.labelMedium, textAlign = TextAlign.Center)
        }

        // members strip
        LazyRow(Modifier.fillMaxWidth().padding(vertical = 16.dp), contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 20.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            items(g.members, key = { it.node.joinToString("") { b -> "%02x".format(b) } }) { m ->
                val hex = m.node.joinToString("") { b -> "%02x".format(b) }
                val peer = state.peers[hex]
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Avatar(m.name, m.hue.toInt(), size = 56.dp, talking = g.talkerNode == hex)
                    Spacer(Modifier.height(6.dp))
                    Text(if (hex == state.nodeId) stringResource(R.string.group_you) else m.name, style = MaterialTheme.typography.labelMedium, maxLines = 1)
                    if (peer != null && hex != state.nodeId) Text(if (peer.hops > 1) stringResource(R.string.group_hops, peer.hops) else peer.link.name.lowercase(), style = TelemetryStyle, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }

        Spacer(Modifier.weight(1f))

        // who is talking
        AnimatedContent(g.floor to g.talkerName, transitionSpec = { (fadeIn() + slideInVertically { it / 2 }) togetherWith fadeOut() }, label = "who") { (floor, who) ->
            Text(
                when (floor) {
                    FloorState.Talking -> stringResource(R.string.group_talking)
                    FloorState.Busy -> stringResource(R.string.group_busy, who ?: "")
                    FloorState.Queued -> stringResource(R.string.group_queued)
                    FloorState.Pending -> "…"
                    FloorState.Idle -> stringResource(R.string.group_idle)
                },
                Modifier.fillMaxWidth(), textAlign = TextAlign.Center, style = MaterialTheme.typography.titleMedium,
                color = when (floor) { FloorState.Talking -> MaterialTheme.titi.transmit; FloorState.Busy -> MaterialTheme.titi.receive; else -> MaterialTheme.colorScheme.onSurfaceVariant },
            )
        }
        Spacer(Modifier.height(20.dp))

        Box(Modifier.fillMaxWidth().padding(horizontal = 24.dp), contentAlignment = Alignment.Center) {
            TalkButton(
                floor = g.floor, fullDuplex = g.fullDuplex, muted = state.muted, talkerName = g.talkerName, levelDbfs = state.levelDbfs,
                onDown = { engine.pttDown() }, onUp = { engine.pttUp() }, onToggleMute = { engine.setMuted(!state.muted) },
            )
        }
        Spacer(Modifier.height(28.dp))

        // mode + SOS
        Row(Modifier.fillMaxWidth().padding(horizontal = 24.dp), verticalAlignment = Alignment.CenterVertically) {
            SingleChoiceSegmentedButtonRow(Modifier.weight(1f)) {
                SegmentedButton(selected = !g.fullDuplex, onClick = { engine.setFullDuplex(g.id, false) }, shape = SegmentedButtonDefaults.itemShape(0, 2)) { Text(stringResource(R.string.group_mode_ptt)) }
                SegmentedButton(selected = g.fullDuplex, onClick = { engine.setFullDuplex(g.id, true) }, shape = SegmentedButtonDefaults.itemShape(1, 2)) { Text(stringResource(R.string.group_mode_duplex)) }
            }
            Spacer(Modifier.width(12.dp))
            FilledTonalIconButton({ confirmSos = true }, colors = androidx.compose.material3.IconButtonDefaults.filledTonalIconButtonColors(containerColor = MaterialTheme.titi.emergency.copy(alpha = 0.18f), contentColor = MaterialTheme.titi.emergency)) {
                Icon(Icons.Rounded.Emergency, stringResource(R.string.group_sos))
            }
        }
        Spacer(Modifier.height(24.dp))
    }

    if (showInvite) InviteSheet(engine, state, g.id, g.name) { showInvite = false }

    if (confirmLeave) AlertDialog(
        onDismissRequest = { confirmLeave = false },
        title = { Text(stringResource(R.string.group_leave)) },
        text = { Text(stringResource(R.string.group_leave_confirm, g.name)) },
        confirmButton = { TextButton({ confirmLeave = false; engine.leaveGroup(g.id); onBack() }) { Text(stringResource(R.string.group_leave), color = MaterialTheme.colorScheme.error) } },
        dismissButton = { TextButton({ confirmLeave = false }) { Text(stringResource(R.string.cancel)) } },
    )
    if (confirmSos) AlertDialog(
        onDismissRequest = { confirmSos = false },
        icon = { Icon(Icons.Rounded.Emergency, null, tint = MaterialTheme.titi.emergency) },
        title = { Text(stringResource(R.string.group_sos)) },
        text = { Text(stringResource(R.string.group_sos_confirm, g.name)) },
        confirmButton = { Button({ confirmSos = false; sendSos(ctx, engine, g.id) }, colors = androidx.compose.material3.ButtonDefaults.buttonColors(containerColor = MaterialTheme.titi.emergency, contentColor = Color.White)) { Text(stringResource(R.string.group_sos)) } },
        dismissButton = { TextButton({ confirmSos = false }) { Text(stringResource(R.string.cancel)) } },
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun InviteSheet(engine: EngineHost, state: RadioState, gid: String, gname: String, onDismiss: () -> Unit) {
    var code by remember { mutableStateOf(engine.currentCode(gid)) }
    LaunchedEffect(gid) { while (true) { code = engine.currentCode(gid); delay(1000) } }
    val link = remember(gid) { engine.deepLink(gid) }
    val ctx = LocalContext.current
    val clip = LocalClipboardManager.current
    val candidates = state.peers.values.filter { !it.inGroup }

    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(Modifier.padding(horizontal = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Text(stringResource(R.string.invite_title, gname), style = MaterialTheme.typography.headlineSmall)
            Spacer(Modifier.height(20.dp))
            if (candidates.isNotEmpty()) {
                Text(stringResource(R.string.invite_nearby_label), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.height(10.dp))
                LazyRow(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    items(candidates, key = { it.node }) { p ->
                        Column(Modifier.clickable { engine.invitePeer(gid, p.node) }, horizontalAlignment = Alignment.CenterHorizontally) {
                            Avatar(p.name, p.hue, size = 56.dp); Spacer(Modifier.height(6.dp)); Text(p.name, style = MaterialTheme.typography.labelMedium)
                        }
                    }
                }
                Spacer(Modifier.height(24.dp))
            }
            Text(stringResource(R.string.invite_code_label), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(8.dp))
            val c = code
            Text(c?.first?.replace('-', ' ') ?: "…", style = CodeStyle, color = MaterialTheme.titi.transmit, textAlign = TextAlign.Center, modifier = Modifier.clickable { c?.let { clip.setText(AnnotatedString(it.first)) } })
            Text(stringResource(R.string.invite_code_expires, "${(c?.second ?: 0) / 60}:${"%02d".format((c?.second ?: 0) % 60)}"), style = TelemetryStyle, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(20.dp))
            if (link != null) {
                Text(stringResource(R.string.invite_qr_label), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.height(8.dp))
                Image(rememberQr(link), null, Modifier.size(200.dp).clip(RoundedCornerShape(16.dp)).background(Color.White).padding(8.dp))
                Spacer(Modifier.height(16.dp))
                OutlinedButton({ ctx.startActivity(Intent.createChooser(Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, link.replace("titi://j/", "https://titi.app/j/")), null)) }, shape = RoundedCornerShape(14.dp)) {
                    Icon(Icons.Rounded.Share, null); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.invite_share_link))
                }
            }
            Spacer(Modifier.height(32.dp))
        }
    }
}

private fun shareLocation(ctx: android.content.Context, engine: EngineHost, gid: String) {
    ro.titi.app.util.Location.lastKnown(ctx) { lat, lon, acc -> engine.sendLocation(gid, lat, lon, acc, breadcrumb = false) }
}

private fun sendSos(ctx: android.content.Context, engine: EngineHost, gid: String) {
    ro.titi.app.util.Location.lastKnown(ctx, allowNull = true) { lat, lon, _ -> engine.sendSos(gid, lat, lon, "", cancelled = false) }
    engine.pttDown(uniffi.titi_ffi.FfiPriority.EMERGENCY)
}
