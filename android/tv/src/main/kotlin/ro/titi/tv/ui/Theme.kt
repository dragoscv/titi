package ro.titi.tv.ui

import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.darkColorScheme
import ro.titi.brand.BrandDark

/** Tokens from brand/ (brand pack v1, ADR-0007). */
object Tv {
    val Graphite = BrandDark.bgCanvas
    val Surface = BrandDark.bgSurface
    val Elevated = BrandDark.bgElevated
    val Outline = BrandDark.borderDefault
    val Ink = BrandDark.fgDefault
    val Muted = BrandDark.fgMuted
    val Amber = BrandDark.accentSolid
    val Teal = BrandDark.receiveSolid
    val Emergency = BrandDark.emergencySolid
}

fun hueColor(hue: Int): Color = Color.hsl(hue.coerceIn(0, 359).toFloat(), 0.72f, 0.62f)

@Composable
fun TitiTvTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = darkColorScheme(
            primary = Tv.Amber,
            onPrimary = Tv.Graphite,
            secondary = Tv.Teal,
            onSecondary = Tv.Graphite,
            background = Tv.Graphite,
            onBackground = Tv.Ink,
            surface = Tv.Surface,
            onSurface = Tv.Ink,
            surfaceVariant = Tv.Elevated,
            onSurfaceVariant = Tv.Muted,
            border = Tv.Outline,
        ),
        content = content,
    )
}
