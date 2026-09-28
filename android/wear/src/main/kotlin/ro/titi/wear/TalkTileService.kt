package ro.titi.wear

import android.content.ComponentName
import android.content.Context
import androidx.wear.protolayout.ActionBuilders
import androidx.wear.protolayout.ActionBuilders.launchAction
import androidx.wear.protolayout.TimelineBuilders.Timeline
import androidx.wear.protolayout.material3.ButtonDefaults.filledButtonColors
import androidx.wear.protolayout.material3.MaterialScope
import androidx.wear.protolayout.material3.Typography.BODY_MEDIUM
import androidx.wear.protolayout.material3.Typography.DISPLAY_SMALL
import androidx.wear.protolayout.material3.primaryLayout
import androidx.wear.protolayout.material3.text
import androidx.wear.protolayout.material3.textEdgeButton
import androidx.wear.protolayout.modifiers.clickable
import androidx.wear.protolayout.types.argb
import androidx.wear.protolayout.types.layoutString
import androidx.wear.tiles.Material3TileService
import androidx.wear.tiles.RequestBuilders.TileRequest
import androidx.wear.tiles.TileBuilders.Tile
import androidx.wear.tiles.TileService
import androidx.wear.tiles.tile
import ro.titi.app.core.FloorState

/**
 * Tile: group + who's talking, and a Talk edge button that opens the PTT screen.
 * Tiles are tap-only (no press-and-hold), so the button deep-links into the app,
 * which also satisfies the "mic FGS starts from a visible activity" rule.
 */
class TalkTileService : Material3TileService() {

    override suspend fun MaterialScope.tileResponse(requestParams: TileRequest): Tile {
        val app = application as WearApp
        val s = app.engine.state.value
        val g = s.active
        val open = clickable(
            launchAction(
                ComponentName(this@TalkTileService, MainActivity::class.java),
                mapOf(MainActivity.EXTRA_PAGE to ActionBuilders.intExtra(0)),
            ),
        )
        val status = when {
            g == null -> getString(R.string.no_groups)
            g.floor == FloorState.Talking -> getString(R.string.talking)
            g.floor == FloorState.Busy && g.talkerName != null -> g.talkerName!!
            else -> resources.getQuantityString(R.plurals.online_count, s.peers.values.count { it.inGroup } + 1, s.peers.values.count { it.inGroup } + 1)
        }
        val layout = primaryLayout(
            titleSlot = { text((g?.name ?: getString(R.string.app_name)).layoutString) },
            mainSlot = {
                text(
                    status.layoutString,
                    typography = if (g?.floor == FloorState.Busy) DISPLAY_SMALL else BODY_MEDIUM,
                    color = if (g?.floor == FloorState.Busy) TEAL.argb else colorScheme.onSurface,
                )
            },
            bottomSlot = {
                textEdgeButton(
                    onClick = open,
                    colors = filledButtonColors().copy(containerColor = if (s.sosGroup != null) EMERGENCY.argb else AMBER.argb, labelColor = GRAPHITE.argb),
                    labelContent = { text((if (g == null) getString(R.string.join) else getString(R.string.tap_to_talk)).layoutString) },
                )
            },
            onClick = open,
        )
        return tile(timeline = Timeline.fromLayoutElement(layout))
    }

    companion object {
        private const val AMBER = 0xFFFFB020.toInt()
        private const val TEAL = 0xFF2DD4BF.toInt()
        private const val GRAPHITE = 0xFF0E1013.toInt()
        private const val EMERGENCY = 0xFFFF2D55.toInt()

        fun requestUpdate(ctx: Context) {
            runCatching { TileService.getUpdater(ctx).requestUpdate(TalkTileService::class.java) }
        }
    }
}
