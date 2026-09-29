package ro.titi.tv

import android.Manifest
import android.bluetooth.BluetoothManager
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.os.Build
import androidx.core.content.ContextCompat

/**
 * Most TVs have no app-usable microphone (remote mics are routed to the system assistant only),
 * so talking is enabled only when a real capture device exists.
 */
object Mic {
    private val external = buildSet {
        add(AudioDeviceInfo.TYPE_USB_DEVICE)
        add(AudioDeviceInfo.TYPE_USB_HEADSET)
        add(AudioDeviceInfo.TYPE_BLUETOOTH_SCO)
        add(AudioDeviceInfo.TYPE_WIRED_HEADSET)
        if (Build.VERSION.SDK_INT >= 31) add(AudioDeviceInfo.TYPE_BLE_HEADSET)
    }

    fun hasInput(ctx: Context): Boolean {
        val am = ctx.getSystemService(AudioManager::class.java) ?: return false
        val inputs = runCatching { am.getDevices(AudioManager.GET_DEVICES_INPUTS).toList() }.getOrDefault(emptyList())
        val builtin = ctx.packageManager.hasSystemFeature(PackageManager.FEATURE_MICROPHONE) && inputs.any { it.type == AudioDeviceInfo.TYPE_BUILTIN_MIC }
        return builtin || inputs.any { it.type in external }
    }

    fun granted(ctx: Context) = ContextCompat.checkSelfPermission(ctx, Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED

    /** Mic present and allowed: the only case where this TV can take the floor. */
    fun usable(ctx: Context) = hasInput(ctx) && granted(ctx)

    fun bleUsable(ctx: Context): Boolean {
        if (!ctx.packageManager.hasSystemFeature(PackageManager.FEATURE_BLUETOOTH_LE)) return false
        if (ctx.getSystemService(BluetoothManager::class.java)?.adapter == null) return false
        if (Build.VERSION.SDK_INT < 31) return true
        return listOf(Manifest.permission.BLUETOOTH_SCAN, Manifest.permission.BLUETOOTH_CONNECT, Manifest.permission.BLUETOOTH_ADVERTISE)
            .all { ContextCompat.checkSelfPermission(ctx, it) == PackageManager.PERMISSION_GRANTED }
    }
}
