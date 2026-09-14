package ro.titi.app.ui

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.content.ContextCompat

object Permissions {
    val mic = listOf(Manifest.permission.RECORD_AUDIO)
    val nearby: List<String> = buildList {
        if (Build.VERSION.SDK_INT >= 31) { add(Manifest.permission.BLUETOOTH_SCAN); add(Manifest.permission.BLUETOOTH_ADVERTISE); add(Manifest.permission.BLUETOOTH_CONNECT) }
        if (Build.VERSION.SDK_INT >= 33) add(Manifest.permission.NEARBY_WIFI_DEVICES)
        if (Build.VERSION.SDK_INT < 33) add(Manifest.permission.ACCESS_FINE_LOCATION) // BLE scan / Wi-Fi discovery pre-13
    }
    val notifications: List<String> = if (Build.VERSION.SDK_INT >= 33) listOf(Manifest.permission.POST_NOTIFICATIONS) else emptyList()
    val location = listOf(Manifest.permission.ACCESS_FINE_LOCATION, Manifest.permission.ACCESS_COARSE_LOCATION)
    val camera = listOf(Manifest.permission.CAMERA)

    fun granted(ctx: Context, perms: List<String>) = perms.all { ContextCompat.checkSelfPermission(ctx, it) == PackageManager.PERMISSION_GRANTED }
    fun essentialGranted(ctx: Context) = granted(ctx, mic) && granted(ctx, nearby) && granted(ctx, notifications)
}
