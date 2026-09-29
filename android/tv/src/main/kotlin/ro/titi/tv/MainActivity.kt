package ro.titi.tv

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import ro.titi.app.core.FloorState
import ro.titi.tv.ui.TitiTvTheme
import ro.titi.tv.ui.TvRoot

/**
 * 10-foot shell. D-pad only: ↑/↓ groups, → actions, hold OK on Talk (or Play/Pause, red key) = talk.
 * Talking needs a real capture device; most TVs have none, so they act as a room speaker.
 */
class MainActivity : ComponentActivity() {
    private val app get() = application as TvApp
    /** Re-evaluated on resume: a USB/BT mic can be plugged in while we're in the background. */
    val micUsable = MutableStateFlow(false)
    /** Local one-line hints (no mic, joining…) shown in the toast slot. */
    val hints = MutableSharedFlow<Int>(extraBufferCapacity = 4)
    private var mediaKeyTalking = false

    private val permLauncher = registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {
        micUsable.value = Mic.usable(this)
        RadioService.start(this, RadioService.ACTION_REFRESH)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        micUsable.value = Mic.usable(this)
        setContent { TitiTvTheme { TvRoot(app.engine, app.prefs, this) } }
        if (savedInstanceState == null) askPermissions()
    }

    override fun onResume() {
        super.onResume()
        micUsable.value = Mic.usable(this)
        if (!app.engine.state.value.running) RadioService.start(this)
    }

    override fun onPause() {
        super.onPause()
        // never leave the floor held because a key-up was lost to another window
        mediaKeyTalking = false
        if (app.engine.state.value.active?.floor == FloorState.Talking && !isChangingConfigurations) app.engine.pttUp()
    }

    fun talkDown() {
        val g = app.engine.state.value.active ?: return
        if (!micUsable.value) { hints.tryEmit(R.string.no_mic_hint); return }
        if (g.fullDuplex) app.engine.setMuted(!app.engine.state.value.muted) else app.engine.pttDown()
    }

    fun talkUp() {
        val g = app.engine.state.value.active ?: return
        if (micUsable.value && !g.fullDuplex) app.engine.pttUp()
    }

    /** Play/Pause and the red key talk from anywhere on screen. */
    private fun isTalkMediaKey(code: Int) = code == KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE || code == KeyEvent.KEYCODE_PROG_RED

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        if (!isTalkMediaKey(keyCode)) return super.onKeyDown(keyCode, event)
        if (event.repeatCount == 0 && !mediaKeyTalking) { mediaKeyTalking = true; talkDown() }
        return true
    }

    override fun onKeyUp(keyCode: Int, event: KeyEvent): Boolean {
        if (!isTalkMediaKey(keyCode)) return super.onKeyUp(keyCode, event)
        if (mediaKeyTalking) { mediaKeyTalking = false; talkUp() }
        return true
    }

    private fun askPermissions() {
        val want = buildList {
            if (Mic.hasInput(this@MainActivity)) add(Manifest.permission.RECORD_AUDIO)
            if (Build.VERSION.SDK_INT >= 33) add(Manifest.permission.POST_NOTIFICATIONS)
            if (Build.VERSION.SDK_INT >= 31 && packageManager.hasSystemFeature(PackageManager.FEATURE_BLUETOOTH_LE)) {
                add(Manifest.permission.BLUETOOTH_SCAN); add(Manifest.permission.BLUETOOTH_ADVERTISE); add(Manifest.permission.BLUETOOTH_CONNECT)
            }
        }.filter { ContextCompat.checkSelfPermission(this, it) != PackageManager.PERMISSION_GRANTED }
        if (want.isNotEmpty()) permLauncher.launch(want.toTypedArray())
    }
}
