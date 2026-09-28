package ro.titi.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.titi_ffi.FfiAction
import uniffi.titi_ffi.FfiLinkClass
import uniffi.titi_ffi.FfiProfile
import uniffi.titi_ffi.TitiEngine
import uniffi.titi_ffi.decodeVoiceNote
import uniffi.titi_ffi.encodeVoiceNote
import uniffi.titi_ffi.inviteWordlist
import uniffi.titi_ffi.parseInviteCode
import java.io.File

/** Runs the generated Kotlin bindings against the host build of titi-ffi (JNA). */
class FfiBindingsTest {
    private val repo: File = generateSequence(File("").absoluteFile) { it.parentFile }.first { File(it, "testvectors").isDirectory }

    @Test
    fun inviteCodesFromVectorsParse() {
        val json = File(repo, "testvectors/invite.json").readText()
        val codes = Regex("\"code\"\\s*:\\s*\"([a-z-]+-\\d{2})\"").findAll(json).map { it.groupValues[1] }.toList()
        assertTrue("vectors contain codes", codes.size >= 3)
        codes.forEach { assertTrue("parses $it", parseInviteCode(it)) }
        assertFalse(parseInviteCode("not a code"))
    }

    @Test
    fun wordlistIsTheBip39SizedList() {
        val w = inviteWordlist()
        assertTrue("wordlist size ${w.size}", w.size >= 1024)
        assertEquals("unique words", w.size, w.toSet().size)
    }

    @Test
    fun voiceNoteOpusRoundTripKeepsDuration() {
        val ms = 1_000
        val pcm = ByteArray(48 * ms * 2) { i -> if (i % 2 == 0) ((i / 2 % 96) * 2).toByte() else 0 }
        val packets = encodeVoiceNote(pcm, FfiProfile.STD)
        assertTrue("compressed ${packets.size} < ${pcm.size}", packets.size in 1 until pcm.size / 4)
        val out = decodeVoiceNote(packets, FfiProfile.STD)
        assertEquals("50 × 20 ms frames of 960 samples", 50 * 960 * 2, out.size)
    }

    @Test
    fun oversizeSendsAreFragmentedBelowTheLinkMtu() {
        val e = TitiEngine(ByteArray(0), "Test", 40u, 7u)
        var now = 1_789_400_000_000uL
        e.onLinkUp(1u, FfiLinkClass.LAN, 1200u, now)
        e.createGroup("Ceas", now)
        val gid = e.groups().first().id
        val sends = mutableListOf<ByteArray>()
        fun collect(a: List<FfiAction>) = a.filterIsInstance<FfiAction.Send>().forEach { sends += it.bytes }
        collect(e.sendVoiceNote(gid, FfiProfile.STD, 5_000u, ByteArray(12_000) { it.toByte() }, now))
        repeat(40) { now += 20u; collect(e.tick(now)) }
        assertTrue("fragments were emitted (${sends.size})", sends.count { it[1] == 0x23.toByte() } >= 10)
        assertTrue("every frame ≤ MTU (max ${sends.maxOf { it.size }})", sends.all { it.size <= 1200 })
    }
}
