package ro.titi.app.ui.screens

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Login
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material3.Badge
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import ro.titi.app.R
import ro.titi.app.core.EngineHost
import ro.titi.app.core.FloorState
import ro.titi.app.core.GroupState
import ro.titi.app.core.Peer
import ro.titi.app.core.RadioState
import ro.titi.app.data.Settings
import ro.titi.app.ui.components.Avatar
import ro.titi.app.ui.components.LinkChip
import ro.titi.app.ui.components.icon
import ro.titi.app.ui.theme.titi

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(engine: EngineHost, state: RadioState, settings: Settings, onOpenGroup: (String) -> Unit, onJoin: () -> Unit, onSettings: () -> Unit) {
    var showCreate by remember { mutableStateOf(false) }
    var invitePeer by remember { mutableStateOf<Peer?>(null) }

    LazyColumn(
        Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = 56.dp, bottom = 120.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                Avatar(settings.name.ifBlank { "T" }, settings.hue, size = 40.dp)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(settings.name, style = MaterialTheme.typography.titleMedium)
                    Text(
                        if (state.running) stringResource(R.string.notif_idle, state.peers.size) else stringResource(R.string.link_none),
                        style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                IconButton(onSettings) { Icon(Icons.Rounded.Settings, stringResource(R.string.home_settings)) }
            }
            Spacer(Modifier.height(20.dp))
        }

        item {
            Text(stringResource(R.string.home_nearby), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(10.dp))
            NearbyRadar(state.peers.values.toList(), onTap = { invitePeer = it })
            Spacer(Modifier.height(12.dp))
        }

        item {
            Text(stringResource(R.string.home_groups), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        if (state.groups.isEmpty()) {
            item {
                Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(22.dp)).background(MaterialTheme.colorScheme.surfaceContainer).padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text(stringResource(R.string.home_no_groups), style = MaterialTheme.typography.titleMedium)
                    Spacer(Modifier.height(4.dp))
                    Text(stringResource(R.string.home_no_groups_hint), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
        items(state.groups, key = { it.id }) { g -> GroupCard(g, g.id == state.activeGroup) { onOpenGroup(g.id) } }

        item {
            Spacer(Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Button({ showCreate = true }, Modifier.weight(1f).height(56.dp), shape = RoundedCornerShape(18.dp)) {
                    Icon(Icons.Rounded.Add, null); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.home_create))
                }
                OutlinedButton(onJoin, Modifier.weight(1f).height(56.dp), shape = RoundedCornerShape(18.dp)) {
                    Icon(Icons.Rounded.Login, null); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.home_join))
                }
            }
        }
    }

    if (showCreate) {
        var name by remember { mutableStateOf("") }
        ModalBottomSheet(onDismissRequest = { showCreate = false }) {
            Column(Modifier.padding(24.dp)) {
                Text(stringResource(R.string.create_title), style = MaterialTheme.typography.headlineSmall)
                Spacer(Modifier.height(16.dp))
                OutlinedTextField(name, { name = it }, Modifier.fillMaxWidth(), singleLine = true, placeholder = { Text(stringResource(R.string.create_hint)) }, shape = RoundedCornerShape(16.dp))
                Spacer(Modifier.height(16.dp))
                Button({ engine.createGroup(name.trim()); showCreate = false }, Modifier.fillMaxWidth().height(56.dp), enabled = name.trim().length >= 2, shape = RoundedCornerShape(18.dp)) {
                    Text(stringResource(R.string.create_button))
                }
                Spacer(Modifier.height(24.dp))
            }
        }
    }

    invitePeer?.let { p ->
        ModalBottomSheet(onDismissRequest = { invitePeer = null }) {
            Column(Modifier.padding(24.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Avatar(p.name, p.hue, size = 48.dp); Spacer(Modifier.width(12.dp))
                    Column { Text(p.name, style = MaterialTheme.typography.titleMedium); LinkChip(p.link, p.bars, p.hops) }
                }
                Spacer(Modifier.height(20.dp))
                Text(stringResource(R.string.home_invite), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.height(8.dp))
                if (state.groups.isEmpty()) {
                    Button({ engine.createGroup(p.name); invitePeer = null }, Modifier.fillMaxWidth().height(52.dp), shape = RoundedCornerShape(16.dp)) { Text(stringResource(R.string.home_create)) }
                }
                for (g in state.groups) {
                    OutlinedButton({ engine.invitePeer(g.id, p.node); invitePeer = null }, Modifier.fillMaxWidth().height(52.dp), shape = RoundedCornerShape(16.dp)) { Text(g.name) }
                    Spacer(Modifier.height(8.dp))
                }
                Spacer(Modifier.height(16.dp))
            }
        }
    }
}

