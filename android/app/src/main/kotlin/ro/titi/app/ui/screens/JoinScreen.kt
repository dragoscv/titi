package ro.titi.app.ui.screens

import android.Manifest
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.QrCodeScanner
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import ro.titi.app.R
import ro.titi.app.core.EngineHost
import ro.titi.app.core.RadioState
import ro.titi.app.ui.QrScanner
import ro.titi.app.ui.theme.CodeStyle
import uniffi.titi_ffi.parseInviteCode

@Composable
fun JoinScreen(engine: EngineHost, state: RadioState, onBack: () -> Unit) {
    var code by remember { mutableStateOf("") }
    var scanning by remember { mutableStateOf(false) }
    var searching by remember { mutableStateOf(false) }
    val ctx = LocalContext.current
    val camLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { if (it) scanning = true }
    val valid = parseInviteCode(code.trim().lowercase().replace(Regex("\\s+"), "-"))

    Column(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background).safeDrawingPadding()) {
        Row(Modifier.fillMaxWidth().padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onBack) { Icon(Icons.AutoMirrored.Rounded.ArrowBack, stringResource(R.string.back)) }
            Text(stringResource(R.string.join_title), style = MaterialTheme.typography.titleLarge)
        }
        if (scanning) {
            Box(Modifier.fillMaxWidth().weight(1f).padding(20.dp).clip(RoundedCornerShape(24.dp))) {
                QrScanner { text ->
                    scanning = false
                    if (text.startsWith("titi://") || ro.titi.app.util.InviteLinks.HOSTS.any { text.contains("$it/j/") }) {
                        engine.joinByLink(if (text.startsWith("http")) "titi://j/" + text.substringAfter("/j/") else text); searching = true
                    } else { code = text }
                }
            }
            OutlinedButton({ scanning = false }, Modifier.padding(20.dp).fillMaxWidth()) { Text(stringResource(R.string.cancel)) }
        } else {
            Column(Modifier.padding(24.dp)) {
                Text(stringResource(R.string.join_code_help), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Spacer(Modifier.height(24.dp))
                OutlinedTextField(
                    code, { code = it }, Modifier.fillMaxWidth(), singleLine = true, textStyle = CodeStyle.copy(color = MaterialTheme.colorScheme.onSurface),
                    placeholder = { Text(stringResource(R.string.join_code_hint), style = CodeStyle) },
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, keyboardType = KeyboardType.Ascii, imeAction = ImeAction.Go, autoCorrectEnabled = false),
                    shape = RoundedCornerShape(18.dp),
                )
                Spacer(Modifier.height(16.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    OutlinedButton(
                        { if (ContextCompat.checkSelfPermission(ctx, Manifest.permission.CAMERA) == android.content.pm.PackageManager.PERMISSION_GRANTED) scanning = true else camLauncher.launch(Manifest.permission.CAMERA) },
                        Modifier.weight(1f).height(56.dp), shape = RoundedCornerShape(18.dp),
                    ) { Icon(Icons.Rounded.QrCodeScanner, null); Spacer(Modifier.width(8.dp)); Text(stringResource(R.string.join_scan)) }
                    Button(
                        { engine.joinByCode(code.trim().lowercase().replace(Regex("\\s+"), "-")); searching = true },
                        Modifier.weight(1f).height(56.dp), enabled = valid && !searching, shape = RoundedCornerShape(18.dp),
                    ) { Text(stringResource(R.string.join_button)) }
                }
                if (searching) {
                    Spacer(Modifier.height(28.dp))
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp); Spacer(Modifier.width(12.dp))
                        Text(stringResource(R.string.join_searching), style = MaterialTheme.typography.bodyMedium)
                    }
                    if (state.peers.isEmpty()) {
                        Spacer(Modifier.height(12.dp))
                        Text(stringResource(R.string.join_no_peers), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }
    }
}
