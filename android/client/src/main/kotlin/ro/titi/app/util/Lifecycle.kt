package ro.titi.app.util

import android.app.Activity
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import ro.titi.app.data.Prefs

/**
 * App-wide radio lifecycle shared by phone, watch and TV:
 * - [quit]: stop the radio service (removes the ongoing notification / watch-face icon)
 *   and close every task of the app. Nothing restarts until the user opens the app again.
 * - "Run in background" off: the radio stops as soon as no Titi screen is visible and
 *   starts again when one is.
 */
object RadioLifecycle {
    private var installed = false

    /**
     * Call once from Application.onCreate. `stopService` must stop the FGS (engine stops in its
     * onDestroy). Activities (re)start the service in onResume, so only the stop side lives here.
     */
    fun install(prefs: Prefs, stopService: () -> Unit) {
        if (installed) return
        installed = true
        ProcessLifecycleOwner.get().lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onStop(owner: LifecycleOwner) {
                if (!prefs.settingsNow().keepRunning) stopService()
            }
        })
    }

    fun quit(activity: Activity, stopService: () -> Unit) {
        stopService()
        activity.finishAndRemoveTask()
    }
}




