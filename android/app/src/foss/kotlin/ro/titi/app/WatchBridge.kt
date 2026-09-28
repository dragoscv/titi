package ro.titi.app

import android.content.Context

/** FOSS build has no Play Services Data Layer: the watch joins by code instead. */
object WatchBridge {
    suspend fun watches(ctx: Context): List<Pair<String, String>> = emptyList()
    suspend fun sendJoin(ctx: Context, link: String, name: String, hue: Int): Boolean = false
}
