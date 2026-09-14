package ro.titi.app.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.navigation3.runtime.NavEntry
import androidx.navigation3.runtime.rememberNavBackStack
import androidx.navigation3.ui.NavDisplay
import kotlinx.coroutines.launch
import ro.titi.app.R
import ro.titi.app.core.EngineHost
import ro.titi.app.core.Invite
import ro.titi.app.core.Toast
import ro.titi.app.data.Prefs
import ro.titi.app.data.Settings
import ro.titi.app.ui.components.Avatar
import ro.titi.app.ui.screens.ChatScreen
import ro.titi.app.ui.screens.GroupScreen
import ro.titi.app.ui.screens.HomeScreen
import ro.titi.app.ui.screens.JoinScreen
import ro.titi.app.ui.screens.OnboardingScreen
import ro.titi.app.ui.screens.SettingsScreen
import ro.titi.app.ui.Settings as SettingsRoute

@Composable
fun TitiApp(engine: EngineHost, prefs: Prefs, settings: Settings) {
    val ctx = LocalContext.current
    val state by engine.state.collectAsState()
    val snack = remember { SnackbarHostState() }
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    LaunchedEffect(Unit) {
        engine.toasts.collect { t ->
            val msg = when (t) {
                is Toast.Raw -> t.text
                is Toast.Text -> if (t.arg != null) ctx.getString(t.res, t.arg) else ctx.getString(t.res)
            }
            scope.launch { snack.showSnackbar(msg) }
        }
    }

    val backStack = rememberNavBackStack(if (settings.onboarded) Home else Onboarding)
    // Jump into a group when we join one from a link/code while on Home
    LaunchedEffect(state.groups.size, state.activeGroup) {
        val top = backStack.lastOrNull()
        if (top == Join && state.activeGroup != null) { backStack.removeLastOrNull(); backStack.add(Group(state.activeGroup!!)) }
    }

    Scaffold(snackbarHost = { SnackbarHost(snack) }, containerColor = MaterialTheme.colorScheme.background) { _ ->
        Box(Modifier.fillMaxSize()) {
            NavDisplay(
                backStack = backStack,
                onBack = { backStack.removeLastOrNull() },
                entryProvider = { key ->
                    when (key) {
                        is Onboarding -> NavEntry(key) {
                            OnboardingScreen(prefs, engine) { backStack.clear(); backStack.add(Home) }
                        }
                        is Home -> NavEntry(key) {
                            HomeScreen(engine, state, settings,
                                onOpenGroup = { engine.setActiveGroup(it); backStack.add(Group(it)) },
                                onJoin = { backStack.add(Join) },
                                onSettings = { backStack.add(SettingsRoute) })
                        }
                        is Group -> NavEntry(key) {
                            GroupScreen(engine, state, key.id, onBack = { backStack.removeLastOrNull() }, onChat = { backStack.add(Chat(key.id)) })
                        }
                        is Chat -> NavEntry(key) { ChatScreen(engine, state, key.id, onBack = { backStack.removeLastOrNull() }) }
                        is Join -> NavEntry(key) { JoinScreen(engine, state, onBack = { backStack.removeLastOrNull() }) }
                        is SettingsRoute -> NavEntry(key) { SettingsScreen(prefs, engine, settings, onBack = { backStack.removeLastOrNull() }) }
                        else -> NavEntry(key) { Text("?") }
                    }
                },
            )
            InviteBanner(state.invites.firstOrNull(), onAccept = { engine.acceptInvite(it) }, onDecline = { engine.declineInvite(it) })
        }
    }
}

@Composable
private fun InviteBanner(inv: Invite?, onAccept: (Invite) -> Unit, onDecline: (Invite) -> Unit) {
    AnimatedVisibility(inv != null, enter = slideInVertically { -it }, exit = slideOutVertically { -it }) {
        val i = inv ?: return@AnimatedVisibility
        Card(
            Modifier.fillMaxWidth().statusBarsPadding().padding(12.dp),
            colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
            elevation = CardDefaults.cardElevation(8.dp),
        ) {
            Row(Modifier.padding(14.dp), verticalAlignment = Alignment.CenterVertically) {
                Avatar(i.hostName, (i.host.take(2).toIntOrNull(16) ?: 40) * 360 / 256)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(stringResource(R.string.invite_offered_title, i.hostName), style = MaterialTheme.typography.titleSmall)
                    Text(stringResource(R.string.invite_offered_body, i.name, i.members), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    TextButton(onClick = { onDecline(i) }) { Text(stringResource(R.string.invite_decline)) }
                    Button(onClick = { onAccept(i) }) { Text(stringResource(R.string.invite_accept)) }
                }
            }
        }
    }
}
