package ro.titi.app.audio

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import ro.titi.app.core.Cue
import ro.titi.app.data.SoundPack
import kotlin.math.PI
import kotlin.math.sin

/**
 * Synthesised UI cues (ADR-0007 sound packs) — no asset files, deterministic,
 * ~1 ms of CPU each. Haptics-only mode skips audio.
 */
class Cues(ctx: Context) {
    private val vib: Vibrator = (ctx.getSystemService(VibratorManager::class.java)).defaultVibrator
    private val scope = CoroutineScope(Dispatchers.Default)
    var pack: SoundPack = SoundPack.Bird
    var hapticsOnly = false

    fun play(c: Cue) {
        haptic(c)
        if (hapticsOnly) return
        val tones: List<Tone> = when (pack) {
            SoundPack.Bird -> when (c) {
                Cue.Granted -> listOf(Tone(1760f, 45, .5f), Tone(2350f, 70, .45f))
                Cue.Released -> listOf(Tone(1320f, 50, .4f), Tone(880f, 70, .35f))
                Cue.Denied -> listOf(Tone(330f, 60, .35f), Tone(0f, 40, 0f), Tone(330f, 60, .35f))
                Cue.Warning -> listOf(Tone(1046f, 40, .35f), Tone(0f, 60, 0f), Tone(1046f, 40, .35f))
                Cue.Incoming -> listOf(Tone(1568f, 40, .3f))
                Cue.Sos -> listOf(Tone(880f, 120, .6f), Tone(1320f, 120, .6f), Tone(880f, 120, .6f), Tone(1320f, 120, .6f))
            }
            SoundPack.Radio -> when (c) {
                Cue.Granted -> listOf(Tone(1000f, 60, .45f))
                Cue.Released -> listOf(Tone(1000f, 40, .4f), Tone(0f, 30, 0f), Tone(800f, 60, .35f))
                Cue.Denied -> listOf(Tone(400f, 120, .4f))
                Cue.Warning -> listOf(Tone(1200f, 50, .35f))
                Cue.Incoming -> listOf(Tone(1400f, 30, .25f))
                Cue.Sos -> listOf(Tone(700f, 200, .6f), Tone(0f, 80, 0f), Tone(700f, 200, .6f))
            }
            SoundPack.Minimal -> when (c) {
                Cue.Granted -> listOf(Tone(1200f, 25, .3f))
                Cue.Released -> listOf(Tone(900f, 25, .25f))
                Cue.Denied -> listOf(Tone(300f, 50, .25f))
                Cue.Warning -> listOf(Tone(1200f, 25, .25f))
                Cue.Incoming -> emptyList()
                Cue.Sos -> listOf(Tone(900f, 150, .5f), Tone(900f, 150, .5f))
            }
        }
        if (tones.isNotEmpty()) scope.launch { synth(tones) }
    }

    private fun haptic(c: Cue) {
        val eff = when (c) {
            Cue.Granted -> VibrationEffect.createPredefined(VibrationEffect.EFFECT_CLICK)
            Cue.Released -> VibrationEffect.createPredefined(VibrationEffect.EFFECT_TICK)
            Cue.Denied -> VibrationEffect.createWaveform(longArrayOf(0, 40, 60, 40), -1)
            Cue.Warning -> VibrationEffect.createPredefined(VibrationEffect.EFFECT_DOUBLE_CLICK)
            Cue.Incoming -> VibrationEffect.createPredefined(VibrationEffect.EFFECT_TICK)
            Cue.Sos -> VibrationEffect.createWaveform(longArrayOf(0, 150, 100, 150, 100, 150), -1)
        }
        runCatching { vib.vibrate(eff) }
    }

    private data class Tone(val hz: Float, val ms: Int, val gain: Float)

    private fun synth(tones: List<Tone>) {
        val sr = 48_000
        val total = tones.sumOf { it.ms } * sr / 1000
        val pcm = ShortArray(total)
        var i = 0
        for (t in tones) {
            val n = t.ms * sr / 1000
            for (k in 0 until n) {
                val env = when {
                    k < n * 0.1 -> k / (n * 0.1)
                    k > n * 0.7 -> (n - k) / (n * 0.3)
                    else -> 1.0
                }
                pcm[i++] = (sin(2 * PI * t.hz * k / sr) * env * t.gain * 32767).toInt().toShort()
            }
        }
        val track = AudioTrack.Builder()
            .setAudioAttributes(AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_ASSISTANCE_SONIFICATION).setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION).build())
            .setAudioFormat(AudioFormat.Builder().setSampleRate(sr).setEncoding(AudioFormat.ENCODING_PCM_16BIT).setChannelMask(AudioFormat.CHANNEL_OUT_MONO).build())
            .setBufferSizeInBytes(pcm.size * 2)
            .setTransferMode(AudioTrack.MODE_STATIC)
            .build()
        track.write(pcm, 0, pcm.size)
        track.play()
        Thread.sleep((total * 1000L / sr) + 20)
        track.release()
    }
}
