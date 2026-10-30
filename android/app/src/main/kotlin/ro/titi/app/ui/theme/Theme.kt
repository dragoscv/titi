package ro.titi.app.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import ro.titi.app.R
import ro.titi.app.data.ThemeMode
import ro.titi.brand.BrandDark
import ro.titi.brand.BrandLight

// ADR-0007 "Signal Amber" — tokens come from brand/ (brand pack v1) via the generated
// ro.titi.brand.BrandTokens.kt; never hand-type hex here.
object Palette {
    val Graphite = BrandDark.bgCanvas
    val Surface = BrandDark.bgSurface
    val Elevated = BrandDark.bgElevated
    val Outline = BrandDark.borderDefault
    val Text = BrandDark.fgDefault
    val Muted = BrandDark.fgMuted
    val Amber = BrandDark.accentSolid
    val AmberDeep = BrandDark.accentPressed
    val Teal = BrandDark.receiveSolid
    val Danger = BrandDark.dangerSolid
    val Success = BrandDark.successSolid
    val Emergency = BrandDark.emergencySolid

    // light
    val LightBg = BrandLight.bgCanvas
    val LightSurface = BrandLight.bgSurface
    val LightElevated = BrandLight.bgElevated
    val LightOutline = BrandLight.borderDefault
    val LightText = BrandLight.fgDefault
    val LightMuted = BrandLight.fgMuted
}

/** Semantic colours that Material 3 has no slot for. */
data class TitiColors(
    val transmit: Color,
    val transmitPressed: Color,
    val receive: Color,
    val danger: Color,
    val success: Color,
    val emergency: Color,
    val muted: Color,
    val elevated: Color,
)

val LocalTitiColors = staticCompositionLocalOf {
    TitiColors(Palette.Amber, Palette.AmberDeep, Palette.Teal, Palette.Danger, Palette.Success, Palette.Emergency, Palette.Muted, Palette.Elevated)
}

private val Dark: ColorScheme = darkColorScheme(
    primary = Palette.Amber,
    onPrimary = Palette.Graphite,
    primaryContainer = Palette.AmberDeep,
    onPrimaryContainer = Palette.Graphite,
    secondary = Palette.Teal,
    onSecondary = Palette.Graphite,
    background = Palette.Graphite,
    onBackground = Palette.Text,
    surface = Palette.Surface,
    onSurface = Palette.Text,
    surfaceVariant = Palette.Elevated,
    onSurfaceVariant = Palette.Muted,
    surfaceContainer = Palette.Surface,
    surfaceContainerHigh = Palette.Elevated,
    surfaceContainerHighest = BrandDark.bgHighest,
    outline = Palette.Outline,
    outlineVariant = BrandDark.borderSubtle,
    error = Palette.Danger,
    onError = Palette.Graphite,
)

private val Light: ColorScheme = lightColorScheme(
    primary = BrandLight.accentFg,
    onPrimary = Color.White,
    primaryContainer = BrandLight.accentContainer,
    onPrimaryContainer = BrandLight.accentOnContainer,
    secondary = BrandLight.receiveFg,
    onSecondary = Color.White,
    background = Palette.LightBg,
    onBackground = Palette.LightText,
    surface = Palette.LightSurface,
    onSurface = Palette.LightText,
    surfaceVariant = Palette.LightElevated,
    onSurfaceVariant = Palette.LightMuted,
    surfaceContainer = Palette.LightSurface,
    surfaceContainerHigh = Palette.LightElevated,
    surfaceContainerHighest = BrandLight.bgHighest,
    outline = Palette.LightOutline,
    outlineVariant = BrandLight.borderSubtle,
    error = BrandLight.dangerFg,
    onError = Color.White,
)

private val Contrast: ColorScheme = Dark.copy(
    background = Color.Black,
    surface = Color.Black,
    surfaceVariant = Color(0xFF111111),
    surfaceContainer = Color.Black,
    surfaceContainerHigh = Color(0xFF151515),
    onSurfaceVariant = Color(0xFFE0E0E0),
    outline = Color(0xFF808080),
    primary = Color(0xFFFFC542),
)

