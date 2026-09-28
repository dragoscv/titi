package ro.titi.app.ui

import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageAnalysis
import androidx.camera.core.ImageProxy
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LocalLifecycleOwner
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean

/** foss flavour: CameraX + zxing (no Play Services / ML Kit). */
@Composable
fun QrScanner(onResult: (String) -> Unit) {
    val owner = LocalLifecycleOwner.current
    val done = remember { AtomicBoolean(false) }
    val worker = remember { Executors.newSingleThreadExecutor() }
    DisposableEffect(Unit) { onDispose { worker.shutdown() } }
    AndroidView(factory = { c ->
        val view = PreviewView(c)
        val future = ProcessCameraProvider.getInstance(c)
        val main = ContextCompat.getMainExecutor(c)
        future.addListener({
            val provider = future.get()
            val preview = Preview.Builder().build().also { it.surfaceProvider = view.surfaceProvider }
            val analysis = ImageAnalysis.Builder().setBackpressureStrategy(ImageAnalysis.STRATEGY_KEEP_ONLY_LATEST).build()
            analysis.setAnalyzer(worker) { img ->
                val v = img.use { QrDecode.luminance(it.yPlane(), it.width, it.height) }
                if (v != null && done.compareAndSet(false, true)) main.execute { onResult(v) }
            }
            provider.unbindAll()
            provider.bindToLifecycle(owner, CameraSelector.DEFAULT_BACK_CAMERA, preview, analysis)
        }, main)
        view
    }, modifier = Modifier.fillMaxSize())
}

/** Copies the Y plane into a tight width×height buffer (rows may be padded). */
private fun ImageProxy.yPlane(): ByteArray {
    val p = planes[0]
    val buf = p.buffer.duplicate().also { it.rewind() }
    val out = ByteArray(width * height)
    if (p.rowStride == width && p.pixelStride == 1) { buf.get(out, 0, minOf(out.size, buf.remaining())); return out }
    val row = ByteArray(p.rowStride)
    for (y in 0 until height) {
        buf.position(y * p.rowStride)
        buf.get(row, 0, minOf(p.rowStride, buf.remaining()))
        for (x in 0 until width) out[y * width + x] = row[x * p.pixelStride]
    }
    return out
}
