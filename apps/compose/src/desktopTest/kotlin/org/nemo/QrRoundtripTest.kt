package org.nemo

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class QrRoundtripTest {
    @Test
    fun contactQrEncodesBytesTheScannerReadsBack() {
        val payload = byteArrayOf(0x01, 0x02, 0xFE.toByte(), 0x00, 0x7F)
        val matrix = encodeContactQr(payload)
        val (pixels, w, h) = qrMatrixToArgb(matrix)
        val decoded = decodeQrArgb(pixels, w, h)
        assertEquals(String(payload, Charsets.ISO_8859_1), decoded)
    }

    @Test
    fun emptyOrShortBufferIsNotACode() {
        assertNull(decodeQrArgb(intArrayOf(), 0, 0))
        assertNull(decodeQrArgb(IntArray(4) { 0xFFFFFFFF.toInt() }, 2, 2))
    }
}
