package ro.titi.app

import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager
import ro.titi.app.core.EngineHost
import ro.titi.app.data.Prefs

class TitiApp : Application() {
    lateinit var prefs: Prefs
        private set
    lateinit var engine: EngineHost
        private set

    override fun onCreate() {
        super.onCreate()
        instance = this
        prefs = Prefs(this)
        engine = EngineHost(this, prefs)
        createChannels()
        androidx.lifecycle.ProcessLifecycleOwner.get().lifecycle.addObserver(object : androidx.lifecycle.DefaultLifecycleObserver {
            override fun onStart(owner: androidx.lifecycle.LifecycleOwner) { foreground = true }
            override fun onStop(owner: androidx.lifecycle.LifecycleOwner) { foreground = false }
        })
    }

    private fun createChannels() {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL_RADIO, getString(R.string.notif_channel_radio), NotificationManager.IMPORTANCE_LOW).apply {
                description = getString(R.string.notif_channel_radio_desc)
                setShowBadge(false)
            },
        )
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL_ALERTS, getString(R.string.notif_channel_alerts), NotificationManager.IMPORTANCE_HIGH).apply {
                description = getString(R.string.notif_channel_alerts_desc)
            },
        )
    }

    companion object {
        const val CHANNEL_RADIO = "radio"
        const val CHANNEL_ALERTS = "alerts"
        @Volatile var foreground = false
            private set
        lateinit var instance: TitiApp
            private set
    }
}
