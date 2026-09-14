package ro.titi.app.transport

import java.io.ByteArrayOutputStream

/**
 * Minimal protobuf wire helpers for the handful of `signal.proto` messages the
 * relay transport needs. Fields are written/read by number; unknown fields are
 * skipped. Kept deliberately tiny — the full Wire-generated classes arrive with
 * `buf generate` for the rest of the app.
 */
object MiniProto {
    class Writer {
        private val out = ByteArrayOutputStream()
        fun varint(field: Int, v: Long): Writer { tag(field, 0); writeVarint(v); return this }
        fun bytes(field: Int, v: ByteArray): Writer { tag(field, 2); writeVarint(v.size.toLong()); out.write(v); return this }
        fun string(field: Int, v: String): Writer = bytes(field, v.encodeToByteArray())
        fun message(field: Int, w: Writer): Writer = bytes(field, w.toByteArray())
        fun toByteArray(): ByteArray = out.toByteArray()
        private fun tag(field: Int, wt: Int) = writeVarint(((field shl 3) or wt).toLong())
        private fun writeVarint(v0: Long) {
            var v = v0
            while (v and 0x7FL.inv() != 0L) { out.write(((v and 0x7F) or 0x80).toInt()); v = v ushr 7 }
            out.write(v.toInt())
        }
    }

    class Reader(private val b: ByteArray, private var pos: Int = 0, private val end: Int = b.size) {
        var field = 0; private set
        private var wt = 0
        fun next(): Boolean {
            if (pos >= end) return false
            val t = readVarint(); field = (t ushr 3).toInt(); wt = (t and 7).toInt(); return true
        }
        fun varint(): Long = readVarint()
        fun bytes(): ByteArray { val n = readVarint().toInt(); val r = b.copyOfRange(pos, pos + n); pos += n; return r }
        fun string(): String = bytes().decodeToString()
        fun message(): Reader { val n = readVarint().toInt(); val r = Reader(b, pos, pos + n); pos += n; return r }
        fun skip() {
            when (wt) {
                0 -> readVarint()
                1 -> pos += 8
                2 -> pos += readVarint().toInt()
                5 -> pos += 4
                else -> pos = end
            }
        }
        private fun readVarint(): Long {
            var shift = 0; var r = 0L
            while (true) {
                val x = b[pos++].toInt() and 0xFF
                r = r or ((x and 0x7F).toLong() shl shift)
                if (x and 0x80 == 0) return r
                shift += 7
            }
        }
    }
}
