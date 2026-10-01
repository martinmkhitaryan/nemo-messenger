package org.nemo

import java.io.File

internal actual fun loadReadReceiptsEnabled(): Boolean {
    val ctx = appContext ?: return true
    val file = File(ctx.filesDir, "read_receipts")
    if (!file.isFile) return true
    return file.readText().trim() != "off"
}

internal actual fun saveReadReceiptsEnabled(enabled: Boolean) {
    val ctx = appContext ?: return
    File(ctx.filesDir, "read_receipts").writeText(if (enabled) "on" else "off")
}
