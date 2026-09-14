package ro.titi.app.audio

import android.annotation.SuppressLint
import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.AudioTrack
import android.media.MediaRecorder
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import ro.titi.app.core.toLeBytes
import ro.titi.app.core.toShorts
import uniffi.titi_ffi.FfiProfile
import uniffi.titi_ffi.decodeVoiceNote
import uniffi.titi_ffi.encodeVoiceNote
import java.io.ByteArrayOutputStream
import java.util.concurrent.atomic.AtomicBoolean

/** Records 48 kHz mono PCM, encodes to Opus via the core on stop. Max 60 s. */
class VoiceNoteRecorder(@Suppress("unused") private val ctx: Context) {
    private var rec: AudioRecord? = null
    private var thread: Thread? = null
    private val running = AtomicBoolean(false)
    private val pcm = ByteArrayOutputStream()
    private var startMs = 0L

    @SuppressLint("MissingPermission")
    fun start() {
        if (running.getAndSet(true)) return
        pcm.reset(); startMs = System.currentTimeMillis()
        val min = AudioRecord.getMinBufferSize(48_000, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
        val r = AudioRecord(MediaRecorder.AudioSource.VOICE_RECOGNITION, 48_000, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT, maxOf(min, 19200))
        if (r.state != AudioRecord.STATE_INITIALIZED) { running.set(false); return }
        rec = r; r.startRecording()
        thread = Thread {
            val buf = ShortArray(960)
            while (running.get() && System.currentTimeMillis() - startMs < 60_000) {
                val n = r.read(buf, 0, buf.size, AudioRecord.READ_BLOCKING)
                if (n > 0) synchronized(pcm) { pcm.write(buf.copyOf(n).toLeBytes()) }
            }
        }.also { it.start() }
    }

    /** @return (opus packets, duration ms, profile) or null if too short */
    fun stop(): Triple<ByteArray, Int, FfiProfile>? {
        if (!running.getAndSet(false)) return null
        thread?.join(500)
        rec?.let { runCatching { it.stop() }; it.release() }; rec = null
        val ms = (System.currentTimeMillis() - startMs).toInt()
        if (ms < 400) return null
        val bytes = synchronized(pcm) { pcm.toByteArray() }
        val packets = runCatching { encodeVoiceNote(bytes, FfiProfile.STD) }.getOrNull() ?: return null
        return Triple(packets, ms, FfiProfile.STD)
    }
}

object VoiceNotePlayer {
    private val scope = CoroutineScope(Dispatchers.IO)
    fun play(packets: ByteArray, profile: FfiProfile) = scope.launch {
        val pcm = runCatching { decodeVoiceNote(packets, profile) }.getOrNull()?.toShorts() ?: return@launch
        val t = AudioTrack.Builder()
            .setAudioAttributes(AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA).setContentType(AudioAttributes.CONTENT_TYPE_SPEECH).build())
            .setAudioFormat(AudioFormat.Builder().setSampleRate(48_000).setEncoding(AudioFormat.ENCODING_PCM_16BIT).setChannelMask(AudioFormat.CHANNEL_OUT_MONO).build())
            .setBufferSizeInBytes(pcm.size * 2).setTransferMode(AudioTrack.MODE_STATIC).build()
        t.write(pcm, 0, pcm.size); t.play()
        Thread.sleep(pcm.size * 1000L / 48_000 + 50); t.release()
    }
}
