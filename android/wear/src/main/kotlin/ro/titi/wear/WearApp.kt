package ro.titi.wear

import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import ro.titi.app.core.EngineHost
import ro.titi.app.data.Prefs

/** Watch process: one Rust engine node, shared by activity, service, tile and complication. */
class WearApp : Application() {
    lateinit var prefs: Prefs
        private set
    lateinit var engine: EngineHost
        private set
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    /** Set when the phone pushed an invite; the UI shows a confirmation. */
    val pendingFromPhone = kotlinx.coroutines.flow.MutableStateFlow(false)

    override fun onCreate() {
        super.onCreate()
        instance = this
        prefs = Prefs(this)
        engine = EngineHost(this, prefs)
        createChannels()
        ProcessLifecycleOwner.get().lifecycle.addObserver(object : DefaultLifecycleObserver {
            override fun onStart(owner: LifecycleOwner) { foreground = true }
            override fun onStop(owner: LifecycleOwner) { foreground = false }
        })
        refreshSurfacesOnChange()
        ro.titi.app.util.RadioLifecycle.install(prefs) { RadioService.stop(this) }
    }

    /** Tile + complication follow the radio state (debounced: both are IPC to the system UI). */
    @OptIn(FlowPreview::class)
    private fun refreshSurfacesOnChange() {
        scope.launch {
            engine.state
                .map { s -> Triple(s.active?.name, s.active?.floor, s.active?.members?.size to s.peers.size) to (s.active?.talkerName to s.sosGroup) }
                .distinctUntilChanged()
                .debounce(750)
                .collect {
                    TalkTileService.requestUpdate(this@WearApp)
                    GroupComplicationService.requestUpdate(this@WearApp)
                }
        }
    }

    private fun createChannels() {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL_RADIO, getString(R.string.notif_channel), NotificationManager.IMPORTANCE_LOW).apply { setShowBadge(false) })
        nm.createNotificationChannel(NotificationChannel(CHANNEL_ALERTS, getString(R.string.notif_alerts), NotificationManager.IMPORTANCE_HIGH))
    }

    companion object {
        const val CHANNEL_RADIO = "radio"
        const val CHANNEL_ALERTS = "alerts"
        @Volatile var foreground = false
            private set
        lateinit var instance: WearApp
            private set
    }
}
