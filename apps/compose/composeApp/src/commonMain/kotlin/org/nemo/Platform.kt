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

/**
 * Mobile send-morph duration (ms). Mobile morphs composer → bubble (~480 ms).
 * Desktop has no morph — new bubbles appear in place (250 ms,
 * cubic-bezier(.4,0,.2,1)). See MessageSendAnimation header.
 */
internal expect fun messageSendFlyDurationMs(): Int

/**
 * True on mobile: use the composer → bubble morph overlay.
 * False on desktop: no morph, bubbles appear in place.
 */
internal expect fun messageSendFlyMobileFeel(): Boolean
