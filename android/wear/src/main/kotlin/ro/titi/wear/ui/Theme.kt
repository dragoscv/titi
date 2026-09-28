package ro.titi.wear.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.wear.compose.material3.ColorScheme
import androidx.wear.compose.material3.MaterialTheme
import androidx.wear.compose.material3.MotionScheme
import androidx.wear.compose.material3.dynamicColorScheme

/** ADR-0007 "Signal Amber" on true black (WO-V13); semantic colours stay fixed across watch faces. */
object Titi {
    val Amber = Color(0xFFFFB020)
    val AmberDeep = Color(0xFFD98E00)
    val Transmit = Color(0xFFFF6A3D)
    val Teal = Color(0xFF2DD4BF)
    val Emergency = Color(0xFFFF2D55)
    val Graphite = Color(0xFF0E1013)
    val Surface = Color(0xFF1B1F25)
    val Muted = Color(0xFF9AA3B2)
}

private val Brand = ColorScheme(
    primary = Titi.Amber,
    onPrimary = Titi.Graphite,
    primaryContainer = Color(0xFF3A2A00),
    onPrimaryContainer = Color(0xFFFFDDA0),
    tertiary = Titi.Teal,
    onTertiary = Titi.Graphite,
    error = Titi.Emergency,
    onError = Color.White,
)

/**
 * One UI 8 Watch follows the watch-face palette: containers take the dynamic scheme
 * when available, while talk/receive/SOS keep brand meaning (amber / teal / red).
 */
@Composable
fun TitiWearTheme(content: @Composable () -> Unit) {
    val ctx = LocalContext.current
    val scheme = remember(ctx) { dynamicColorScheme(ctx) ?: Brand }
    MaterialTheme(colorScheme = scheme, motionScheme = MotionScheme.expressive(), content = content)
}
