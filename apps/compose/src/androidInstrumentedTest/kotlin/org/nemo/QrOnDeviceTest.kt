package org.nemo

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Test
import org.junit.runner.RunWith
import kotlin.test.assertEquals
import kotlin.test.assertNull

@RunWith(AndroidJUnit4::class)
class QrOnDeviceTest {
    @Test
    fun contactQrRoundtrip() {
        val payload = byteArrayOf(0x01, 0x02, 0xFE.toByte(), 0x00, 0x7F)
        val (pixels, w, h) = qrMatrixToArgb(encodeContactQr(payload))
        assertEquals(String(payload, Charsets.ISO_8859_1), decodeQrArgb(pixels, w, h))
        assertNull(decodeQrArgb(intArrayOf(), 0, 0))
    }
}
