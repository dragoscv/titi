package ro.titi.app.service

import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.lifecycle.LifecycleService
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import ro.titi.app.MainActivity
import ro.titi.app.R
import ro.titi.app.TitiApp
import ro.titi.app.core.FloorState
import ro.titi.app.core.RadioState
import ro.titi.app.transport.BleTransport
import ro.titi.app.transport.LanTransport
import ro.titi.app.transport.RelayTransport
import ro.titi.app.transport.Transport

/**
 * Foreground service that keeps the radio alive with the screen off:
 * owns the transports, a partial wake lock and a Wi-Fi low-latency lock, and
 * shows a media-style notification with a Talk action (A-11, A-22).
 */
class RadioService : LifecycleService() {
    private lateinit var app: TitiApp
    private var wakeLock: PowerManager.WakeLock? = null
    private var wifiLock: WifiManager.WifiLock? = null

    override fun onCreate() {
        super.onCreate()
        android.util.Log.i("RadioService", "onCreate")
        app = application as TitiApp
        startForeground(NOTIF_ID, buildNotification(app.engine.state.value))
        acquireLocks()
        app.engine.start(buildTransports())
        lifecycleScope.launch {
            app.engine.state.collectLatest { s ->
                ServiceCompat.startForeground(this@RadioService, NOTIF_ID, buildNotification(s), fgsType())
            }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        super.onStartCommand(intent, flags, startId)
        when (intent?.action) {
            ACTION_STOP -> { stopSelf(); return START_NOT_STICKY }
            ACTION_PTT_TOGGLE -> {
                val e = app.engine
                if (e.state.value.active?.floor == FloorState.Talking) e.pttUp() else e.pttDown()
            }
            ACTION_MUTE_TOGGLE -> app.engine.setMuted(!app.engine.state.value.muted)
        }
        return START_STICKY
    }

    override fun onDestroy() {
        app.engine.stop()
        wakeLock?.let { if (it.isHeld) it.release() }
        wifiLock?.let { if (it.isHeld) it.release() }
        super.onDestroy()
    }

    override fun onBind(intent: Intent): IBinder? { super.onBind(intent); return null }

    private fun buildTransports(): List<Transport> {
        val s = app.prefs.settingsNow()
        val list = mutableListOf<Transport>()
        list += LanTransport(this)
        if (s.useBle) list += BleTransport(this) { app.engine.engine.nodeId() }
        if (s.useInternet) list += RelayTransport({ app.prefs.settingsNow().relayUrl }, { app.engine.engine.nodeId() }, { app.prefs.settingsNow().name }, { app.prefs.settingsNow().hue })
        return list
    }

    private fun fgsType(): Int =
        if (Build.VERSION.SDK_INT >= 34) ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE or ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE
        else if (Build.VERSION.SDK_INT >= 29) ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE else 0

    private fun acquireLocks() {
        val pm = getSystemService(PowerManager::class.java)
        wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "titi:radio").apply { acquire() }
        val wm = applicationContext.getSystemService(WifiManager::class.java)
        wifiLock = wm.createWifiLock(if (Build.VERSION.SDK_INT >= 29) WifiManager.WIFI_MODE_FULL_LOW_LATENCY else WifiManager.WIFI_MODE_FULL_HIGH_PERF, "titi:wifi").apply { acquire() }
    }

    private fun buildNotification(s: RadioState): Notification {
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val g = s.active
        val title = when {
            g == null -> getString(R.string.notif_idle, s.peers.size)
            g.floor == FloorState.Busy && g.talkerName != null -> getString(R.string.notif_talking, g.talkerName)
            g.floor == FloorState.Talking -> getString(R.string.group_talking)
            else -> getString(R.string.notif_listening, g.name)
        }
        val talkLabel = if (g?.floor == FloorState.Talking) getString(R.string.notif_action_stop) else getString(R.string.notif_action_talk)
        return NotificationCompat.Builder(this, TitiApp.CHANNEL_RADIO)
            .setSmallIcon(R.drawable.ic_launcher_monochrome)
            .setContentTitle(title)
            .setContentText(g?.let { "${it.members.size} · ${it.link?.name?.lowercase() ?: ""}".trim(' ', '·') })
            .setContentIntent(open)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setColor(getColor(R.color.amber))
            .addAction(0, talkLabel, action(ACTION_PTT_TOGGLE, 1))
            .addAction(0, if (s.muted) getString(R.string.group_unmute) else getString(R.string.notif_action_mute), action(ACTION_MUTE_TOGGLE, 2))
            .addAction(0, getString(R.string.notif_action_stop), action(ACTION_STOP, 3))
            .setStyle(androidx.media.app.NotificationCompat.MediaStyle().setShowActionsInCompactView(0, 1))
            .build()
    }

    private fun action(a: String, code: Int): PendingIntent =
        PendingIntent.getService(this, code, Intent(this, RadioService::class.java).setAction(a), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)

    companion object {
        const val NOTIF_ID = 1
        const val ACTION_STOP = "ro.titi.app.STOP"
        const val ACTION_PTT_TOGGLE = "ro.titi.app.PTT_TOGGLE"
        const val ACTION_MUTE_TOGGLE = "ro.titi.app.MUTE_TOGGLE"

        fun start(ctx: Context) {
            ctx.startForegroundService(Intent(ctx, RadioService::class.java))
        }
        fun stop(ctx: Context) {
            ctx.startService(Intent(ctx, RadioService::class.java).setAction(ACTION_STOP))
        }
    }
}