@Composable
private fun GroupCard(g: GroupState, active: Boolean, onClick: () -> Unit) {
    val talking = g.floor == FloorState.Busy || g.floor == FloorState.Talking
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(22.dp))
            .background(if (active) MaterialTheme.colorScheme.surfaceContainerHigh else MaterialTheme.colorScheme.surfaceContainer)
            .clickable(onClick = onClick).padding(18.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(Modifier.size(48.dp).clip(RoundedCornerShape(16.dp)).background(if (talking) MaterialTheme.titi.receive else MaterialTheme.colorScheme.primary.copy(alpha = 0.18f)), contentAlignment = Alignment.Center) {
            Text(g.name.take(1).uppercase(), style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold, color = if (talking) MaterialTheme.colorScheme.background else MaterialTheme.colorScheme.primary)
        }
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            Text(g.name, style = MaterialTheme.typography.titleMedium)
            Text(
                if (talking && g.talkerName != null) stringResource(R.string.group_busy, g.talkerName)
                else if (g.members.size == 1) stringResource(R.string.home_member_one) else stringResource(R.string.home_members, g.members.size),
                style = MaterialTheme.typography.bodySmall, color = if (talking) MaterialTheme.titi.receive else MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        if (g.unread > 0) Badge { Text(g.unread.toString()) }
        g.link?.let { Spacer(Modifier.width(8.dp)); Icon(it.icon(), null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant) }
    }
}

@Composable
private fun NearbyRadar(peers: List<Peer>, onTap: (Peer) -> Unit) {
    val sweep = rememberInfiniteTransition(label = "radar")
    val angle by sweep.animateFloat(0f, 360f, infiniteRepeatable(tween(3200, easing = LinearEasing), RepeatMode.Restart), label = "a")
    val teal = MaterialTheme.titi.receive
    val outline = MaterialTheme.colorScheme.outline
    Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(22.dp)).background(MaterialTheme.colorScheme.surfaceContainer).padding(16.dp)) {
        if (peers.isEmpty()) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Canvas(Modifier.size(56.dp)) {
                    val r = size.minDimension / 2
                    for (k in 1..3) drawCircle(outline, r * k / 3, style = Stroke(1.dp.toPx()))
                    val rad = Math.toRadians(angle.toDouble())
                    drawLine(teal, center, center + Offset((r * Math.cos(rad)).toFloat(), (r * Math.sin(rad)).toFloat()), strokeWidth = 2.dp.toPx())
                }
                Spacer(Modifier.width(14.dp))
                Text(stringResource(R.string.home_nearby_empty), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        } else {
            LazyRow(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                items(peers, key = { it.node }) { p ->
                    Column(Modifier.clickable { onTap(p) }, horizontalAlignment = Alignment.CenterHorizontally) {
                        Avatar(p.name, p.hue, size = 52.dp)
                        Spacer(Modifier.height(6.dp))
                        Text(p.name, style = MaterialTheme.typography.labelMedium, maxLines = 1)
                        Icon(p.link.icon(), null, Modifier.size(14.dp), tint = if (p.inGroup) teal else MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }
    }
}
