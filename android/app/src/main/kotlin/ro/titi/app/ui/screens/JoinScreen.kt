package ro.titi.app.ui.screens

import android.Manifest
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.mlkit.vision.MlKitAnalyzer
import androidx.camera.view.PreviewView
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
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import com.google.mlkit.vision.barcode.BarcodeScanner
import com.google.mlkit.vision.barcode.BarcodeScannerOptions
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.barcode.common.Barcode
import ro.titi.app.R
import ro.titi.app.core.EngineHost
import ro.titi.app.core.RadioState
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
                    if (text.startsWith("titi://") || text.contains("titi.app/j/")) {
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

@Composable
private fun QrScanner(onResult: (String) -> Unit) {
    val ctx = LocalContext.current
    val owner = LocalLifecycleOwner.current
    val scanner: BarcodeScanner = remember { BarcodeScanning.getClient(BarcodeScannerOptions.Builder().setBarcodeFormats(Barcode.FORMAT_QR_CODE).build()) }
    var done by remember { mutableStateOf(false) }
    DisposableEffect(Unit) { onDispose { scanner.close() } }
    AndroidView(factory = { c ->
        val view = PreviewView(c)
        val future = ProcessCameraProvider.getInstance(c)
        future.addListener({
            val provider = future.get()
            val preview = Preview.Builder().build().also { it.surfaceProvider = view.surfaceProvider }
            val analysis = ImageAnalysis.Builder().setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST).build()
            analysis.setAnalyzer(ContextCompat.getMainExecutor(c), MlKitAnalyzer(listOf(scanner), ImageAnalysis.COORDINATE_SYSTEM_ORIGINAL, ContextCompat.getMainExecutor(c)) { r ->
                val v = r.getValue(scanner)?.firstOrNull()?.rawValue
                if (v != null && !done) { done = true; onResult(v) }
            })
            provider.unbindAll()
            provider.bindToLifecycle(owner, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
        }, ContextCompat.getMainExecutor(c))
        view
    }, modifier = Modifier.fillMaxSize())
}
