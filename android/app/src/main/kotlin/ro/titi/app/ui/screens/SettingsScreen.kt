package ro.titi.app.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import ro.titi.app.BuildConfig
import ro.titi.app.R
import ro.titi.app.core.EngineHost
import ro.titi.app.data.Prefs
import ro.titi.app.data.QualityOverride
import ro.titi.app.data.Settings
import ro.titi.app.data.SoundPack
import ro.titi.app.data.ThemeMode
import ro.titi.app.ui.theme.TelemetryStyle
import ro.titi.app.ui.theme.hueColor

@Composable
fun SettingsScreen(prefs: Prefs, engine: EngineHost, s: Settings, onBack: () -> Unit) {
    val scope = rememberCoroutineScope()
    fun set(f: (Settings) -> Settings) = scope.launch { prefs.update(f) }

    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).safeDrawingPadding().verticalScroll(rememberScrollState())) {
        Row(Modifier.fillMaxWidth().padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.back)) }
            Text(stringResource(R.string.settings_title), style = MaterialTheme.typography.titleLarge)
        }
        Column(Modifier.padding(horizontal = 20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Section(R.string.settings_profile) {
                OutlinedTextField(s.name, { v -> set { it.copy(name = v) }; engine.setDisplayName(v, s.hue) }, Modifier.fillMaxWidth(), label = { Text(stringResource(R.string.settings_name)) }, singleLine = true, shape = RoundedCornerShape(14.dp))
                Spacer(Modifier.height(12.dp))
                Text(stringResource(R.string.settings_colour), style = MaterialTheme.typography.labelLarge)
                Spacer(Modifier.height(8.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    for (h in listOf(40, 15, 350, 290, 220, 170, 120, 80)) {
                        androidx.compose.foundation.layout.Box(
                            Modifier.size(36.dp).clip(CircleShape).background(hueColor(h))
                                .then(if (h == s.hue) Modifier.border(3.dp, MaterialTheme.colorScheme.onBackground, CircleShape) else Modifier)
                                .clickable { set { it.copy(hue = h) }; engine.setDisplayName(s.name, h) },
                        )
                    }
                }
            }
            Section(R.string.settings_audio) {
                Text(stringResource(R.string.settings_quality), style = MaterialTheme.typography.labelLarge)
                Spacer(Modifier.height(8.dp))
                SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                    QualityOverride.entries.forEachIndexed { i, q ->
                        SegmentedButton(selected = s.quality == q, onClick = { set { it.copy(quality = q) } }, shape = SegmentedButtonDefaults.itemShape(i, QualityOverride.entries.size)) {
                            Text(when (q) { QualityOverride.Auto -> "Auto"; QualityOverride.Hq -> "HQ"; QualityOverride.Std -> "Std"; QualityOverride.Low -> "Low" })
                        }
                    }
                }
                Spacer(Modifier.height(12.dp))
                Text(stringResource(R.string.settings_sounds), style = MaterialTheme.typography.labelLarge)
                Spacer(Modifier.height(8.dp))
                SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                    SoundPack.entries.forEachIndexed { i, p ->
                        SegmentedButton(selected = s.soundPack == p, onClick = { set { it.copy(soundPack = p) } }, shape = SegmentedButtonDefaults.itemShape(i, 3)) {
                            Text(stringResource(when (p) { SoundPack.Bird -> R.string.settings_sound_bird; SoundPack.Radio -> R.string.settings_sound_radio; SoundPack.Minimal -> R.string.settings_sound_minimal }))
                        }
                    }
                }
                Toggle(R.string.settings_haptics_only, s.hapticsOnly) { v -> set { it.copy(hapticsOnly = v) } }
                Toggle(R.string.settings_ptt_volume, s.volumePtt) { v -> set { it.copy(volumePtt = v) } }
            }
            Section(R.string.settings_transports) {
                Toggle(R.string.settings_transport_internet, s.useInternet) { v -> set { it.copy(useInternet = v) } }
                Toggle(R.string.settings_transport_ble, s.useBle) { v -> set { it.copy(useBle = v) } }
                Toggle(R.string.settings_transport_hotspot, s.allowHotspot) { v -> set { it.copy(allowHotspot = v) } }
                Toggle(R.string.settings_battery, s.batterySaver, R.string.settings_battery_desc) { v -> set { it.copy(batterySaver = v) } }
            }
            Section(R.string.settings_theme) {
                SingleChoiceSegmentedButtonRow(Modifier.fillMaxWidth()) {
                    ThemeMode.entries.forEachIndexed { i, m ->
                        SegmentedButton(selected = s.theme == m, onClick = { set { it.copy(theme = m) } }, shape = SegmentedButtonDefaults.itemShape(i, 4)) {
                            Text(stringResource(when (m) { ThemeMode.System -> R.string.settings_theme_system; ThemeMode.Dark -> R.string.settings_theme_dark; ThemeMode.Light -> R.string.settings_theme_light; ThemeMode.Contrast -> R.string.settings_theme_contrast }), maxLines = 1)
                        }
                    }
                }
            }
            Section(R.string.settings_about) {
                Text(stringResource(R.string.settings_version, BuildConfig.VERSION_NAME), style = MaterialTheme.typography.bodyMedium)
                Spacer(Modifier.height(6.dp))
                Text(stringResource(R.string.settings_node_id), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(engine.nodeIdHex, style = TelemetryStyle)
            }
            Spacer(Modifier.height(40.dp))
        }
    }
}

@Composable
private fun Section(title: Int, content: @Composable () -> Unit) {
    Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(22.dp)).background(MaterialTheme.colorScheme.surfaceContainer).padding(18.dp)) {
        Text(stringResource(title), style = MaterialTheme.typography.titleSmall, color = MaterialTheme.colorScheme.primary)
        Spacer(Modifier.height(12.dp))
        content()
    }
}

@Composable
private fun Toggle(title: Int, value: Boolean, desc: Int? = null, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(stringResource(title), style = MaterialTheme.typography.bodyLarge)
            desc?.let { Text(stringResource(it), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant) }
        }
        Spacer(Modifier.width(12.dp))
        Switch(value, onChange)
    }
}
