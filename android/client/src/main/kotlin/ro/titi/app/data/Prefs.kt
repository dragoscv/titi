package ro.titi.app.data

import android.content.Context
import android.util.Base64
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking

private val Context.store: DataStore<Preferences> by preferencesDataStore("titi")

enum class ThemeMode { System, Dark, Light, Contrast }
enum class SoundPack { Bird, Radio, Minimal }
enum class QualityOverride { Auto, Hq, Std, Low }

data class Settings(
    val name: String = "",
    val hue: Int = 40,
    val onboarded: Boolean = false,
    val theme: ThemeMode = ThemeMode.System,
    val soundPack: SoundPack = SoundPack.Bird,
    val hapticsOnly: Boolean = false,
    val volumePtt: Boolean = true,
    val quality: QualityOverride = QualityOverride.Auto,
    val useInternet: Boolean = true,
    val useBle: Boolean = true,
    val allowHotspot: Boolean = true,
    val batterySaver: Boolean = false,
    val relayUrl: String = DEFAULT_RELAY,
    /** Watch only: request Wi-Fi for voice instead of the Bluetooth proxy to the phone. */
    val useWifiVoice: Boolean = true,
) {
    companion object {
        const val DEFAULT_RELAY = "wss://titi-relay-x3clqgvrdq-ew.a.run.app/v1/ws"
    }
}

class Prefs(private val ctx: Context) {
    private object K {
        val seed = stringPreferencesKey("identity_seed")
        val groups = stringPreferencesKey("groups_blob")
        val name = stringPreferencesKey("name")
        val hue = intPreferencesKey("hue")
        val onboarded = booleanPreferencesKey("onboarded")
        val theme = stringPreferencesKey("theme")
        val sound = stringPreferencesKey("sound")
        val hapticsOnly = booleanPreferencesKey("haptics_only")
        val volumePtt = booleanPreferencesKey("volume_ptt")
        val quality = stringPreferencesKey("quality")
        val useInternet = booleanPreferencesKey("use_internet")
        val useBle = booleanPreferencesKey("use_ble")
        val allowHotspot = booleanPreferencesKey("allow_hotspot")
        val batterySaver = booleanPreferencesKey("battery_saver")
        val relayUrl = stringPreferencesKey("relay_url")
        val useWifiVoice = booleanPreferencesKey("use_wifi_voice")
    }

    val settings: Flow<Settings> = ctx.store.data.map { it.toSettings() }

    fun settingsNow(): Settings = runBlocking { settings.first() }

    private fun Preferences.toSettings() = Settings(
        name = this[K.name] ?: "",
        hue = this[K.hue] ?: 40,
        onboarded = this[K.onboarded] ?: false,
        theme = this[K.theme]?.let { runCatching { ThemeMode.valueOf(it) }.getOrNull() } ?: ThemeMode.System,
        soundPack = this[K.sound]?.let { runCatching { SoundPack.valueOf(it) }.getOrNull() } ?: SoundPack.Bird,
        hapticsOnly = this[K.hapticsOnly] ?: false,
        volumePtt = this[K.volumePtt] ?: true,
        quality = this[K.quality]?.let { runCatching { QualityOverride.valueOf(it) }.getOrNull() } ?: QualityOverride.Auto,
        useInternet = this[K.useInternet] ?: true,
        useBle = this[K.useBle] ?: true,
        allowHotspot = this[K.allowHotspot] ?: true,
        batterySaver = this[K.batterySaver] ?: false,
        relayUrl = this[K.relayUrl] ?: Settings.DEFAULT_RELAY,
        useWifiVoice = this[K.useWifiVoice] ?: true,
    )

    suspend fun update(block: (Settings) -> Settings) {
        ctx.store.edit { p ->
            // never read the store from inside edit (single-writer deadlock); use the snapshot
            val s = block(p.toSettings())
            p[K.name] = s.name
            p[K.hue] = s.hue
            p[K.onboarded] = s.onboarded
            p[K.theme] = s.theme.name
            p[K.sound] = s.soundPack.name
            p[K.hapticsOnly] = s.hapticsOnly
            p[K.volumePtt] = s.volumePtt
            p[K.quality] = s.quality.name
            p[K.useInternet] = s.useInternet
            p[K.useBle] = s.useBle
            p[K.allowHotspot] = s.allowHotspot
            p[K.batterySaver] = s.batterySaver
            p[K.relayUrl] = s.relayUrl
            p[K.useWifiVoice] = s.useWifiVoice
        }
    }

    // Identity seed and group blob are written synchronously from the engine
    // thread (rare, small); DataStore edits are cheap enough here.
    fun identitySeed(): ByteArray? = runBlocking { ctx.store.data.first()[K.seed] }?.let { Base64.decode(it, Base64.NO_WRAP) }
    fun saveIdentitySeed(seed: ByteArray) = runBlocking { ctx.store.edit { it[K.seed] = Base64.encodeToString(seed, Base64.NO_WRAP) } }
    fun groupsBlob(): ByteArray? = runBlocking { ctx.store.data.first()[K.groups] }?.let { Base64.decode(it, Base64.NO_WRAP) }
    /** Called from the 20 ms engine tick — never block it on disk I/O. Writes are serialised by DataStore. */
    fun saveGroupsBlob(b: ByteArray) {
        val enc = Base64.encodeToString(b, Base64.NO_WRAP)
        io.launch { ctx.store.edit { it[K.groups] = enc } }
    }
    // parallelism 1 → edits are enqueued in call order, so the latest blob always wins
    private val io = kotlinx.coroutines.CoroutineScope(kotlinx.coroutines.SupervisorJob() + kotlinx.coroutines.Dispatchers.IO.limitedParallelism(1))
}
