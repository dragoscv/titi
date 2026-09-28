package ro.titi.app.ui

import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.NotFoundException
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader

/** Pure-JVM QR decoding (zxing), used by the foss scanner and unit-tested on the host. */
object QrDecode {
    private val hints = mapOf(DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE), DecodeHintType.TRY_HARDER to true)

    /** @param y 8-bit luminance, row-major, exactly width×height. @return text or null. */
    fun luminance(y: ByteArray, width: Int, height: Int): String? {
        val src = PlanarYUVLuminanceSource(y, width, height, 0, 0, width, height, false)
        return try {
            QRCodeReader().decode(BinaryBitmap(HybridBinarizer(src)), hints).text
        } catch (_: NotFoundException) {
            null
        } catch (_: com.google.zxing.ChecksumException) {
            null
        } catch (_: com.google.zxing.FormatException) {
            null
        }
    }
}
