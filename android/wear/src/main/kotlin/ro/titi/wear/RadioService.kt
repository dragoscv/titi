package ro.titi.wear

import android.app.Notification
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.os.Build
import android.os.PowerManager
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import androidx.lifecycle.LifecycleService
import androidx.lifecycle.lifecycleScope
import androidx.wear.ongoing.OngoingActivity
import androidx.wear.ongoing.Status
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

/**
 * Keeps the watch radio alive with the screen off. Wear OS 6: a microphone FGS may
 * only start while the app is visible, so this is started from [MainActivity] only.
 * Posts an Ongoing Activity (WO-V4) so the session is one tap away from the watch face.
 */
class RadioService : LifecycleService() {
    private val app get() = application as WearApp
    private var started = false
    private var wakeLock: PowerManager.WakeLock? = null
    private var ongoing: OngoingActivity? = null
    private val cm by lazy { getSystemService(ConnectivityManager::class.java) }

    override fun onCreate() {
        super.onCreate()
        val ok = runCatching { ServiceCompat.startForeground(this, NOTIF_ID, buildNotification(app.engine.state.value).build(), fgsType()) }
            .onFailure { Log.w(TAG, "startForeground refused", it) }.isSuccess
        if (!ok) { stopSelf(); return }
        started = true
        app.engine.start(buildTransports())
        if (app.prefs.settingsNow().useWifiVoice) requestWifi()
        lifecycleScope.launch {
            app.engine.state
                .map { s -> listOf(s.active?.id, s.active?.name, s.active?.floor, s.active?.talkerName, s.active?.members?.size, s.peers.size, s.muted) }
                .distinctUntilChanged()
                .collect { updateNotification(app.engine.state.value) }
        }
        lifecycleScope.launch {
            app.engine.state.map { it.groups.isNotEmpty() }.distinctUntilChanged().collect { setWakeLock(it) }
        }
        lifecycleScope.launch { app.engine.alerts.collect { postAlert(it) } }
        lifecycleScope.launch {
            app.prefs.settings.map { Triple(it.useBle, it.useInternet, it.useWifiVoice) }.distinctUntilChanged().drop(1).collect { (ble, net, wifi) ->
                app.engine.setTransport(LinkIds.BLE_L2CAP, if (ble) ble() else null)
                app.engine.setTransport(LinkIds.INTERNET, if (net) relay() else null)
                if (wifi) requestWifi() else releaseWifi()
            }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        super.onStartCommand(intent, flags, startId)
        if (!started) { stopSelf(); return START_NOT_STICKY }
        when (intent?.action) {
            ACTION_STOP -> stopSelf()
            ACTION_PTT_TOGGLE -> toggleTalk(app)
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        if (started) {
            releaseWifi()
            app.engine.stop()
            setWakeLock(false)
        }
        super.onDestroy()
    }

    private fun ble() = BleTransport(this) { app.engine.engine.nodeId() }
    private fun relay() = RelayTransport({ app.prefs.settingsNow().relayUrl }, { app.engine.engine.nodeId() }, { app.prefs.settingsNow().name }, { app.prefs.settingsNow().hue })

    private fun buildTransports(): List<Transport> {
        val s = app.prefs.settingsNow()
        return buildList {
            add(LanTransport(this@RadioService))
            if (s.useBle) add(ble())
            if (s.useInternet) add(relay())
        }
    }

    // ---- Wi-Fi: watches keep Wi-Fi off while the phone link is up; ask for it explicitly ----

    private var wifiCb: ConnectivityManager.NetworkCallback? = null

    private fun requestWifi() {
        if (wifiCb != null) return
        val cb = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                // relay traffic over Wi-Fi instead of the ~4 KB/s Bluetooth proxy
                runCatching { cm.bindProcessToNetwork(network) }
                (app.engine.transport(LinkIds.INTERNET) as? RelayTransport)?.onNetworkAvailable()
            }
            override fun onLost(network: Network) { runCatching { cm.bindProcessToNetwork(null) } }
        }
        wifiCb = cb
        runCatching {
            cm.requestNetwork(NetworkRequest.Builder().addTransportType(NetworkCapabilities.TRANSPORT_WIFI).build(), cb)
        }.onFailure { Log.w(TAG, "requestNetwork wifi", it); wifiCb = null }
    }

    private fun releaseWifi() {
        wifiCb?.let { runCatching { cm.unregisterNetworkCallback(it) } }
        wifiCb = null
        runCatching { cm.bindProcessToNetwork(null) }
    }

