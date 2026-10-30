package ro.titi.app.core

import android.content.res.Resources
import ro.titi.client.R
import uniffi.titi_ffi.FfiQuoteKind
import uniffi.titi_ffi.FfiReplyState

/** Shared chat wording for phone, watch and TV (resources live in the client module). */
object ChatText {
    /** Quote line for a reply: the preview, a kind label, or why the original is gone. */
    fun replyPreview(res: Resources, r: ChatReply): String = when (r.state) {
        FfiReplyState.DELETED -> res.getString(R.string.chat_reply_deleted)
        FfiReplyState.MISSING -> res.getString(R.string.chat_reply_missing)
        FfiReplyState.PRESENT -> when (r.kind) {
            FfiQuoteKind.TEXT -> r.preview
            FfiQuoteKind.VOICE -> res.getString(R.string.chat_quote_voice)
            FfiQuoteKind.LOCATION -> res.getString(R.string.chat_quote_location)
            FfiQuoteKind.SOS -> res.getString(R.string.chat_quote_sos)
        }
    }

    fun replySender(res: Resources, r: ChatReply): String = if (r.mine) res.getString(R.string.chat_you) else r.senderName

    /** "Forwarded" / "Forwarded many times", or null when not forwarded. */
    fun forwarded(res: Resources, count: Int): String? = when {
        count >= ChatRules.FORWARDED_MANY -> res.getString(R.string.chat_forwarded_many)
        count > 0 -> res.getString(R.string.chat_forwarded)
        else -> null
    }

    /** Screen-reader label for a reaction chip, e.g. "👍 2, including you". */
    fun reaction(res: Resources, r: ChatReaction): String =
        if (r.mine) res.getString(R.string.cd_reaction_mine, r.emoji, r.count) else res.getString(R.string.cd_reaction, r.emoji, r.count)
}
