package org.nemo

import java.io.File

internal actual fun loadReadReceiptsEnabled(): Boolean {
    val file = readReceiptsFile()
    if (!file.isFile) return true
    return file.readText().trim() != "off"
}

internal actual fun saveReadReceiptsEnabled(enabled: Boolean) {
    val file = readReceiptsFile()
    file.parentFile?.mkdirs()
    file.writeText(if (enabled) "on" else "off")
}

private fun readReceiptsFile(): File = File(System.getProperty("user.home"), ".local/share/nemo/read_receipts")
