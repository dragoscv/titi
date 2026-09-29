package ro.titi.tv.ui

import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.tv.material3.MaterialTheme
import androidx.tv.material3.darkColorScheme

object Tv {
    val Graphite = Color(0xFF0E1013)
    val Surface = Color(0xFF16191E)
    val Elevated = Color(0xFF1E2229)
    val Outline = Color(0xFF2A2F38)
    val Ink = Color(0xFFF2F4F7)
    val Muted = Color(0xFF9AA3B2)
    val Amber = Color(0xFFFFB020)
    val Teal = Color(0xFF2DD4BF)
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
