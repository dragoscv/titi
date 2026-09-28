package ro.titi.wear

import android.util.Log
import com.google.android.gms.wearable.MessageEvent
import com.google.android.gms.wearable.WearableListenerService
import kotlinx.coroutines.launch
import org.json.JSONObject

/**
 * Phone → watch provisioning over the Wearable Data Layer (gms only).
 *
 * `/titi/join`    payload = a `titi://j/…` deep link minted by the phone for one group
 * `/titi/profile` payload = JSON { name, hue } so the watch shows up as the same person
 *
 * The watch is its own node (own identity); the link lets it join like any invitee.
 * Joining needs no microphone, so this is allowed from the background.
 */
class PhoneBridgeService : WearableListenerService() {
    override fun onMessageReceived(event: MessageEvent) {
        val app = application as WearApp
        val body = String(event.data, Charsets.UTF_8)
        when (event.path) {
            PATH_JOIN -> {
                if (!body.startsWith("titi://j/") && !body.startsWith("https://titi.app/j/")) { Log.w(TAG, "ignored non-invite payload"); return }
                app.engine.joinByLink(body)
                app.pendingFromPhone.value = true
            }
            PATH_PROFILE -> runCatching {
                val j = JSONObject(body)
                val name = j.optString("name").take(40)
                val hue = j.optInt("hue", 40).coerceIn(0, 359)
                app.scope.launch {
                    app.prefs.update { s -> s.copy(name = if (s.name.isBlank()) "$name ⌚" else s.name, hue = hue, onboarded = true) }
                    app.engine.setDisplayName(app.prefs.settingsNow().name, hue)
                }
            }.onFailure { Log.w(TAG, "profile", it) }
        }
    }

    companion object {
        private const val TAG = "PhoneBridge"
        const val PATH_JOIN = "/titi/join"
        const val PATH_PROFILE = "/titi/profile"
    }
}
