package org.nemo

import androidx.compose.runtime.Composable

/** Platform path helpers and side-effecting APIs. Paths are absolute filesystem strings. */
expect fun pickLocalFile(): String?

expect fun deviceVaultSecret(vaultPath: String): ByteArray

expect fun defaultHomeUrl(): String

expect fun copyToClipboard(text: String)

/** System / gesture back. Android only; desktop is a no-op. */
@Composable
expect fun NemoBackHandler(enabled: Boolean = true, onBack: () -> Unit)
