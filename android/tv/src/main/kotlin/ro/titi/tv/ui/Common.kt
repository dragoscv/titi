package ro.titi.tv.ui

import android.graphics.Bitmap
import android.view.KeyEvent as NativeKeyEvent
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.tv.material3.Border
import androidx.tv.material3.ClickableSurfaceDefaults
import androidx.tv.material3.Text
import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel

/** OK / Enter on the focused element. Media Play-Pause and the red key are handled by the activity. */
private val OK_KEYS = setOf(NativeKeyEvent.KEYCODE_DPAD_CENTER, NativeKeyEvent.KEYCODE_ENTER, NativeKeyEvent.KEYCODE_NUMPAD_ENTER)

/** Hold OK = talk: first key-down takes the floor, key-up releases it (repeats are swallowed). */
fun Modifier.holdToTalk(onDown: () -> Unit, onUp: () -> Unit): Modifier = onPreviewKeyEvent { e ->
    val n = e.nativeKeyEvent
    if (n.keyCode !in OK_KEYS) return@onPreviewKeyEvent false
    when (e.type) {
        KeyEventType.KeyDown -> { if (n.repeatCount == 0) onDown(); true }
        KeyEventType.KeyUp -> { onUp(); true }
        else -> false
    }
}

fun isOkKey(code: Int) = code in OK_KEYS

@Composable
fun tileShape(radius: Dp = 13.dp) = ClickableSurfaceDefaults.shape(shape = RoundedCornerShape(radius))

@Composable
fun amberFocusBorder(radius: Dp = 13.dp) = ClickableSurfaceDefaults.border(
    focusedBorder = Border(border = BorderStroke(2.dp, Tv.Amber), shape = RoundedCornerShape(radius)),
)

@Composable
fun Avatar(name: String, hue: Int, size: Dp, modifier: Modifier = Modifier, ring: Color? = null) {
    Box(
        modifier
            .size(size)
            .let { if (ring != null) it.border(size / 20, ring, CircleShape) else it }
            .background(hueColor(hue), CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            name.trim().firstOrNull()?.uppercase() ?: "?",
            color = Tv.Graphite,
            fontSize = (size.value * 0.42f).sp,
            fontWeight = FontWeight.Bold,
        )
    }
}

@Composable
fun rememberQr(text: String, size: Int = 512): ImageBitmap = remember(text) {
    val m = QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, size, size, mapOf(EncodeHintType.MARGIN to 1, EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M))
    val dark = 0xFF0E1013.toInt()
    val light = 0xFFFFFFFF.toInt()
    val px = IntArray(size * size)
    for (y in 0 until size) for (x in 0 until size) px[y * size + x] = if (m[x, y]) dark else light
    Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888).apply { setPixels(px, 0, size, 0, 0, size, size) }.asImageBitmap()
}
