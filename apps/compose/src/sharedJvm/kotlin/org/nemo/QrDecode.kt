package org.nemo

import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.EncodeHintType
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.BitMatrix
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel

internal fun encodeContactQr(payload: ByteArray): BitMatrix {
    val hints = mapOf(
        EncodeHintType.CHARACTER_SET to "ISO-8859-1",
        EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M,
        EncodeHintType.MARGIN to 1,
    )
    return QRCodeWriter().encode(
        String(payload, Charsets.ISO_8859_1),
        BarcodeFormat.QR_CODE,
        0,
        0,
        hints,
    )
}

internal fun qrMatrixToArgb(matrix: BitMatrix, modulePx: Int = 8): Triple<IntArray, Int, Int> {
    val n = matrix.width
    val w = (n * modulePx).coerceAtLeast(1)
    val pixels = IntArray(w * w) { 0xFFFFFFFF.toInt() }
    val black = 0xFF000000.toInt()
    for (y in 0 until n) {
        for (x in 0 until n) {
            if (!matrix.get(x, y)) continue
            for (dy in 0 until modulePx) {
                for (dx in 0 until modulePx) {
                    pixels[(y * modulePx + dy) * w + (x * modulePx + dx)] = black
                }
            }
        }
    }
    return Triple(pixels, w, w)
}

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
