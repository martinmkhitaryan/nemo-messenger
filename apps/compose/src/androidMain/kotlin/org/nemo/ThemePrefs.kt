package org.nemo

import java.io.File

internal actual fun loadThemeMode(): NemoThemeMode {
    val ctx = appContext ?: return NemoThemeMode.System
    val raw = File(ctx.filesDir, "theme_mode").takeIf { it.isFile }?.readText()?.trim().orEmpty()
    return NemoThemeMode.entries.find { it.name.equals(raw, ignoreCase = true) } ?: NemoThemeMode.System
}

internal actual fun saveThemeMode(mode: NemoThemeMode) {
    val ctx = appContext ?: return
    File(ctx.filesDir, "theme_mode").writeText(mode.name)
}
