package ro.titi.tv

import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager
import android.provider.Settings as SysSettings
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.runBlocking
import ro.titi.app.core.EngineHost
import ro.titi.app.data.Prefs

/** TV process: one Rust engine node, shared by the activity and the radio service. */
class TvApp : Application() {
    lateinit var prefs: Prefs
        private set
    lateinit var engine: EngineHost
        private set
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    override fun onCreate() {
        super.onCreate()
        prefs = Prefs(this)
        autoOnboard()
        engine = EngineHost(this, prefs)
        createChannels()
    }

    /** TVs have no comfortable keyboard: name the node after the device once, before the engine reads it. */
    private fun autoOnboard() {
        if (prefs.settingsNow().onboarded) return
        val device = runCatching { SysSettings.Global.getString(contentResolver, SysSettings.Global.DEVICE_NAME) }.getOrNull()
        val name = device?.takeIf { it.isNotBlank() } ?: getString(R.string.default_tv_name)
        runBlocking { prefs.update { it.copy(name = name, hue = 200, onboarded = true) } }
    }

    private fun createChannels() {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL_RADIO, getString(R.string.notif_channel), NotificationManager.IMPORTANCE_LOW).apply { setShowBadge(false) })
        nm.createNotificationChannel(NotificationChannel(CHANNEL_ALERTS, getString(R.string.notif_alerts), NotificationManager.IMPORTANCE_HIGH))
    }

    companion object {
        const val CHANNEL_RADIO = "radio"
        const val CHANNEL_ALERTS = "alerts"
    }
}
