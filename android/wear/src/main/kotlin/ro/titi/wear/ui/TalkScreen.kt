package ro.titi.wear.ui

import android.media.AudioManager
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.focusable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.MicOff
import androidx.compose.material.icons.rounded.VolumeUp
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.scale
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.rotary.onRotaryScrollEvent
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.wear.compose.material3.Icon
import androidx.wear.compose.material3.LevelIndicator
import androidx.wear.compose.material3.MaterialTheme
import androidx.wear.compose.material3.ScreenScaffold
import androidx.wear.compose.material3.Text
import kotlinx.coroutines.delay
import ro.titi.app.core.EngineHost
import ro.titi.app.core.FloorState
import ro.titi.app.core.GroupState
import ro.titi.app.core.RadioState
import ro.titi.wear.R
import kotlin.math.abs

/**
 * The watch's main surface: one big morphing Talk button.
 * idle circle (amber) → pressed squircle → talking wide squircle (hot orange + level ring)
 * → receiving (teal, breathing halo + talker's name). Rotary bezel = volume.
 */
@Composable
fun TalkScreen(engine: EngineHost, state: RadioState, onNoGroup: () -> Unit) {
    val g = state.active
    val ctx = LocalContext.current
    val am = remember { ctx.getSystemService(AudioManager::class.java) }
    val fr = remember { FocusRequester() }
    var volumeShownAt by remember { mutableLongStateOf(0L) }
    var volume by remember { mutableFloatStateOf(am.volumeFraction()) }
    var acc by remember { mutableFloatStateOf(0f) }
    LaunchedEffect(Unit) { runCatching { fr.requestFocus() } }

    ScreenScaffold {
        Box(
            Modifier
                .fillMaxSize()
                .onRotaryScrollEvent { ev ->
                    // bezel detents: ~one volume step per click
                    acc += ev.verticalScrollPixels
                    if (abs(acc) > 48f) {
                        am.adjustStreamVolume(AudioManager.STREAM_VOICE_CALL, if (acc > 0) AudioManager.ADJUST_RAISE else AudioManager.ADJUST_LOWER, 0)
                        acc = 0f
                        volume = am.volumeFraction()
                        volumeShownAt = System.currentTimeMillis()
                    }
                    true
                }
                .focusRequester(fr)
                .focusable(),
            contentAlignment = Alignment.Center,
        ) {
            if (g == null) {
                NoGroup(onNoGroup)
            } else {
                val level by engine.level.collectAsState()
                Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text(
                        g.name, style = MaterialTheme.typography.titleMedium, maxLines = 1, overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.padding(horizontal = 36.dp), textAlign = TextAlign.Center,
                    )
                    TalkButton(
                        floor = g.floor, talker = g.talkerName, muted = state.muted, fullDuplex = g.fullDuplex, levelDbfs = level,
                        onDown = { engine.pttDown() }, onUp = { engine.pttUp() }, onToggleMute = { engine.setMuted(!state.muted) },
                    )
                    StatusLine(g, state)
                }
            }
            VolumeOverlay(volume, volumeShownAt)
        }
    }
}