val Manrope = FontFamily(
    Font(R.font.manrope, FontWeight.Normal),
    Font(R.font.manrope, FontWeight.Medium),
    Font(R.font.manrope, FontWeight.SemiBold),
    Font(R.font.manrope, FontWeight.Bold),
    Font(R.font.manrope, FontWeight.ExtraBold),
)
val GeistMono = FontFamily(Font(R.font.geist_mono, FontWeight.Normal), Font(R.font.geist_mono, FontWeight.Medium))

private val base = Typography()
val TitiTypography = Typography(
    displayLarge = base.displayLarge.copy(fontFamily = Manrope, fontWeight = FontWeight.ExtraBold, letterSpacing = (-1.5).sp),
    displayMedium = base.displayMedium.copy(fontFamily = Manrope, fontWeight = FontWeight.ExtraBold, letterSpacing = (-1).sp),
    displaySmall = base.displaySmall.copy(fontFamily = Manrope, fontWeight = FontWeight.Bold, letterSpacing = (-0.5).sp),
    headlineLarge = base.headlineLarge.copy(fontFamily = Manrope, fontWeight = FontWeight.Bold, letterSpacing = (-0.5).sp),
    headlineMedium = base.headlineMedium.copy(fontFamily = Manrope, fontWeight = FontWeight.Bold),
    headlineSmall = base.headlineSmall.copy(fontFamily = Manrope, fontWeight = FontWeight.SemiBold),
    titleLarge = base.titleLarge.copy(fontFamily = Manrope, fontWeight = FontWeight.SemiBold),
    titleMedium = base.titleMedium.copy(fontFamily = Manrope, fontWeight = FontWeight.SemiBold),
    titleSmall = base.titleSmall.copy(fontFamily = Manrope, fontWeight = FontWeight.SemiBold),
    bodyLarge = base.bodyLarge.copy(fontFamily = Manrope),
    bodyMedium = base.bodyMedium.copy(fontFamily = Manrope),
    bodySmall = base.bodySmall.copy(fontFamily = Manrope),
    labelLarge = base.labelLarge.copy(fontFamily = Manrope, fontWeight = FontWeight.SemiBold),
    labelMedium = base.labelMedium.copy(fontFamily = Manrope, fontWeight = FontWeight.SemiBold),
    labelSmall = base.labelSmall.copy(fontFamily = Manrope, fontWeight = FontWeight.Medium),
)

val CodeStyle = TextStyle(fontFamily = GeistMono, fontWeight = FontWeight.Medium, fontSize = 28.sp, letterSpacing = 1.sp)
val TelemetryStyle = TextStyle(fontFamily = GeistMono, fontSize = 12.sp, letterSpacing = 0.5.sp)

@Composable
fun TitiTheme(mode: ThemeMode = ThemeMode.System, content: @Composable () -> Unit) {
    val dark = when (mode) {
        ThemeMode.System -> isSystemInDarkTheme()
        ThemeMode.Dark, ThemeMode.Contrast -> true
        ThemeMode.Light -> false
    }
    val scheme = when {
        mode == ThemeMode.Contrast -> Contrast
        dark -> Dark
        else -> Light
    }
    val titi = if (dark) {
        TitiColors(Palette.Amber, Palette.AmberDeep, Palette.Teal, Palette.Danger, Palette.Success, Palette.Emergency, Palette.Muted, if (mode == ThemeMode.Contrast) Color(0xFF151515) else Palette.Elevated)
    } else {
        TitiColors(Palette.Amber, Palette.AmberDeep, Color(0xFF0F9F8F), Color(0xFFD32F2F), Color(0xFF1B8F4E), Palette.Emergency, Palette.LightMuted, Palette.LightElevated)
    }
    androidx.compose.runtime.CompositionLocalProvider(LocalTitiColors provides titi) {
        MaterialTheme(colorScheme = scheme, typography = TitiTypography, content = content)
    }
}

val MaterialTheme.titi: TitiColors
    @Composable get() = LocalTitiColors.current

/** Avatar colour from the user's hue (0..360) with fixed S/L for legibility. */
fun hueColor(hue: Int, dark: Boolean = true): Color = Color.hsl(hue.coerceIn(0, 359).toFloat(), 0.72f, if (dark) 0.62f else 0.45f)
