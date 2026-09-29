package ro.titi.app.util

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class InviteLinksTest {
    private val path = "g/h/c/s/v"

    @Test
    fun generatedLinksUseNewOrigin() {
        assertEquals("https://titi.dragoscatalin.ro/j/$path", InviteLinks.toWebLink("titi://j/$path"))
    }

    @Test
    fun acceptsNewLegacyAndDeepLinks() {
        assertTrue(InviteLinks.isInviteUrl("https://titi.dragoscatalin.ro/j/$path"))
        assertTrue(InviteLinks.isInviteUrl("https://titi.app/j/$path"))
        assertTrue(InviteLinks.isInviteUrl("titi://j/$path"))
    }

    @Test
    fun rejectsForeignHosts() {
        assertFalse(InviteLinks.isInviteUrl("https://evil.example/j/$path"))
        assertFalse(InviteLinks.isInviteUrl("https://titi.app.evil.example/j/$path"))
        assertNull(InviteLinks.toDeepLink("hello"))
    }

    @Test
    fun legacyAndNewWebLinksMapToSameDeepLink() {
        assertEquals("titi://j/$path", InviteLinks.toDeepLink("https://titi.app/j/$path"))
        assertEquals("titi://j/$path", InviteLinks.toDeepLink("https://titi.dragoscatalin.ro/j/$path"))
    }
}
