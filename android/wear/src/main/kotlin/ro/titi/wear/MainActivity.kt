package ro.titi.wear

import android.Manifest
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Emergency
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.view.KeyEvent
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.wear.compose.foundation.pager.HorizontalPager
import androidx.wear.compose.foundation.pager.rememberPagerState
import androidx.wear.compose.material3.AnimatedPage
import androidx.wear.compose.material3.AppScaffold
import androidx.wear.compose.material3.Button
import androidx.wear.compose.material3.ConfirmationDialog
import androidx.wear.compose.material3.HorizontalPagerScaffold
import androidx.wear.compose.material3.MaterialTheme
import androidx.wear.compose.material3.PagerScaffoldDefaults
import androidx.wear.compose.material3.ScreenScaffold
import androidx.wear.compose.material3.SuccessConfirmationDialog
import androidx.wear.compose.material3.Text
import androidx.wear.compose.material3.TimeText
import androidx.wear.compose.material3.onehandedgesture.OneHandedGestureAction
import androidx.wear.compose.material3.onehandedgesture.oneHandedGesture
import androidx.wear.compose.material3.onehandedgesture.rememberOneHandedGestureConfiguration
import androidx.wear.compose.navigation.SwipeDismissableNavHost
import androidx.wear.compose.navigation.composable
import androidx.wear.compose.navigation.rememberSwipeDismissableNavController
import kotlinx.coroutines.flow.MutableStateFlow
import ro.titi.app.core.FloorState
import ro.titi.wear.ui.ActionsScreen
import ro.titi.wear.ui.ChatScreen
import ro.titi.wear.ui.GroupsScreen
import ro.titi.wear.ui.JoinScreen
import ro.titi.wear.ui.RecordScreen
import ro.titi.wear.ui.SettingsScreen
import ro.titi.wear.ui.TalkScreen
import ro.titi.wear.ui.TitiWearTheme

/**
 * Watch shell. Pages (swipe ←/→, rotary on lists): Groups · Talk (start) · Messages · Group.
 * Swipe-right from the edge dismisses sub-screens (join, record, settings).
 */
