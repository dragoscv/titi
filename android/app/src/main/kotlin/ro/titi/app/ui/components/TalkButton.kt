package ro.titi.app.ui.components

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.changedToUp
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.MicOff
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp
import ro.titi.app.R
import ro.titi.app.core.FloorState
import ro.titi.app.ui.theme.titi

/**
 * The Talk button (ADR-0007). Shapes:
 *  - Idle PTT:        circle 176 dp
 *  - Pressed/pending: squircle 196 dp (armed)
 *  - Transmitting:    wide bar 100% × 96 dp with live level
 *  - Busy (someone):  circle, dimmed, shows talker
 *  - Full-duplex:     wide "mute" pill
 * Spring 380/30 for every morph.
 */
@Composable
fun TalkButton(
    floor: FloorState,
    fullDuplex: Boolean,
    muted: Boolean,
    talkerName: String?,
    levelDbfs: Float,
    onDown: () -> Unit,
    onUp: () -> Unit,
    onToggleMute: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val haptics = LocalHapticFeedback.current
    var pressed by remember { mutableStateOf(false) }
    val onDown by androidx.compose.runtime.rememberUpdatedState(onDown)
    val onUp by androidx.compose.runtime.rememberUpdatedState(onUp)
    val onToggleMute by androidx.compose.runtime.rememberUpdatedState(onToggleMute)
    val spring = spring<androidx.compose.ui.unit.Dp>(stiffness = 380f, dampingRatio = 0.78f)
    val springF = spring<Float>(stiffness = 380f, dampingRatio = 0.78f)

    val transmitting = floor == FloorState.Talking
    val wide = transmitting || fullDuplex
    val armed = pressed && !transmitting && !fullDuplex

    val targetW = when { wide -> 320.dp; armed -> 196.dp; else -> 176.dp }
    val targetH = when { wide -> 96.dp; armed -> 196.dp; else -> 176.dp }
    val targetR = when { wide -> 48.dp; armed -> 56.dp; else -> 88.dp }
    val w by animateDpAsState(targetW, spring, label = "w")
    val h by animateDpAsState(targetH, spring, label = "h")
    val r by animateDpAsState(targetR, spring, label = "r")
    val scale by animateFloatAsState(if (pressed && !wide) 0.96f else 1f, springF, label = "scale")

    val c = MaterialTheme.titi
    val busy = floor == FloorState.Busy || floor == FloorState.Queued
    val bg = when {
        fullDuplex && muted -> MaterialTheme.colorScheme.surfaceContainerHigh
        fullDuplex -> c.receive
        transmitting -> c.transmit
        busy -> MaterialTheme.colorScheme.surfaceContainerHigh
        armed -> c.transmitPressed
        else -> c.transmit
    }
    val fg = if (busy || (fullDuplex && muted)) MaterialTheme.colorScheme.onSurface else Color(0xFF0E1013)
    val level = ((levelDbfs + 50f) / 50f).coerceIn(0f, 1f)
    val levelAnim by animateFloatAsState(if (transmitting) level else 0f, spring(stiffness = Spring.StiffnessMedium), label = "lvl")

    val cd = stringResource(R.string.cd_talk_button)
    val stateDesc = when {
        fullDuplex -> if (muted) stringResource(R.string.group_mute) else stringResource(R.string.group_mode_duplex)
        transmitting -> stringResource(R.string.group_talking)
        busy -> stringResource(R.string.group_busy, talkerName ?: "")
        else -> stringResource(R.string.group_idle)
    }

    Box(
        modifier
            .size(w, h)
            .scale(scale)
            .clip(RoundedCornerShape(r))
            .background(bg)
            .drawBehind {
                if (transmitting) {
                    // level glow sweeping from the left
                    drawRect(
                        Brush.horizontalGradient(listOf(Color.White.copy(alpha = 0.28f * levelAnim), Color.Transparent)),
                        size = size.copy(width = size.width * (0.25f + 0.75f * levelAnim)),
                    )
                }
            }
            .semantics {
                contentDescription = cd; stateDescription = stateDesc; role = Role.Button
                // TalkBack: double-tap toggles (a hold gesture is not reachable via accessibility)
                onClick { if (fullDuplex) onToggleMute() else if (transmitting) onUp() else onDown(); true }
            }
            .pointerInput(fullDuplex) {
                if (fullDuplex) {
                    awaitEachGesture {
                        awaitFirstDown()
                        val up = waitForUpOrCancellation()
                        if (up != null) { haptics.performHapticFeedback(HapticFeedbackType.LongPress); onToggleMute() }
                    }
                } else {
                    awaitEachGesture {
                        val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                        down.consume()
                        pressed = true
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        onDown()
                        try {
                            // A thumb drifts while talking: hold until the finger LIFTS, never
                            // cancel on movement (waitForUpOrCancellation would drop the floor
                            // after touch slop).
                            while (true) {
                                val ev = awaitPointerEvent(PointerEventPass.Initial)
                                ev.changes.forEach { it.consume() }
                                if (ev.changes.all { it.changedToUp() || !it.pressed }) break
                            }
                            haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        } finally {
                            // also runs if the gesture is cancelled (screen left mid-hold)
                            pressed = false
                            onUp()
                        }
                    }
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        AnimatedContent(
            targetState = Triple(wide, transmitting, fullDuplex && muted),
            transitionSpec = { (fadeIn() + scaleIn(initialScale = 0.9f)) togetherWith (fadeOut() + scaleOut(targetScale = 1.1f)) },
            label = "content",
        ) { (isWide, isTx, isMuted) ->
            if (isWide) {
                Row(Modifier.padding(horizontal = 24.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    Icon(if (isMuted) Icons.Rounded.MicOff else if (isTx) Icons.Rounded.GraphicEq else Icons.Rounded.Mic, null, Modifier.size(30.dp), tint = fg)
                    Column {
                        Text(
                            when { isMuted -> stringResource(R.string.group_unmute); isTx -> stringResource(R.string.group_talking); else -> stringResource(R.string.group_mode_duplex) },
                            style = MaterialTheme.typography.titleMedium, color = fg,
                        )
                        Text(
                            when { isMuted -> stringResource(R.string.group_mode_duplex); isTx -> stringResource(R.string.group_release); else -> stringResource(R.string.group_mute) },
                            style = MaterialTheme.typography.labelSmall, color = fg.copy(alpha = 0.72f),
                        )
                    }
                    if (isTx) {
                        Box(Modifier.weight(1f))
                        LevelMeter(levelAnim, fg)
                    }
                }
            } else {
                Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Icon(Icons.Rounded.Mic, null, Modifier.size(44.dp), tint = fg)
                    Text(
                        if (busy) (talkerName ?: "") else stringResource(R.string.group_hold_to_talk),
                        style = MaterialTheme.typography.labelLarge, color = fg.copy(alpha = if (busy) 0.9f else 0.85f),
                    )
                }
            }
        }
    }
}

@Composable
private fun LevelMeter(level: Float, color: Color) {
    Row(horizontalArrangement = Arrangement.spacedBy(3.dp), verticalAlignment = Alignment.CenterVertically) {
        for (i in 0 until 6) {
            val thr = (i + 1) / 6f
            val on = level >= thr * 0.9f
            Box(
                Modifier
                    .width(4.dp)
                    .height((10 + i * 4).dp)
                    .clip(RoundedCornerShape(2.dp))
                    .background(if (on) color else color.copy(alpha = 0.25f)),
            )
        }
    }
}
