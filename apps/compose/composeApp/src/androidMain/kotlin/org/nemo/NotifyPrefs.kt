package org.nemo

import java.io.File

internal actual fun loadNotifyMode(): NemoNotifyMode {
    val ctx = appContext ?: return NemoNotifyMode.Foreground
    val raw = File(ctx.filesDir, "notify_mode").takeIf { it.isFile }?.readText()?.trim().orEmpty()
    // Foreground is the default: real-time delivery unless the user opts out.
    return NemoNotifyMode.entries.find { it.name.equals(raw, ignoreCase = true) }
        ?: NemoNotifyMode.Foreground
}

internal actual fun saveNotifyMode(mode: NemoNotifyMode) {
    val ctx = appContext ?: return
    File(ctx.filesDir, "notify_mode").writeText(mode.name)
}