class MainActivity : ComponentActivity() {
    private val app get() = application as WearApp
    /** Requested page + sequence: the pager only tracks the request, so asking for the page it
     *  "already" wants (Talk, after the user swiped to Groups) must still be a new value. */
    private val pageReq = MutableStateFlow(PAGE_TALK to 0)
    private fun go(p: Int) { pageReq.value = p to pageReq.value.second + 1 }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        go(intent.pageExtra())
        setContent { TitiWearTheme { Root() } }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        go(intent.pageExtra())
    }

    override fun onResume() {
        super.onResume()
        // legal FGS start point: we're visible (Wear OS 6 while-in-use mic rule)
        if (micGranted() && !app.engine.state.value.running) RadioService.start(this)
    }

    override fun onPause() {
        super.onPause()
        // never leave the mic open because a lift was missed (ambient, notification shade)
        if (app.engine.state.value.active?.floor == FloorState.Talking && !isChangingConfigurations) app.engine.pttUp()
    }

    /** Galaxy Watch buttons are system-owned, but other watches expose STEM keys: toggle talk. */
    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        if ((keyCode == KeyEvent.KEYCODE_STEM_1 || keyCode == KeyEvent.KEYCODE_STEM_PRIMARY) && event.repeatCount == 0 && app.engine.state.value.active != null) {
            RadioService.toggleTalk(app); return true
        }
        return super.onKeyDown(keyCode, event)
    }

    /** Stops the radio (ongoing activity + watch-face icon go away) and closes the app. */
    private fun quit() = ro.titi.app.util.RadioLifecycle.quit(this) { RadioService.stop(this) }

    private fun micGranted() = ContextCompat.checkSelfPermission(this, Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED

    private fun Intent.pageExtra() = getIntExtra(EXTRA_PAGE, PAGE_TALK - 1).let { if (it < 0) PAGE_TALK else it + 1 }.coerceIn(0, PAGE_COUNT - 1)

    @Composable
    private fun Root() {
        val ctx = LocalContext.current
        var granted by remember { androidx.compose.runtime.mutableStateOf(micGranted()) }
        LifecycleResumeEffect(Unit) { granted = micGranted(); onPauseOrDispose { } }
        val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {
            granted = micGranted()
            if (granted) RadioService.start(ctx)
        }
        AppScaffold(timeText = { TimeText() }) {
            if (!granted) {
                PermissionScreen { launcher.launch(permissions()) }
            } else {
                Main()
            }
        }
    }

    @Composable
    private fun Main() {
        val nav = rememberSwipeDismissableNavController()
        val state by app.engine.state.collectAsState()
        val settingsOrNull by app.prefs.settings.collectAsState(initial = null)
        val settings = settingsOrNull ?: return
        val fromPhone by app.pendingFromPhone.collectAsState()
        val request by pageReq.collectAsState()
        val target = request.first

        SwipeDismissableNavHost(navController = nav, startDestination = "home") {
            composable("home") {
                val pager = rememberPagerState(initialPage = target, pageCount = { PAGE_COUNT })
                LaunchedEffect(request) { pager.animateScrollToPage(target) }
                // double-pinch = toggle talk on the Talk page (hands-free; no-op where unsupported)
                val gesture = rememberOneHandedGestureConfiguration(action = OneHandedGestureAction.Primary)
                HorizontalPagerScaffold(pagerState = pager) {
                    HorizontalPager(
                        state = pager,
                        flingBehavior = PagerScaffoldDefaults.snapWithSpringFlingBehavior(state = pager),
                    ) { p ->
                        AnimatedPage(pageIndex = p, pagerState = pager) {
                            when (p) {
                                PAGE_GROUPS -> GroupsScreen(app.engine, state, onOpen = { go(PAGE_TALK) }, onJoin = { nav.navigate("join") })
                                PAGE_TALK -> androidx.compose.foundation.layout.Box(
                                    Modifier.oneHandedGesture(
                                        gestureConfiguration = gesture,
                                        onGestureLabel = stringResource(R.string.double_pinch_hint),
                                        onGesture = { if (state.active != null) RadioService.toggleTalk(app) },
                                    ),
                                ) { TalkScreen(app.engine, state, onNoGroup = { nav.navigate("join") }) }
                                PAGE_CHAT -> if (state.active != null) ChatScreen(app.engine, state, onRecord = { nav.navigate("record") }) else EmptyPage()
                                PAGE_ACTIONS -> ActionsScreen(app.engine, state, app.prefs, settings, onSettings = { nav.navigate("settings") }, onQuit = ::quit)
                            }
                        }
                    }
                }
            }
            composable("join") { JoinScreen(app.engine) { nav.popBackStack() } }
            composable("record") { RecordScreen(app.engine, state) { nav.popBackStack() } }
            // collect inside the destination: the nav graph is remembered, so a value captured from
            // the outer scope goes stale (the switch showed ON while DataStore held false)
            composable("settings") {
                val live by app.prefs.settings.collectAsState(initial = settings)
                SettingsScreen(app.prefs, live, onQuit = ::quit)
            }
        }

        SuccessConfirmationDialog(
            visible = fromPhone,
            onDismissRequest = { app.pendingFromPhone.value = false },
            curvedText = null,
        )
        val sosAlert = state.groups.firstOrNull { it.talkerPrio >= 2 && it.floor == FloorState.Busy }
        ConfirmationDialog(
            visible = sosAlert != null && target != PAGE_TALK,
            onDismissRequest = { go(PAGE_TALK) },
            curvedText = null,
        ) { androidx.wear.compose.material3.Icon(Icons.Rounded.Emergency, null, tint = ro.titi.wear.ui.Titi.Emergency) }
    }

    @Composable
    private fun EmptyPage() {
        ScreenScaffold { Text(stringResource(R.string.no_groups), Modifier.fillMaxSize().padding(32.dp), textAlign = TextAlign.Center) }
    }

    @Composable
    private fun PermissionScreen(onAllow: () -> Unit) {
        ScreenScaffold {
            Column(Modifier.fillMaxSize().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterVertically)) {
                Text(stringResource(R.string.perm_title), style = MaterialTheme.typography.titleMedium, textAlign = TextAlign.Center)
                Text(stringResource(R.string.perm_body), style = MaterialTheme.typography.bodySmall, textAlign = TextAlign.Center)
                Button(onClick = onAllow, label = { Text(stringResource(R.string.perm_allow)) })
            }
        }
    }

    private fun permissions(): Array<String> = buildList {
        add(Manifest.permission.RECORD_AUDIO)
        if (Build.VERSION.SDK_INT >= 33) add(Manifest.permission.POST_NOTIFICATIONS)
        if (Build.VERSION.SDK_INT >= 31) { add(Manifest.permission.BLUETOOTH_SCAN); add(Manifest.permission.BLUETOOTH_ADVERTISE); add(Manifest.permission.BLUETOOTH_CONNECT) }
        else add(Manifest.permission.ACCESS_FINE_LOCATION)
    }.toTypedArray()

    companion object {
        const val EXTRA_PAGE = "page"
        const val PAGE_GROUPS = 0
        const val PAGE_TALK = 1
        const val PAGE_CHAT = 2
        const val PAGE_ACTIONS = 3
        const val PAGE_COUNT = 4
    }
}
