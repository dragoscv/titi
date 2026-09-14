package ro.titi.app.audio

import android.annotation.SuppressLint
import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioRecord
import android.media.AudioTrack
import android.media.MediaRecorder
import android.media.audiofx.AcousticEchoCanceler
import android.media.audiofx.AutomaticGainControl
import android.media.audiofx.NoiseSuppressor
import android.os.Process
import android.util.Log
import java.util.concurrent.atomic.AtomicBoolean

/**
 * 48 kHz mono PCM16 in/out in 20 ms ticks (960 samples), matching
 * `titi_core::audio::TICK_SAMPLES`. Capture runs on its own thread and hands
 * frames to [onPcm]; playback is fed by the engine's `Play` actions through a
 * lock-free-ish ring so the Rust mixer stays the single source of timing.
 *
 * Uses VOICE_COMMUNICATION (hardware AEC/NS when available) + low-latency
 * AudioTrack, and MODE_IN_COMMUNICATION for correct speaker/BT-headset routing.
 */
class AudioEngine(private val ctx: Context, private val onPcm: (ShortArray, Long) -> Unit) {
    private val am = ctx.getSystemService(AudioManager::class.java)
    private var record: AudioRecord? = null
    private var track: AudioTrack? = null
    private var captureThread: Thread? = null
    private var playThread: Thread? = null
    private val capturing = AtomicBoolean(false)
    private val playing = AtomicBoolean(false)
    private val ring = PcmRing(SAMPLE_RATE * 2) // 2 s

    private var aec: AcousticEchoCanceler? = null
    private var ns: NoiseSuppressor? = null
    private var agc: AutomaticGainControl? = null

    @Volatile var muted = false
    @Volatile var speakerphone = true
        set(v) { field = v; applyRouting() }

