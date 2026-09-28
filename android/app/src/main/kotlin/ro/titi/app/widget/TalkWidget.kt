package ro.titi.app.widget

import android.content.Context
import android.content.Intent
import androidx.glance.GlanceId
import androidx.glance.GlanceModifier
import androidx.glance.GlanceTheme
import androidx.glance.action.ActionParameters
import androidx.glance.action.clickable
import androidx.glance.appwidget.GlanceAppWidget
import androidx.glance.appwidget.GlanceAppWidgetReceiver
import androidx.glance.appwidget.action.ActionCallback
import androidx.glance.appwidget.action.actionRunCallback
import androidx.glance.appwidget.cornerRadius
import androidx.glance.appwidget.provideContent
import androidx.glance.background
import androidx.glance.layout.Alignment
import androidx.glance.layout.Box
import androidx.glance.layout.fillMaxSize
import androidx.glance.layout.padding
import androidx.glance.text.FontWeight
import androidx.glance.text.Text
import androidx.glance.text.TextStyle
import androidx.glance.unit.ColorProvider
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import ro.titi.app.R
import ro.titi.app.service.RadioService

/** Home/lock-screen Talk toggle (A-22). Tap = start/stop transmitting. */
class TalkWidget : GlanceAppWidget() {
    override suspend fun provideGlance(context: Context, id: GlanceId) {
        provideContent {
            GlanceTheme {
                Box(
                    GlanceModifier.fillMaxSize().cornerRadius(28.dp).background(ColorProvider(Color(0xFFFFB020))).padding(12.dp).clickable(actionRunCallback<TalkAction>()),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(context.getString(R.string.notif_action_talk), style = TextStyle(color = ColorProvider(Color(0xFF0E1013)), fontSize = 22.sp, fontWeight = FontWeight.Bold))
                }
            }
        }
    }
}

class TalkAction : ActionCallback {
    override suspend fun onAction(context: Context, glanceId: GlanceId, parameters: ActionParameters) {
        val app = context.applicationContext as ro.titi.app.TitiApp
        if (app.engine.state.value.running) {
            val e = app.engine
            if (e.state.value.active?.floor == ro.titi.app.core.FloorState.Talking) e.pttUp() else e.pttDown()
        } else {
            // radio off: open the app (a mic service may only start from a visible activity)
            context.startActivity(Intent(context, ro.titi.app.MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
        }
    }
}

class TalkWidgetReceiver : GlanceAppWidgetReceiver() {
    override val glanceAppWidget: GlanceAppWidget = TalkWidget()
}
