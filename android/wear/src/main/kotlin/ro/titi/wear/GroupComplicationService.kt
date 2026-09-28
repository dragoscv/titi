package ro.titi.wear

import android.app.PendingIntent
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.drawable.Icon
import androidx.wear.watchface.complications.data.ComplicationData
import androidx.wear.watchface.complications.data.ComplicationType
import androidx.wear.watchface.complications.data.MonochromaticImage
import androidx.wear.watchface.complications.data.MonochromaticImageComplicationData
import androidx.wear.watchface.complications.data.PlainComplicationText
import androidx.wear.watchface.complications.data.ShortTextComplicationData
import androidx.wear.watchface.complications.data.SmallImage
import androidx.wear.watchface.complications.data.SmallImageComplicationData
import androidx.wear.watchface.complications.data.SmallImageType
import androidx.wear.watchface.complications.datasource.ComplicationDataSourceUpdateRequester
import androidx.wear.watchface.complications.datasource.ComplicationRequest
import androidx.wear.watchface.complications.datasource.SuspendingComplicationDataSourceService
import ro.titi.app.core.FloorState

/** Watch-face complication: online count in the active group (▶ name while someone talks). */
class GroupComplicationService : SuspendingComplicationDataSourceService() {

    override fun getPreviewData(type: ComplicationType): ComplicationData? = build(type, "3", "Titi")

    override suspend fun onComplicationRequest(request: ComplicationRequest): ComplicationData? {
        val s = (application as WearApp).engine.state.value
        val g = s.active
        val short = when {
            g == null -> "–"
            g.floor == FloorState.Busy -> "▶"
            g.floor == FloorState.Talking -> "●"
            else -> (s.peers.values.count { it.inGroup } + 1).toString()
        }
        val desc = g?.talkerName ?: g?.name ?: getString(R.string.app_name)
        return build(request.complicationType, short, desc)
    }

    private fun build(type: ComplicationType, short: String, desc: String): ComplicationData? {
        val icon = Icon.createWithResource(this, R.drawable.ic_launcher_monochrome)
        val tap = PendingIntent.getActivity(this, 7, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK), PendingIntent.FLAG_IMMUTABLE)
        val cd = PlainComplicationText.Builder(desc).build()
        return when (type) {
            ComplicationType.SHORT_TEXT -> ShortTextComplicationData.Builder(PlainComplicationText.Builder(short).build(), cd)
                .setMonochromaticImage(MonochromaticImage.Builder(icon).build())
                .setTitle(PlainComplicationText.Builder(desc.take(8)).build())
                .setTapAction(tap).build()
            ComplicationType.MONOCHROMATIC_IMAGE -> MonochromaticImageComplicationData.Builder(MonochromaticImage.Builder(icon).build(), cd).setTapAction(tap).build()
            ComplicationType.SMALL_IMAGE -> SmallImageComplicationData.Builder(SmallImage.Builder(icon, SmallImageType.ICON).build(), cd).setTapAction(tap).build()
            else -> null
        }
    }

    companion object {
        fun requestUpdate(ctx: Context) {
            runCatching { ComplicationDataSourceUpdateRequester.create(ctx, ComponentName(ctx, GroupComplicationService::class.java)).requestUpdateAll() }
        }
    }
}
