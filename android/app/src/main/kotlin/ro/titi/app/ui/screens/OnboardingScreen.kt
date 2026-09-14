package ro.titi.app.ui.screens

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.LocationOn
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.Notifications
import androidx.compose.material.icons.rounded.Radar
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import ro.titi.app.R
import ro.titi.app.core.EngineHost
import ro.titi.app.data.Prefs
import ro.titi.app.ui.Permissions
import ro.titi.app.ui.components.Avatar
import ro.titi.app.ui.theme.hueColor
import ro.titi.app.ui.theme.titi

@Composable
fun OnboardingScreen(prefs: Prefs, engine: EngineHost, onDone: () -> Unit) {
    var step by remember { mutableIntStateOf(0) }
    var name by remember { mutableStateOf("") }
    var hue by remember { mutableIntStateOf(40) }
    val scope = rememberCoroutineScope()

    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).safeDrawingPadding()) {
        AnimatedContent(step, transitionSpec = { slideInHorizontally { it } togetherWith slideOutHorizontally { -it } }, label = "onb") { s ->
            when (s) {
                0 -> NameStep(name, hue, { name = it }, { hue = it }) {
                    scope.launch { prefs.update { p -> p.copy(name = name.trim(), hue = hue) } }
                    engine.setDisplayName(name.trim(), hue)
                    step = 1
                }
                else -> PermissionsStep {
                    scope.launch { prefs.update { p -> p.copy(onboarded = true) }; onDone() }
                }
            }
        }
    }
}

@Composable
private fun NameStep(name: String, hue: Int, onName: (String) -> Unit, onHue: (Int) -> Unit, onNext: () -> Unit) {
    Column(Modifier.fillMaxSize().padding(24.dp), verticalArrangement = Arrangement.SpaceBetween) {
        Column {
            Spacer(Modifier.height(48.dp))
            Box(Modifier.size(72.dp).clip(RoundedCornerShape(22.dp)).background(MaterialTheme.titi.transmit), contentAlignment = Alignment.Center) {
                Icon(Icons.Rounded.Mic, null, Modifier.size(36.dp), tint = MaterialTheme.colorScheme.background)
            }
            Spacer(Modifier.height(28.dp))
            Text(stringResource(R.string.onboarding_title), style = MaterialTheme.typography.displaySmall)
            Spacer(Modifier.height(12.dp))
            Text(stringResource(R.string.onboarding_subtitle), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(36.dp))
            Text(stringResource(R.string.onboarding_name_label), style = MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(10.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Avatar(name.ifBlank { "?" }, hue, size = 52.dp)
                Spacer(Modifier.width(14.dp))
                OutlinedTextField(
                    name, onName, Modifier.weight(1f), singleLine = true,
                    placeholder = { Text(stringResource(R.string.onboarding_name_hint)) },
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Words, imeAction = ImeAction.Done),
                    shape = RoundedCornerShape(16.dp),
                )
            }
            Spacer(Modifier.height(24.dp))
            Text(stringResource(R.string.onboarding_colour_label), style = MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(10.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                for (h in listOf(40, 15, 350, 290, 220, 170, 120, 80)) {
                    Box(
                        Modifier.size(40.dp).clip(CircleShape).background(hueColor(h))
                            .then(if (h == hue) Modifier.border(3.dp, MaterialTheme.colorScheme.onBackground, CircleShape) else Modifier)
                            .clickable { onHue(h) },
                        contentAlignment = Alignment.Center,
                    ) { if (h == hue) Icon(Icons.Rounded.Check, null, tint = MaterialTheme.colorScheme.background) }
                }
            }
        }
        Button(onNext, Modifier.fillMaxWidth().height(56.dp), enabled = name.trim().length >= 2, shape = RoundedCornerShape(18.dp)) {
            Text(stringResource(R.string.onboarding_continue), style = MaterialTheme.typography.titleMedium)
        }
    }
}

@Composable
private fun PermissionsStep(onFinish: () -> Unit) {
    val ctx = LocalContext.current
    var tick by remember { mutableIntStateOf(0) }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { tick++ }
    @Suppress("UNUSED_EXPRESSION") tick
    val essential = Permissions.essentialGranted(ctx)

    Column(Modifier.fillMaxSize().padding(24.dp), verticalArrangement = Arrangement.SpaceBetween) {
        Column {
            Spacer(Modifier.height(48.dp))
            Text(stringResource(R.string.perm_title), style = MaterialTheme.typography.displaySmall)
            Spacer(Modifier.height(12.dp))
            Text(stringResource(R.string.perm_subtitle), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(28.dp))
            PermRow(Icons.Rounded.Mic, R.string.perm_mic, R.string.perm_mic_why, Permissions.granted(ctx, Permissions.mic)) { launcher.launch(Permissions.mic.toTypedArray()) }
            PermRow(Icons.Rounded.Radar, R.string.perm_nearby, R.string.perm_nearby_why, Permissions.granted(ctx, Permissions.nearby)) { launcher.launch(Permissions.nearby.toTypedArray()) }
            if (Permissions.notifications.isNotEmpty()) {
                PermRow(Icons.Rounded.Notifications, R.string.perm_notifications, R.string.perm_notifications_why, Permissions.granted(ctx, Permissions.notifications)) { launcher.launch(Permissions.notifications.toTypedArray()) }
            }
            PermRow(Icons.Rounded.LocationOn, R.string.perm_location, R.string.perm_location_why, Permissions.granted(ctx, Permissions.location)) { launcher.launch(Permissions.location.toTypedArray()) }
        }
        Button(onFinish, Modifier.fillMaxWidth().height(56.dp), enabled = essential, shape = RoundedCornerShape(18.dp)) {
            Text(stringResource(R.string.perm_finish), style = MaterialTheme.typography.titleMedium)
        }
    }
}

@Composable
private fun PermRow(icon: ImageVector, title: Int, why: Int, granted: Boolean, onGrant: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = 8.dp).clip(RoundedCornerShape(18.dp)).background(MaterialTheme.colorScheme.surfaceContainer).padding(16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(icon, null, tint = if (granted) MaterialTheme.titi.success else MaterialTheme.colorScheme.primary)
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            Text(stringResource(title), style = MaterialTheme.typography.titleSmall)
            Text(stringResource(why), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Spacer(Modifier.width(8.dp))
        if (granted) Icon(Icons.Rounded.Check, stringResource(R.string.perm_granted), tint = MaterialTheme.titi.success)
        else OutlinedButton(onGrant, shape = RoundedCornerShape(12.dp)) { Text(stringResource(R.string.perm_grant)) }
    }
}
