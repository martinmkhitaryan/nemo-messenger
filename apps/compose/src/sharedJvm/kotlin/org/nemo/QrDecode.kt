package org.nemo

import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader

internal fun decodeQrArgb(pixels: IntArray, width: Int, height: Int): String? {
    if (width <= 0 || height <= 0 || pixels.size < width * height) return null
    return try {
        val source = RGBLuminanceSource(width, height, pixels)
        val bitmap = BinaryBitmap(HybridBinarizer(source))
        val hints = mapOf(
            DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE),
            DecodeHintType.TRY_HARDER to true,
        )
        QRCodeReader().decode(bitmap, hints).text
    } catch (_: Exception) {
        null
    }
}
