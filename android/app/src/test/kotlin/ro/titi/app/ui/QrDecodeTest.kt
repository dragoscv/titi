package ro.titi.app.ui

import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The foss scanner's decoder (zxing, no ML Kit) reads the same QR the invite sheet renders. */
class QrDecodeTest {
    private fun luminanceOf(text: String, size: Int): ByteArray {
        val m = QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, size, size, mapOf(EncodeHintType.MARGIN to 2))
        return ByteArray(size * size) { i -> if (m[i % size, i / size]) 0 else 0xFF.toByte() }
    }

    @Test
    fun decodesInviteDeepLink() {
        val link = "titi://j/AbCdEf0123456789_-xyz"
        assertEquals(link, QrDecode.luminance(luminanceOf(link, 400), 400, 400))
    }

    @Test
    fun blankFrameYieldsNull() {
        assertNull(QrDecode.luminance(ByteArray(320 * 240) { 0x80.toByte() }, 320, 240))
    }
}
