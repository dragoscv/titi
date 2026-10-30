package ro.titi.wear.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.wear.compose.material3.ColorScheme
import androidx.wear.compose.material3.MaterialTheme
import androidx.wear.compose.material3.MotionScheme
import androidx.wear.compose.material3.dynamicColorScheme
import ro.titi.brand.BrandDark

/** ADR-0007 "Signal Amber" on true black (WO-V13); semantic colours stay fixed across watch faces. Tokens from brand/ (brand pack v1). */
object Titi {
    val Amber = BrandDark.accentSolid
    val AmberDeep = BrandDark.accentPressed
    val Transmit = Color(0xFFFF6A3D)
    val Teal = BrandDark.receiveSolid
    val Emergency = BrandDark.emergencySolid
    val Graphite = BrandDark.bgCanvas
    val Surface = Color(0xFF1B1F25)
    val Muted = BrandDark.fgMuted
}

private val Brand = ColorScheme(
    primary = Titi.Amber,
    onPrimary = Titi.Graphite,
    primaryContainer = BrandDark.accentContainer,
    onPrimaryContainer = BrandDark.accentOnContainer,
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