@Composable
private fun NoGroup(onJoin: () -> Unit) {
    Column(Modifier.padding(28.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(stringResource(R.string.no_groups), style = MaterialTheme.typography.titleMedium, textAlign = TextAlign.Center)
        Text(stringResource(R.string.no_groups_hint), style = MaterialTheme.typography.bodySmall, textAlign = TextAlign.Center, color = MaterialTheme.colorScheme.onSurfaceVariant)
        androidx.wear.compose.material3.Button(onClick = onJoin, label = { Text(stringResource(R.string.join_code)) })
    }
}

@Composable
private fun StatusLine(g: GroupState, s: RadioState) {
    val online = s.peers.values.count { it.inGroup } + 1
    val text = when {
        g.suspended -> stringResource(R.string.suspended)
        g.floor == FloorState.Talking -> stringResource(R.string.release_to_send)
        g.floor == FloorState.Queued -> stringResource(R.string.queued)
        s.muted && g.fullDuplex -> stringResource(R.string.muted)
        else -> pluralStringResource(R.plurals.online_count, online, online)
    }
    AnimatedContent(text, transitionSpec = { fadeIn(tween(180)) togetherWith fadeOut(tween(120)) }, label = "status") { t ->
        Text(t, style = MaterialTheme.typography.labelSmall, color = if (g.suspended) Titi.Emergency else MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
fun TalkButton(
    floor: FloorState,
    talker: String?,
    muted: Boolean,
    fullDuplex: Boolean,
    levelDbfs: Float,
    onDown: () -> Unit,
    onUp: () -> Unit,
    onToggleMute: () -> Unit,
) {
    val haptics = LocalHapticFeedback.current
    var pressed by remember { mutableStateOf(false) }
    val onDownL by rememberUpdatedState(onDown)
    val onUpL by rememberUpdatedState(onUp)
    val onMuteL by rememberUpdatedState(onToggleMute)
    val talking = floor == FloorState.Talking
    val receiving = floor == FloorState.Busy
    val springDp = spring<androidx.compose.ui.unit.Dp>(dampingRatio = 0.72f, stiffness = 420f)

    // shape morph: circle → squircle (pressed) → wide squircle (talking)
    val size by animateDpAsState(when { talking -> 168.dp; pressed -> 158.dp; receiving -> 146.dp; else -> 150.dp }, springDp, label = "size")
    val corner by animateDpAsState(when { talking -> 44.dp; pressed -> 52.dp; else -> 75.dp }, springDp, label = "corner")
    val scale by animateFloatAsState(if (pressed && !talking) 0.94f else 1f, spring(dampingRatio = 0.6f, stiffness = Spring.StiffnessMediumLow), label = "scale")
    val bg by animateColorAsState(
        when {
            fullDuplex && muted -> Titi.Surface
            talking -> Titi.Transmit
            receiving -> Titi.Teal.copy(alpha = 0.22f)
            floor == FloorState.Queued -> Titi.AmberDeep
            else -> Titi.Amber
        },
        tween(220), label = "bg",
    )
    val fg = if (receiving || (fullDuplex && muted)) Color.White else Titi.Graphite
    val level = ((levelDbfs + 50f) / 50f).coerceIn(0f, 1f)
    val lvl by animateFloatAsState(if (talking) level else 0f, spring(stiffness = Spring.StiffnessMedium), label = "lvl")

    // breathing halo while someone else talks
    val halo = rememberInfiniteTransition(label = "halo")
    val pulse by halo.animateFloat(0f, 1f, infiniteRepeatable(tween(1100), RepeatMode.Restart), label = "pulse")

    val cd = stringResource(R.string.cd_talk)
    val sd = when {
        talking -> stringResource(R.string.cd_talk_state_talking)
        receiving -> stringResource(R.string.cd_talk_state_busy, talker ?: "")
        else -> stringResource(R.string.cd_talk_state_idle)
    }
    Box(
        Modifier
            .size(size + 24.dp)
            .drawBehind {
                val r = this.size.minDimension / 2f
                if (receiving) {
                    drawCircle(Titi.Teal.copy(alpha = 0.45f * (1f - pulse)), radius = r * (0.78f + 0.22f * pulse), style = Stroke(width = 3.dp.toPx()))
                }
                if (talking && lvl > 0.02f) {
                    val sw = 5.dp.toPx()
                    drawArc(
                        Titi.Transmit, startAngle = -90f, sweepAngle = 360f * lvl, useCenter = false,
                        topLeft = Offset(sw, sw), size = Size(this.size.width - 2 * sw, this.size.height - 2 * sw),
                        style = Stroke(width = sw, cap = StrokeCap.Round),
                    )
                }
            },
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier
                .size(size)
                .scale(scale)
                .clip(RoundedCornerShape(corner))
                .background(bg)
                .semantics {
                    contentDescription = cd; stateDescription = sd; role = Role.Button
                    onClick { if (fullDuplex) onMuteL() else if (talking) onUpL() else onDownL(); true }
                }
                .pointerInput(fullDuplex) {
                    awaitEachGesture {
                        val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                        down.consume()
                        if (fullDuplex) {
                            haptics.performHapticFeedback(HapticFeedbackType.LongPress); onMuteL()
                            return@awaitEachGesture
                        }
                        pressed = true
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        onDownL()
                        try {
                            // hold until lift; drifting fingers never drop the floor
                            while (true) {
                                val ev = awaitPointerEvent(PointerEventPass.Initial)
                                ev.changes.forEach { it.consume() }
                                if (ev.changes.all { !it.pressed }) break
                            }
                            haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        } finally {
                            pressed = false
                            onUpL()
                        }
                    }
                },
            contentAlignment = Alignment.Center,
        ) {
            AnimatedContent(
                targetState = Triple(talking, receiving, fullDuplex && muted),
                transitionSpec = { (fadeIn(tween(160)) + scaleIn(initialScale = 0.85f)) togetherWith (fadeOut(tween(100)) + scaleOut(targetScale = 1.1f)) },
                label = "content",
            ) { (tx, rx, mute) ->
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Icon(
                        when { mute -> Icons.Rounded.MicOff; tx -> Icons.Rounded.GraphicEq; rx -> Icons.Rounded.VolumeUp; else -> Icons.Rounded.Mic },
                        contentDescription = null, tint = fg, modifier = Modifier.size(if (tx) 46.dp else 40.dp),
                    )
                    Text(
                        when {
                            mute -> stringResource(R.string.unmute)
                            tx -> stringResource(R.string.talking)
                            rx -> talker ?: stringResource(R.string.listening)
                            else -> stringResource(R.string.hold_to_talk)
                        },
                        style = MaterialTheme.typography.labelMedium, color = fg, maxLines = 1, overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.padding(horizontal = 14.dp),
                    )
                }
            }
        }
    }
}

@Composable
private fun androidx.compose.foundation.layout.BoxScope.VolumeOverlay(volume: Float, shownAt: Long) {
    var visible by remember { mutableStateOf(false) }
    LaunchedEffect(shownAt) {
        if (shownAt == 0L) return@LaunchedEffect
        visible = true; delay(1500); visible = false
    }
    if (visible) LevelIndicator(value = { volume }, modifier = Modifier.align(Alignment.CenterStart))
}

private fun AudioManager.volumeFraction(): Float {
    val max = getStreamMaxVolume(AudioManager.STREAM_VOICE_CALL).coerceAtLeast(1)
    return getStreamVolume(AudioManager.STREAM_VOICE_CALL).toFloat() / max
}
