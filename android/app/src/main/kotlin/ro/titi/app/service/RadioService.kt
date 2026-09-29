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
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import androidx.lifecycle.LifecycleService
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import ro.titi.app.MainActivity
import ro.titi.app.R
import ro.titi.app.TitiApp
import ro.titi.app.core.Alert
import ro.titi.app.core.FloorState
import ro.titi.app.core.RadioState
import ro.titi.app.transport.LinkIds
import ro.titi.app.ui.Permissions
import ro.titi.app.ui.components.labelRes
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
    private var media: MediaButtons? = null
    private var started = false

    private data class NotifModel(val floor: FloorState?, val talker: String?, val group: String?, val members: Int, val link: uniffi.titi_ffi.FfiLinkClass?, val muted: Boolean, val peers: Int)

    override fun onCreate() {
        super.onCreate()
        android.util.Log.i("RadioService", "onCreate")
        app = application as TitiApp
        // A mic FGS can only start while the app is eligible for while-in-use
        // permissions (Android 14+). A sticky restart from the background or a
        // widget tap before onboarding would throw — bail out instead of crash-looping.
        val ok = Permissions.essentialGranted(this) && app.prefs.settingsNow().onboarded &&
            runCatching { ServiceCompat.startForeground(this, NOTIF_ID, buildNotification(app.engine.state.value), fgsType()) }
                .onFailure { android.util.Log.w("RadioService", "startForeground refused", it) }.isSuccess
        if (!ok) {
            // startForegroundService() obliges a startForeground() call even when we bail;
            // satisfy it with a type that needs no while-in-use permission, then leave.
            runCatching {
                ServiceCompat.startForeground(this, NOTIF_ID, buildNotification(app.engine.state.value), if (Build.VERSION.SDK_INT >= 34) ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE else 0)
            }
            ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
            started = false; stopSelf(); return
        }
        started = true
        acquireLocks()
        app.engine.start(buildTransports())
        media = MediaButtons(this) { toggleTalk() }
        lifecycleScope.launch {
            app.engine.state
                .map { s -> NotifModel(s.active?.floor, s.active?.talkerName, s.active?.name, s.active?.members?.size ?: 0, s.active?.link, s.muted, s.peers.size) }
                .distinctUntilChanged()
                .collect { ro.titi.app.util.Notify.post(this@RadioService, NOTIF_ID, buildNotification(app.engine.state.value)) }
        }
        lifecycleScope.launch {
            app.engine.state.map { it.groups.isNotEmpty() }.distinctUntilChanged().collect { inGroup -> setWakeLock(inGroup) }
        }
        lifecycleScope.launch { app.engine.alerts.collect { postAlert(it) } }
        registerSystemCallbacks()
        lifecycleScope.launch {
            app.prefs.settings.map { Triple(it.useBle, it.useInternet, it.relayUrl) }.distinctUntilChanged().drop(1).collect { (ble, net, _) ->
                app.engine.setTransport(LinkIds.BLE_L2CAP, if (ble) BleTransport(this@RadioService) { app.engine.engine.nodeId() } else null)
                app.engine.setTransport(LinkIds.INTERNET, if (net) relay() else null)
            }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        super.onStartCommand(intent, flags, startId)
        if (!started) { stopSelf(); return START_NOT_STICKY }
        when (intent?.action) {
            ACTION_STOP -> { stopSelf(); return START_NOT_STICKY }
            ACTION_PTT_TOGGLE -> toggleTalk()
            ACTION_MUTE_TOGGLE -> app.engine.setMuted(!app.engine.state.value.muted)
            ACTION_SOS_CANCEL -> app.engine.cancelSos()
        }
        // No sticky restart: a background restart can't legally reopen the mic FGS.
        return START_NOT_STICKY
    }

    private fun toggleTalk() {
        val e = app.engine
        if (e.state.value.active?.floor == FloorState.Talking) e.pttUp() else e.pttDown()
    }

    override fun onDestroy() {
        if (started) {
            runCatching { getSystemService(android.net.ConnectivityManager::class.java).unregisterNetworkCallback(netCb) }
            runCatching { unregisterReceiver(btReceiver) }
            app.engine.stop()
            media?.release()
            setWakeLock(false)
            wifiLock?.let { if (it.isHeld) it.release() }
        }
        super.onDestroy()
    }

    override fun onBind(intent: Intent): IBinder? { super.onBind(intent); return null }

    private fun relay() = RelayTransport({ app.prefs.settingsNow().relayUrl }, { app.engine.engine.nodeId() }, { app.prefs.settingsNow().name }, { app.prefs.settingsNow().hue })

    private val netCb = object : android.net.ConnectivityManager.NetworkCallback() {
        override fun onAvailable(network: android.net.Network) {
            (app.engine.transport(LinkIds.INTERNET) as? RelayTransport)?.onNetworkAvailable()
        }
    }

    /** BLE can't start while Bluetooth is off; rebuild it when the adapter turns on. */
    private val btReceiver = object : android.content.BroadcastReceiver() {
        override fun onReceive(c: Context, i: Intent) {
            val st = i.getIntExtra(android.bluetooth.BluetoothAdapter.EXTRA_STATE, -1)
            if (st == android.bluetooth.BluetoothAdapter.STATE_ON && app.prefs.settingsNow().useBle) {
                app.engine.setTransport(LinkIds.BLE_L2CAP, BleTransport(this@RadioService) { app.engine.engine.nodeId() })
            } else if (st == android.bluetooth.BluetoothAdapter.STATE_OFF) {
                app.engine.setTransport(LinkIds.BLE_L2CAP, null)
            }
        }
    }

    private fun registerSystemCallbacks() {
        runCatching { getSystemService(android.net.ConnectivityManager::class.java).registerDefaultNetworkCallback(netCb) }
        androidx.core.content.ContextCompat.registerReceiver(
            this, btReceiver, android.content.IntentFilter(android.bluetooth.BluetoothAdapter.ACTION_STATE_CHANGED),
            androidx.core.content.ContextCompat.RECEIVER_NOT_EXPORTED,
        )
    }

    private fun buildTransports(): List<Transport> {
        val s = app.prefs.settingsNow()
        val list = mutableListOf<Transport>()
        list += LanTransport(this)
        if (s.useBle) list += BleTransport(this) { app.engine.engine.nodeId() }
        if (s.useInternet) list += relay()
        return list
    }

    private fun fgsType(): Int =
        if (Build.VERSION.SDK_INT >= 34) ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE or ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE
        else if (Build.VERSION.SDK_INT >= 29) ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE else 0

    private fun acquireLocks() {
        val wm = applicationContext.getSystemService(WifiManager::class.java)
        @Suppress("DEPRECATION")
        wifiLock = wm?.createWifiLock(if (Build.VERSION.SDK_INT >= 29) WifiManager.WIFI_MODE_FULL_LOW_LATENCY else WifiManager.WIFI_MODE_FULL_HIGH_PERF, "titi:wifi")?.apply { setReferenceCounted(false); acquire() }
    }

    /** The 20 ms engine tick needs the CPU with the screen off — but only while we are in a group. */
    private fun setWakeLock(on: Boolean) {
        if (on) {
            val wl = wakeLock ?: getSystemService(PowerManager::class.java).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "titi:radio").apply { setReferenceCounted(false) }.also { wakeLock = it }
            if (!wl.isHeld) wl.acquire()
        } else wakeLock?.let { if (it.isHeld) it.release() }
    }

    private fun postAlert(a: Alert) {
        val nm = NotificationManagerCompat.from(this)
        if (!nm.areNotificationsEnabled()) return
        val open = PendingIntent.getActivity(this, 10, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val b = NotificationCompat.Builder(this, TitiApp.CHANNEL_ALERTS)
            .setSmallIcon(R.drawable.ic_launcher_monochrome)
            .setContentIntent(open)
            .setAutoCancel(true)
            .setColor(getColor(R.color.amber))
        when (a) {
            is Alert.Sos -> {
                if (a.cancelled) b.setContentTitle(getString(R.string.alert_sos_cancelled, a.fromName)).setPriority(NotificationCompat.PRIORITY_DEFAULT)
                else {
                    b.setContentTitle(getString(R.string.alert_sos_title, a.fromName))
                        .setContentText(getString(R.string.alert_sos_body, a.groupName))
                        .setPriority(NotificationCompat.PRIORITY_MAX)
                        .setCategory(NotificationCompat.CATEGORY_ALARM)
                    if (a.latE7 != 0L || a.lonE7 != 0L) {
                        val geo = Intent(Intent.ACTION_VIEW, android.net.Uri.parse("geo:${a.latE7 / 1e7},${a.lonE7 / 1e7}?q=${a.latE7 / 1e7},${a.lonE7 / 1e7}"))
                        b.addAction(0, getString(R.string.alert_open_map), PendingIntent.getActivity(this, 11, geo, PendingIntent.FLAG_IMMUTABLE))
                    }
                }
                ro.titi.app.util.Notify.post(this, ALERT_SOS_ID + a.group.hashCode(), b.build())
            }
            is Alert.InviteOffered -> {
                if (TitiApp.foreground) return // the in-app banner handles it
                b.setContentTitle(getString(R.string.invite_offered_title, a.invite.hostName))
                    .setContentText(getString(R.string.invite_offered_body, a.invite.name, a.invite.members))
                    .setCategory(NotificationCompat.CATEGORY_SOCIAL)
                ro.titi.app.util.Notify.post(this, ALERT_INVITE_ID + a.invite.group.hashCode(), b.build())
            }
        }
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
            .setContentText(g?.let { gs -> listOfNotNull(resources.getQuantityString(R.plurals.members, gs.members.size, gs.members.size), gs.link?.let { getString(it.labelRes()) }).joinToString(" · ") })
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
        private const val ALERT_SOS_ID = 1000
        private const val ALERT_INVITE_ID = 2000
        const val ACTION_STOP = "ro.titi.app.STOP"
        const val ACTION_PTT_TOGGLE = "ro.titi.app.PTT_TOGGLE"
        const val ACTION_MUTE_TOGGLE = "ro.titi.app.MUTE_TOGGLE"
        const val ACTION_SOS_CANCEL = "ro.titi.app.SOS_CANCEL"
        const val EXTRA_GROUP = "group"

        fun start(ctx: Context) {
            runCatching { ctx.startForegroundService(Intent(ctx, RadioService::class.java)) }
                .onFailure { android.util.Log.w("RadioService", "start refused", it) }
        }
        fun stop(ctx: Context) {
            // stopService is legal from the background (startService with an action is not, API 26+)
            ctx.stopService(Intent(ctx, RadioService::class.java))
        }
    }
}
