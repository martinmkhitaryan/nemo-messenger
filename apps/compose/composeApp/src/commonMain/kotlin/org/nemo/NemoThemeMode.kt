package org.nemo

import androidx.compose.runtime.Composable

internal enum class NemoThemeMode {
    System,
    Light,
    Dark,
    ;

    fun resolveDark(systemDark: Boolean): Boolean = when (this) {
        System -> systemDark
        Light -> false
        Dark -> true
    }

    val label: String
        get() = when (this) {
            System -> "System"
            Light -> "Light"
            Dark -> "Dark"
        }
}

internal expect fun loadThemeMode(): NemoThemeMode

internal expect fun saveThemeMode(mode: NemoThemeMode)

@Composable
expect fun rememberPickFile(onPicked: (String) -> Unit): () -> Unit

/** Opens a platform save dialog; invokes [onResult] with true when bytes were written. */
@Composable
expect fun rememberSaveFile(onResult: (Boolean) -> Unit): (fileName: String, bytes: ByteArray) -> Unit

@Composable
expect fun rememberScanQr(onText: (String) -> Unit): () -> Unit

@Composable
expect fun rememberEnsureMic(onReady: () -> Unit): () -> Unit
