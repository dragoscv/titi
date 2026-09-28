package ro.titi.app

import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.titi_ffi.inviteWordlist

/** Instrumentation smoke: the app launches, the Rust core loads on-device, the activity reaches RESUMED. */
@RunWith(AndroidJUnit4::class)
class LaunchSmokeTest {
    @Test
    fun nativeCoreLoadsOnDevice() {
        assertTrue(inviteWordlist().size >= 1024)
    }

    @Test
    fun mainActivityResumes() {
        ActivityScenario.launch(MainActivity::class.java).use { s ->
            s.onActivity { a -> assertTrue(!a.isFinishing) }
            assertTrue(s.state.isAtLeast(androidx.lifecycle.Lifecycle.State.RESUMED))
        }
        assertTrue(InstrumentationRegistry.getInstrumentation().targetContext.packageName.startsWith("ro.titi.app"))
    }
}
