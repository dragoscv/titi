package ro.titi.tv

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.PowerManager
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.lifecycle.LifecycleService
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import ro.titi.app.core.Alert
import ro.titi.app.core.FloorState
import ro.titi.app.core.RadioState
import ro.titi.app.transport.BleTransport
import ro.titi.app.transport.LanTransport
import ro.titi.app.transport.LinkIds
import ro.titi.app.transport.RelayTransport
import ro.titi.app.transport.Transport
import ro.titi.app.util.Notify

/**
 * Keeps the room radio alive while another app is on screen. A TV is primarily a speaker:
 * the service is mediaPlayback|connectedDevice, plus microphone only when a capture device exists.
 * Started from [MainActivity.onResume] (a visible activity may start any FGS type).
 */
class RadioService : LifecycleService() {
    private val app get() = application as TvApp
    private var started = false
    private var wakeLock: PowerManager.WakeLock? = null

    override fun onCreate() {
        super.onCreate()
        val ok = runCatching { ServiceCompat.startForeground(this, NOTIF_ID, buildNotification(app.engine.state.value).build(), fgsType()) }
            .onFailure { Log.w(TAG, "startForeground refused", it) }.isSuccess
        if (!ok) { stopSelf(); return }
        started = true
        app.engine.start(buildTransports())
        lifecycleScope.launch {
            app.engine.state
                .map { s -> listOf(s.active?.id, s.active?.name, s.active?.floor, s.active?.talkerName, s.active?.members?.size, s.muted) }
                .distinctUntilChanged()
                .collect { updateNotification(app.engine.state.value) }
        }
        lifecycleScope.launch {
            app.engine.state.map { it.groups.isNotEmpty() }.distinctUntilChanged().collect { setWakeLock(it) }
        }
        lifecycleScope.launch { app.engine.alerts.collect { postAlert(it) } }
        lifecycleScope.launch {
            app.prefs.settings.map { it.useBle to it.useInternet }.distinctUntilChanged().drop(1).collect { (ble, net) ->
                app.engine.setTransport(LinkIds.BLE_L2CAP, if (ble && Mic.bleUsable(this@RadioService)) ble() else null)
                app.engine.setTransport(LinkIds.INTERNET, if (net) relay() else null)
            }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        super.onStartCommand(intent, flags, startId)
        if (!started) { stopSelf(); return START_NOT_STICKY }
        when (intent?.action) {
            ACTION_STOP -> stopSelf()
            ACTION_REFRESH -> refresh()
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        if (started) {
            app.engine.stop()
            setWakeLock(false)
        }
        super.onDestroy()
    }

    /** Permissions changed (BT granted, mic granted): re-evaluate BLE and the FGS type. */
    private fun refresh() {
        val s = app.prefs.settingsNow()
        if (s.useBle && Mic.bleUsable(this) && app.engine.transport(LinkIds.BLE_L2CAP) == null) app.engine.setTransport(LinkIds.BLE_L2CAP, ble())
        runCatching { ServiceCompat.startForeground(this, NOTIF_ID, buildNotification(app.engine.state.value).build(), fgsType()) }
            .onFailure { Log.w(TAG, "fgs type update refused", it) }
    }

    private fun ble() = BleTransport(this) { app.engine.engine.nodeId() }
    private fun relay() = RelayTransport({ app.prefs.settingsNow().relayUrl }, { app.engine.engine.nodeId() }, { app.prefs.settingsNow().name }, { app.prefs.settingsNow().hue })

    private fun buildTransports(): List<Transport> {
        val s = app.prefs.settingsNow()
        return buildList {
            add(LanTransport(this@RadioService))
            if (s.useBle && Mic.bleUsable(this@RadioService)) add(ble())
            if (s.useInternet) add(relay())
        }
    }

    private fun setWakeLock(on: Boolean) {
        if (on) {
            val wl = wakeLock ?: getSystemService(PowerManager::class.java).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "titi:tv").apply { setReferenceCounted(false) }.also { wakeLock = it }
            if (!wl.isHeld) wl.acquire()
        } else wakeLock?.let { if (it.isHeld) it.release() }
    }

    private fun fgsType(): Int {
        if (Build.VERSION.SDK_INT < 29) return 0
        var t = ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK or ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE
        if (Build.VERSION.SDK_INT >= 30 && Mic.usable(this)) t = t or ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE
        return t
    }

    private fun openIntent(): PendingIntent =
        PendingIntent.getActivity(this, 100, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)

    private fun buildNotification(s: RadioState): NotificationCompat.Builder {
        val g = s.active
        val title = when {
            g == null -> getString(R.string.notif_radio_on)
            g.floor == FloorState.Busy && g.talkerName != null -> getString(R.string.notif_talking, g.talkerName)
            g.floor == FloorState.Talking -> getString(R.string.you_are_talking)
            else -> getString(R.string.notif_listening, g.name)
        }
        return NotificationCompat.Builder(this, TvApp.CHANNEL_RADIO)
            .setSmallIcon(R.drawable.ic_launcher_monochrome)
            .setContentTitle(title)
            .setContentText(g?.let { resources.getQuantityString(R.plurals.members_count, it.members.size, it.members.size) })
            .setContentIntent(openIntent())
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setColor(getColor(R.color.amber))
    }

    private fun updateNotification(s: RadioState) {
        Notify.post(this, NOTIF_ID, buildNotification(s).build())
    }

    private fun postAlert(a: Alert) {
        if (a !is Alert.Sos) return // invites are shown in the TV UI itself
        val b = NotificationCompat.Builder(this, TvApp.CHANNEL_ALERTS)
            .setSmallIcon(R.drawable.ic_launcher_monochrome)
            .setContentIntent(openIntent())
            .setAutoCancel(true)
            .setColor(getColor(R.color.amber))
            .setContentTitle(if (a.cancelled) getString(R.string.sos_cancelled, a.fromName) else getString(R.string.sos_from, a.fromName))
            .setContentText(a.groupName)
            .setPriority(if (a.cancelled) NotificationCompat.PRIORITY_DEFAULT else NotificationCompat.PRIORITY_MAX)
            .setCategory(NotificationCompat.CATEGORY_ALARM)
        Notify.post(this, 1000 + a.group.hashCode(), b.build())
    }

    companion object {
        private const val TAG = "TvRadio"
        const val NOTIF_ID = 1
        const val ACTION_STOP = "ro.titi.tv.STOP"
        const val ACTION_REFRESH = "ro.titi.tv.REFRESH"

        fun start(ctx: Context, action: String? = null) {
            runCatching { ctx.startForegroundService(Intent(ctx, RadioService::class.java).setAction(action)) }
                .onFailure { Log.w(TAG, "start refused", it) }
        }
    }
}
