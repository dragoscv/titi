package ro.titi.app

import android.content.Intent
import android.os.Bundle
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.core.view.WindowCompat
import ro.titi.app.audio.Cues
import ro.titi.app.core.FloorState
import ro.titi.app.service.RadioService
import ro.titi.app.ui.Permissions
import ro.titi.app.ui.TitiApp as TitiRoot
import ro.titi.app.ui.theme.TitiTheme

class MainActivity : ComponentActivity() {
    private val app get() = application as TitiApp
    private var volumePtt = true
    private var volumeHeld = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        WindowCompat.setDecorFitsSystemWindows(window, false)
        setShowWhenLocked(true)
        handleIntent(intent)
        setContent {
            val settingsOrNull by app.prefs.settings.collectAsState(initial = null)
            val settings = settingsOrNull ?: return@setContent // keep splash until DataStore has emitted
            volumePtt = settings.volumePtt
            val ctx = LocalContext.current
            val cues = remember { Cues(ctx) }
            cues.pack = settings.soundPack
            cues.hapticsOnly = settings.hapticsOnly
            LaunchedEffect(Unit) { app.engine.cues.collect { cues.play(it) } }
            LaunchedEffect(settings.onboarded) {
                val ok = Permissions.essentialGranted(ctx)
                android.util.Log.i("MainActivity", "onboarded=${settings.onboarded} perms=$ok")
                if (settings.onboarded && ok) RadioService.start(ctx)
            }
            TitiTheme(settings.theme) {
                TitiRoot(app.engine, app.prefs, settings)
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    private fun handleIntent(i: Intent?) {
        val uri = i?.data ?: return
        val link = when {
            uri.scheme == "titi" -> uri.toString()
            uri.host == "titi.app" && uri.path?.startsWith("/j/") == true -> "titi://j/" + uri.path!!.removePrefix("/j/")
            else -> return
        }
        app.engine.joinByLink(link)
        i.data = null
    }

    // Hardware PTT via volume keys (A-21): hold volume-down to talk while a group is active.
    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        if (volumePtt && keyCode == KeyEvent.KEYCODE_VOLUME_DOWN && app.engine.state.value.active != null) {
            if (!volumeHeld && event.repeatCount == 0) { volumeHeld = true; app.engine.pttDown() }
            return true
        }
        if (keyCode == KeyEvent.KEYCODE_HEADSETHOOK || keyCode == KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE) {
            // BT headset button: toggle
            val e = app.engine
            if (e.state.value.active?.floor == FloorState.Talking) e.pttUp() else e.pttDown()
            return true
        }
        return super.onKeyDown(keyCode, event)
    }

    override fun onKeyUp(keyCode: Int, event: KeyEvent): Boolean {
        if (volumeHeld && keyCode == KeyEvent.KEYCODE_VOLUME_DOWN) { volumeHeld = false; app.engine.pttUp(); return true }
        return super.onKeyUp(keyCode, event)
    }
}
