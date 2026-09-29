package ro.titi.app.util

/**
 * Invite link origins. Links we generate use [WEB_ORIGIN]; parsers also accept the
 * legacy `titi.app` host (old QR codes are still in circulation) and `titi://j/`.
 */
object InviteLinks {
    const val WEB_HOST = "titi.dragoscatalin.ro"
    const val WEB_ORIGIN = "https://$WEB_HOST"
    const val LEGACY_HOST = "titi.app"
    const val DEEP_PREFIX = "titi://j/"
    val HOSTS = setOf(WEB_HOST, LEGACY_HOST)

    /** True for `titi://j/<t>` or `https://<new|legacy host>/j/<t>`. */
    fun isInviteUrl(t: String): Boolean =
        t.startsWith(DEEP_PREFIX) || HOSTS.any { t.startsWith("https://$it/j/") }

    /** `titi://j/<t>` → `https://titi.dragoscatalin.ro/j/<t>`; anything else unchanged. */
    fun toWebLink(l: String): String = l.replace(DEEP_PREFIX, "$WEB_ORIGIN/j/")

    /** Web invite URL (either host) → `titi://j/<t>`; deep links pass through; else null. */
    fun toDeepLink(t: String): String? = when {
        t.startsWith(DEEP_PREFIX) -> t
        isInviteUrl(t) -> DEEP_PREFIX + t.substringAfter("/j/")
        else -> null
    }
}
