package org.nemo

import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberUpdatedState
import uniffi.nemo.NemoClient
import java.io.File

internal actual fun loadNotifyMode(): NemoNotifyMode {
    val raw = notifyFile().takeIf { it.isFile }?.readText()?.trim().orEmpty()
    // Foreground is the default: real-time delivery unless the user opts out.
    return NemoNotifyMode.entries.find { it.name.equals(raw, ignoreCase = true) }
        ?: NemoNotifyMode.Foreground
}

internal actual fun saveNotifyMode(mode: NemoNotifyMode) {
    val file = notifyFile()
    file.parentFile?.mkdirs()
    file.writeText(mode.name)
}

private fun notifyFile(): File = File(System.getProperty("user.home"), ".local/share/nemo/notify_mode")

/** Desktop has no background delivery: the app syncs only while it runs. */
internal actual fun applyNotifyMode(mode: NemoNotifyMode) {
    // No-op.
}

internal actual fun publishClient(client: NemoClient?) {
    // No-op.
}

internal actual fun areNotificationsAllowed(): Boolean = true

internal actual fun openSystemNotificationSettings() {
    // No-op.
}

@Composable
internal actual fun rememberEnsureNotifications(onReady: () -> Unit): () -> Unit {
    val latest = rememberUpdatedState(onReady)
    return { latest.value.invoke() }
}
