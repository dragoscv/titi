package ro.titi.app.ui.components

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Bluetooth
import androidx.compose.material.icons.rounded.Cloud
import androidx.compose.material.icons.rounded.Wifi
import androidx.compose.material.icons.rounded.WifiTethering
import androidx.compose.material.icons.rounded.Radar
import ro.titi.app.R
import ro.titi.app.ui.theme.hueColor
import ro.titi.app.ui.theme.titi
import uniffi.titi_ffi.FfiLinkClass

@Composable
fun Avatar(name: String, hue: Int, size: Dp = 40.dp, talking: Boolean = false, modifier: Modifier = Modifier) {
    val color = hueColor(hue)
    val ring by animateColorAsState(if (talking) MaterialTheme.titi.receive else Color.Transparent, label = "ring")
    val pulse = rememberInfiniteTransition(label = "pulse")
    val scale by pulse.animateFloat(1f, 1.25f, infiniteRepeatable(tween(900), RepeatMode.Reverse), label = "s")
    Box(
        modifier
            .size(size)
            .drawBehind {
                if (talking) {
                    drawCircle(ring.copy(alpha = 0.35f), radius = this.size.minDimension / 2 * scale)
                    drawCircle(ring, radius = this.size.minDimension / 2 + 3.dp.toPx(), style = Stroke(2.dp.toPx()))
                }
            }
            .clip(CircleShape)
            .background(color)
            .semantics { contentDescription = "" },
        contentAlignment = Alignment.Center,
    ) {
        Text(
            name.trim().split(' ').filter { it.isNotEmpty() }.take(2).map { it.first().uppercaseChar() }.joinToString(""),
            color = Color(0xFF0E1013),
            fontWeight = FontWeight.Bold,
            fontSize = (size.value * 0.38f).sp,
        )
    }
}

@Composable
fun SignalBars(bars: Int, modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.onSurface) {
    val dim = MaterialTheme.colorScheme.outline
    val desc = stringResource(R.string.cd_signal_bars, bars)
    Canvas(modifier.size(18.dp, 14.dp).semantics { contentDescription = desc }) {
        val w = size.width / 4f
        for (i in 0 until 4) {
            val h = size.height * (0.35f + 0.65f * (i + 1) / 4f)
            drawRoundRect(
                if (i < bars) color else dim,
                topLeft = Offset(i * w + w * 0.15f, size.height - h),
                size = androidx.compose.ui.geometry.Size(w * 0.7f, h),
                cornerRadius = androidx.compose.ui.geometry.CornerRadius(1.5.dp.toPx()),
            )
        }
    }
}

fun FfiLinkClass.icon(): ImageVector = when (this) {
    FfiLinkClass.LAN -> Icons.Rounded.Wifi
    FfiLinkClass.HOTSPOT -> Icons.Rounded.WifiTethering
    FfiLinkClass.WIFI_AWARE -> Icons.Rounded.Radar
    FfiLinkClass.NEARBY -> Icons.Rounded.Radar
    FfiLinkClass.BLE_L2CAP, FfiLinkClass.BLE_GATT, FfiLinkClass.BT_RFCOMM -> Icons.Rounded.Bluetooth
    FfiLinkClass.INTERNET, FfiLinkClass.WEB_RTC -> Icons.Rounded.Cloud
}

@Composable
fun FfiLinkClass.label(): String = stringResource(labelRes())

fun FfiLinkClass.labelRes(): Int = when (this) {
        FfiLinkClass.LAN -> R.string.link_lan
        FfiLinkClass.HOTSPOT -> R.string.link_hotspot
        FfiLinkClass.WIFI_AWARE -> R.string.link_aware
        FfiLinkClass.NEARBY -> R.string.link_nearby
        FfiLinkClass.BLE_L2CAP, FfiLinkClass.BLE_GATT -> R.string.link_ble
        FfiLinkClass.BT_RFCOMM -> R.string.link_bt
        FfiLinkClass.INTERNET, FfiLinkClass.WEB_RTC -> R.string.link_internet
}

@Composable
fun LinkChip(link: FfiLinkClass?, bars: Int = 0, hops: Int = 0, modifier: Modifier = Modifier) {
    val outline = MaterialTheme.colorScheme.outline
    Row(
        modifier
            .clip(RoundedCornerShape(50))
            .border(1.dp, outline, RoundedCornerShape(50))
            .padding(horizontal = 10.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (link != null) {
            Icon(link.icon(), null, Modifier.size(14.dp), tint = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(link.label(), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            SignalBars(bars, color = MaterialTheme.titi.receive)
            if (hops > 1) {
                Box(Modifier.width(1.dp).size(1.dp, 10.dp).background(outline))
                Text(stringResource(R.string.group_hops, hops), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        } else {
            Text(stringResource(R.string.link_none), style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}