    fun startPlayback() {
        if (playing.getAndSet(true)) return
        am.mode = AudioManager.MODE_IN_COMMUNICATION
        applyRouting()
        val minBuf = AudioTrack.getMinBufferSize(SAMPLE_RATE, AudioFormat.CHANNEL_OUT_MONO, AudioFormat.ENCODING_PCM_16BIT)
        val t = AudioTrack.Builder()
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_VOICE_COMMUNICATION)
                    .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                    .build(),
            )
            .setAudioFormat(
                AudioFormat.Builder().setSampleRate(SAMPLE_RATE).setEncoding(AudioFormat.ENCODING_PCM_16BIT).setChannelMask(AudioFormat.CHANNEL_OUT_MONO).build(),
            )
            .setBufferSizeInBytes(maxOf(minBuf, TICK_SAMPLES * 2 * 4))
            .setPerformanceMode(AudioTrack.PERFORMANCE_MODE_LOW_LATENCY)
            .setTransferMode(AudioTrack.MODE_STREAM)
            .build()
        track = t
        t.play()
        playThread = Thread({
            Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_AUDIO)
            val buf = ShortArray(TICK_SAMPLES)
            while (playing.get()) {
                val n = ring.read(buf)
                if (n < buf.size) java.util.Arrays.fill(buf, n, buf.size, 0)
                t.write(buf, 0, buf.size, AudioTrack.WRITE_BLOCKING)
            }
        }, "titi-play").also { it.start() }
    }

    fun stopPlayback() {
        if (!playing.getAndSet(false)) return
        playThread?.join(500)
        track?.let { runCatching { it.stop() }; it.release() }
        track = null
        if (!capturing.get()) am.mode = AudioManager.MODE_NORMAL
    }

    /** Called from the engine thread with mixed PCM from `Action::Play`. */
    fun play(pcm: ShortArray) {
        ring.write(pcm)
    }

    @SuppressLint("MissingPermission")
    fun startCapture() {
        if (capturing.getAndSet(true)) return
        am.mode = AudioManager.MODE_IN_COMMUNICATION
        val minBuf = AudioRecord.getMinBufferSize(SAMPLE_RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
        val r = AudioRecord.Builder()
            .setAudioSource(MediaRecorder.AudioSource.VOICE_COMMUNICATION)
            .setAudioFormat(
                AudioFormat.Builder().setSampleRate(SAMPLE_RATE).setEncoding(AudioFormat.ENCODING_PCM_16BIT).setChannelMask(AudioFormat.CHANNEL_IN_MONO).build(),
            )
            .setBufferSizeInBytes(maxOf(minBuf, TICK_SAMPLES * 2 * 4))
            .build()
        if (r.state != AudioRecord.STATE_INITIALIZED) {
            Log.e(TAG, "AudioRecord init failed"); capturing.set(false); r.release(); return
        }
        record = r
        val sid = r.audioSessionId
        if (AcousticEchoCanceler.isAvailable()) aec = AcousticEchoCanceler.create(sid)?.apply { enabled = true }
        if (NoiseSuppressor.isAvailable()) ns = NoiseSuppressor.create(sid)?.apply { enabled = true }
        if (AutomaticGainControl.isAvailable()) agc = AutomaticGainControl.create(sid)?.apply { enabled = true }
        r.startRecording()
        captureThread = Thread({
            Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_AUDIO)
            val buf = ShortArray(TICK_SAMPLES)
            while (capturing.get()) {
                var got = 0
                while (got < buf.size && capturing.get()) {
                    val n = r.read(buf, got, buf.size - got, AudioRecord.READ_BLOCKING)
                    if (n <= 0) break
                    got += n
                }
                if (got == buf.size) {
                    if (muted) java.util.Arrays.fill(buf, 0)
                    onPcm(buf.copyOf(), System.currentTimeMillis())
                }
            }
        }, "titi-capture").also { it.start() }
    }

    fun stopCapture() {
        if (!capturing.getAndSet(false)) return
        captureThread?.join(500)
        aec?.release(); ns?.release(); agc?.release()
        aec = null; ns = null; agc = null
        record?.let { runCatching { it.stop() }; it.release() }
        record = null
        if (!playing.get()) am.mode = AudioManager.MODE_NORMAL
    }

    @Suppress("DEPRECATION")
    private fun applyRouting() {
        // Prefer a connected BT/wired headset; otherwise speakerphone (walkie-talkie default).
        val devices = am.availableCommunicationDevices
        val headset = devices.firstOrNull {
            it.type == android.media.AudioDeviceInfo.TYPE_BLUETOOTH_SCO || it.type == android.media.AudioDeviceInfo.TYPE_WIRED_HEADSET ||
                it.type == android.media.AudioDeviceInfo.TYPE_WIRED_HEADPHONES || it.type == android.media.AudioDeviceInfo.TYPE_USB_HEADSET ||
                it.type == android.media.AudioDeviceInfo.TYPE_BLE_HEADSET
        }
        val target = headset ?: if (speakerphone) devices.firstOrNull { it.type == android.media.AudioDeviceInfo.TYPE_BUILTIN_SPEAKER } else devices.firstOrNull { it.type == android.media.AudioDeviceInfo.TYPE_BUILTIN_EARPIECE }
        target?.let { runCatching { am.setCommunicationDevice(it) } }
    }

    fun release() {
        stopCapture(); stopPlayback()
        runCatching { am.clearCommunicationDevice() }
    }

    companion object {
        private const val TAG = "AudioEngine"
        const val SAMPLE_RATE = 48_000
        const val TICK_SAMPLES = 960
    }
}

/** Single-producer/single-consumer short ring buffer. */
class PcmRing(capacity: Int) {
    private val buf = ShortArray(capacity)
    @Volatile private var head = 0 // write
    @Volatile private var tail = 0 // read

    @Synchronized fun write(src: ShortArray) {
        for (s in src) {
            buf[head] = s
            head = (head + 1) % buf.size
            if (head == tail) tail = (tail + 1) % buf.size // overrun: drop oldest
        }
    }

    @Synchronized fun read(dst: ShortArray): Int {
        var n = 0
        while (n < dst.size && tail != head) {
            dst[n++] = buf[tail]
            tail = (tail + 1) % buf.size
        }
        return n
    }
}
