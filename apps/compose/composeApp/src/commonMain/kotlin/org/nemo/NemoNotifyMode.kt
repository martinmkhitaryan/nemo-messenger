package org.nemo

import androidx.compose.runtime.Composable

/**
 * Background message delivery while the app is not in the foreground.
 *
 * Foreground keeps a persistent home-server connection and is real-time, but
 * Android requires an ongoing (non-dismissible) notification for it. Poll
 * wakes periodically instead: no ongoing notification, but messages arrive
 * late. Either way the device fetches and decrypts its own ciphertext;
 * nothing is ever delegated to a push provider.
 */
internal enum class NemoNotifyMode {
    Foreground,
    Poll,
    ;

    val label: String
        get() = when (this) {
            Foreground -> "Persistent connection"
            Poll -> "Periodic check"
        }

    val description: String
        get() = when (this) {
            Foreground -> "Real-time. Shows a silent ongoing notification."
            Poll -> "About every 15 minutes. No ongoing notification; messages arrive late."
        }
}

internal expect fun loadNotifyMode(): NemoNotifyMode

internal expect fun saveNotifyMode(mode: NemoNotifyMode)

/**
 * Start/stop the platform background-sync machinery for [mode].
 * Desktop is a no-op (delivery only happens while the app runs).
 */
internal expect fun applyNotifyMode(mode: NemoNotifyMode)

/** Runs [onReady] immediately on desktop; requests the notifications permission on Android. */
@Composable
internal expect fun rememberEnsureNotifications(onReady: () -> Unit): () -> Unit

/** True when the system will actually display our notifications. Always true on desktop. */
internal expect fun areNotificationsAllowed(): Boolean

/** Opens the OS notification settings for this app. No-op on desktop. */
internal expect fun openSystemNotificationSettings()
