package ro.titi.app.ui

import android.graphics.Bitmap
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.ImageBitmap
import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel

@Composable
fun rememberQr(text: String, size: Int = 512, dark: Int = 0xFF0E1013.toInt(), light: Int = 0xFFFFFFFF.toInt()): ImageBitmap = remember(text) {
    val m = QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, size, size, mapOf(EncodeHintType.MARGIN to 1, EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M))
    val bmp = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
    val px = IntArray(size * size)
    for (y in 0 until size) for (x in 0 until size) px[y * size + x] = if (m[x, y]) dark else light
    bmp.setPixels(px, 0, size, 0, 0, size, size)
    bmp.asImageBitmap()
}
