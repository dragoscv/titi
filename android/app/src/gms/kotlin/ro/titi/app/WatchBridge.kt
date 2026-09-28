package ro.titi.app

import android.content.Context
import android.util.Log
import com.google.android.gms.wearable.CapabilityClient
import com.google.android.gms.wearable.Wearable
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.tasks.await
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import org.json.JSONObject

/** Phone → watch provisioning over the Wearable Data Layer. Contract: wear/PhoneBridgeService. */
object WatchBridge {
    private const val TAG = "WatchBridge"
    private const val CAPABILITY = "titi_watch"
    private const val PATH_JOIN = "/titi/join"
    private const val PATH_PROFILE = "/titi/profile"

    /** Reachable watches with Titi installed; empty when none or Play Services is missing. */
    suspend fun watches(ctx: Context): List<Pair<String, String>> = runCatching {
        withTimeout(5_000) {
            Wearable.getCapabilityClient(ctx).getCapability(CAPABILITY, CapabilityClient.FILTER_REACHABLE).await()
                .nodes.map { it.id to it.displayName }
        }
    }.onFailure { Log.i(TAG, "no watch: ${it.message}") }.getOrDefault(emptyList())

    /** Sends the profile, then the invite link, to every reachable watch. True if at least one got it. */
    suspend fun sendJoin(ctx: Context, link: String, name: String, hue: Int): Boolean = withContext(Dispatchers.IO) {
        val msg = Wearable.getMessageClient(ctx)
        val profile = JSONObject().put("name", name).put("hue", hue).toString().toByteArray()
        watches(ctx).count { (node, _) ->
            runCatching {
                withTimeout(10_000) {
                    msg.sendMessage(node, PATH_PROFILE, profile).await()
                    msg.sendMessage(node, PATH_JOIN, link.toByteArray()).await()
                }
            }.onFailure { Log.w(TAG, "send to $node", it) }.isSuccess
        } > 0
    }
}
