package ro.titi.app.util

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.content.pm.PackageManager
import android.location.LocationManager
import android.os.Build
import androidx.core.content.ContextCompat
import java.util.concurrent.Executors

/** Framework LocationManager only (works in the foss flavour, no GMS). */
object Location {
    private val exec = Executors.newSingleThreadExecutor()

    @SuppressLint("MissingPermission")
    fun lastKnown(ctx: Context, allowNull: Boolean = false, cb: (lat: Double, lon: Double, accM: Float) -> Unit) {
        val granted = ContextCompat.checkSelfPermission(ctx, Manifest.permission.ACCESS_FINE_LOCATION) == PackageManager.PERMISSION_GRANTED ||
            ContextCompat.checkSelfPermission(ctx, Manifest.permission.ACCESS_COARSE_LOCATION) == PackageManager.PERMISSION_GRANTED
        if (!granted) { if (allowNull) cb(0.0, 0.0, 0f); return }
        val lm = ctx.getSystemService(LocationManager::class.java)
        val best = listOf(LocationManager.GPS_PROVIDER, LocationManager.NETWORK_PROVIDER, LocationManager.PASSIVE_PROVIDER)
            .mapNotNull { runCatching { lm.getLastKnownLocation(it) }.getOrNull() }
            .maxByOrNull { it.time }
        if (best != null && System.currentTimeMillis() - best.time < 5 * 60_000) { cb(best.latitude, best.longitude, best.accuracy); return }
        if (Build.VERSION.SDK_INT >= 30 && lm.isProviderEnabled(LocationManager.GPS_PROVIDER)) {
            lm.getCurrentLocation(LocationManager.GPS_PROVIDER, null, exec) { l ->
                if (l != null) cb(l.latitude, l.longitude, l.accuracy) else if (best != null) cb(best.latitude, best.longitude, best.accuracy) else if (allowNull) cb(0.0, 0.0, 0f)
            }
        } else if (best != null) cb(best.latitude, best.longitude, best.accuracy) else if (allowNull) cb(0.0, 0.0, 0f)
    }
}
