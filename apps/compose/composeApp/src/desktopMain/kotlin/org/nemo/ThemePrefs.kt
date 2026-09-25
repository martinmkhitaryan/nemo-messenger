package org.nemo

import java.io.File

internal actual fun loadThemeMode(): NemoThemeMode {
    val raw = themeFile().takeIf { it.isFile }?.readText()?.trim().orEmpty()
    return NemoThemeMode.entries.find { it.name.equals(raw, ignoreCase = true) } ?: NemoThemeMode.System
}

internal actual fun saveThemeMode(mode: NemoThemeMode) {
    val file = themeFile()
    file.parentFile?.mkdirs()
    file.writeText(mode.name)
}

private fun themeFile(): File = File(System.getProperty("user.home"), ".local/share/nemo/theme_mode")
