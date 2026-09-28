package ro.titi.app.service

import android.content.Context
import android.content.Intent
import android.support.v4.media.session.MediaSessionCompat
import android.support.v4.media.session.PlaybackStateCompat
import android.view.KeyEvent

/**
 * Receives Bluetooth-headset / wired-headset button presses with the screen off
 * (an Activity only gets them while focused). A press toggles talk.
 */
class MediaButtons(ctx: Context, private val onToggle: () -> Unit) {
    private val session = MediaSessionCompat(ctx, "titi-ptt").apply {
        setCallback(object : MediaSessionCompat.Callback() {
            override fun onMediaButtonEvent(intent: Intent): Boolean {
                @Suppress("DEPRECATION")
                val ev = intent.getParcelableExtra<KeyEvent>(Intent.EXTRA_KEY_EVENT) ?: return false
                val k = ev.keyCode
                if (k != KeyEvent.KEYCODE_HEADSETHOOK && k != KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE &&
                    k != KeyEvent.KEYCODE_MEDIA_PLAY && k != KeyEvent.KEYCODE_MEDIA_PAUSE
                ) return false
                if (ev.action == KeyEvent.ACTION_DOWN && ev.repeatCount == 0) onToggle()
                return true
            }
        })
        setPlaybackState(
            PlaybackStateCompat.Builder()
                .setActions(PlaybackStateCompat.ACTION_PLAY_PAUSE or PlaybackStateCompat.ACTION_PLAY or PlaybackStateCompat.ACTION_PAUSE)
                .setState(PlaybackStateCompat.STATE_PAUSED, 0, 1f)
                .build(),
        )
        isActive = true
    }

    fun release() {
        session.isActive = false
        session.release()
    }
}