    private fun setWakeLock(on: Boolean) {
        if (on) {
            val wl = wakeLock ?: getSystemService(PowerManager::class.java).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "titi:watch").apply { setReferenceCounted(false) }.also { wakeLock = it }
            if (!wl.isHeld) wl.acquire()
        } else wakeLock?.let { if (it.isHeld) it.release() }
    }

    private fun fgsType(): Int =
        if (Build.VERSION.SDK_INT >= 34) ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE or ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE
        else ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE

    // ---- notification + ongoing activity ----

    private fun openIntent(page: Int = 0): PendingIntent =
        PendingIntent.getActivity(this, 100 + page, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP).putExtra(MainActivity.EXTRA_PAGE, page), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)

    private fun buildNotification(s: RadioState): NotificationCompat.Builder {
        val g = s.active
        val title = when {
            g == null -> getString(R.string.notif_radio_on)
            g.floor == FloorState.Busy && g.talkerName != null -> getString(R.string.notif_talking, g.talkerName)
            g.floor == FloorState.Talking -> getString(R.string.talking)
            else -> getString(R.string.notif_listening, g.name)
        }
        return NotificationCompat.Builder(this, WearApp.CHANNEL_RADIO)
            .setSmallIcon(R.drawable.ic_launcher_monochrome)
            .setContentTitle(title)
            .setContentText(g?.let { resources.getQuantityString(R.plurals.members_count, it.members.size, it.members.size) })
            .setContentIntent(openIntent())
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setCategory(NotificationCompat.CATEGORY_CALL)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setColor(getColor(R.color.amber))
    }

    private fun status(s: RadioState): Status {
        val g = s.active
        val members = g?.let { resources.getQuantityString(R.plurals.online_count, s.peers.values.count { p -> p.inGroup } + 1, s.peers.values.count { p -> p.inGroup } + 1) } ?: ""
        return Status.Builder()
            .addTemplate(getString(R.string.ongoing_status))
            .addPart("group", Status.TextPart(g?.talkerName?.let { getString(R.string.notif_talking, it) } ?: g?.name ?: getString(R.string.notif_radio_on)))
            .addPart("members", Status.TextPart(members))
            .build()
    }

    @android.annotation.SuppressLint("MissingPermission") // guarded by Notify.allowed() below
    private fun updateNotification(s: RadioState) {
        if (!ro.titi.app.util.Notify.allowed(this)) return
        val b = buildNotification(s)
        val oa = ongoing
        if (oa == null) {
            ongoing = OngoingActivity.Builder(applicationContext, NOTIF_ID, b)
                .setStaticIcon(R.drawable.ic_launcher_monochrome)
                .setTouchIntent(openIntent())
                .setStatus(status(s))
                .build().also { it.apply(applicationContext) }
        } else {
            oa.update(applicationContext, status(s))
        }
        ro.titi.app.util.Notify.post(this, NOTIF_ID, b.build())
    }

    private fun postAlert(a: Alert) {
        val nm = NotificationManagerCompat.from(this)
        if (!nm.areNotificationsEnabled()) return
        val b = NotificationCompat.Builder(this, WearApp.CHANNEL_ALERTS)
            .setSmallIcon(R.drawable.ic_launcher_monochrome)
            .setContentIntent(openIntent(2))
            .setAutoCancel(true)
            .setColor(getColor(R.color.amber))
        when (a) {
            is Alert.Sos -> {
                b.setContentTitle(if (a.cancelled) getString(R.string.sos_cancelled, a.fromName) else getString(R.string.sos_from, a.fromName))
                    .setContentText(a.groupName)
                    .setPriority(if (a.cancelled) NotificationCompat.PRIORITY_DEFAULT else NotificationCompat.PRIORITY_MAX)
                    .setCategory(NotificationCompat.CATEGORY_ALARM)
                    .setVibrate(if (a.cancelled) null else longArrayOf(0, 400, 200, 400, 200, 400))
                ro.titi.app.util.Notify.post(this, 1000 + a.group.hashCode(), b.build())
            }
            is Alert.InviteOffered -> if (!WearApp.foreground) {
                b.setContentTitle(a.invite.hostName).setContentText(a.invite.name).setCategory(NotificationCompat.CATEGORY_SOCIAL)
                ro.titi.app.util.Notify.post(this, 2000 + a.invite.group.hashCode(), b.build())
            }
        }
    }

    companion object {
        private const val TAG = "WearRadio"
        const val NOTIF_ID = 1
        const val ACTION_STOP = "ro.titi.wear.STOP"
        const val ACTION_PTT_TOGGLE = "ro.titi.wear.PTT_TOGGLE"

        /** Only call from a visible activity (while-in-use microphone rule). */
        fun start(ctx: Context) {
            runCatching { ctx.startForegroundService(Intent(ctx, RadioService::class.java)) }
                .onFailure { Log.w(TAG, "start refused", it) }
        }

        fun toggleTalk(app: WearApp) {
            val e = app.engine
            if (e.state.value.active?.floor == FloorState.Talking) e.pttUp() else e.pttDown()
        }
    }
}
