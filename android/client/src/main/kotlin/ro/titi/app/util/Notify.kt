package ro.titi.app.util

import android.Manifest
import android.annotation.SuppressLint
import android.app.Notification
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat

/**
 * POST_NOTIFICATIONS is user-revocable on API 33+. Every post goes through here so a
 * revoked grant silently drops the notification instead of throwing (the FGS keeps running).
 */
object Notify {
    fun allowed(ctx: Context) = Build.VERSION.SDK_INT < 33 ||
        ContextCompat.checkSelfPermission(ctx, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED

    @SuppressLint("MissingPermission") // checked by allowed() on the line above
    fun post(ctx: Context, id: Int, n: Notification): Boolean {
        if (!allowed(ctx)) return false
        return runCatching { NotificationManagerCompat.from(ctx).notify(id, n) }.isSuccess
    }
}
