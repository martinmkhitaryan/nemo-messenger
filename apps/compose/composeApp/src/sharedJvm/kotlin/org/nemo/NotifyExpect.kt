package org.nemo

import uniffi.nemo.NemoClient

/**
 * Hand the unlocked client to the platform background-sync machinery, or null
 * when the vault is locked/wiped. Declared here (not commonMain) because only
 * sharedJvm and below can see [NemoClient]. Desktop is a no-op.
 */
internal expect fun publishClient(client: NemoClient?)

/**
 * Clear this conversation's tray row now that the user sees the thread.
 * UI-side: called when a chat opens. Android cancels the row; desktop is a
 * no-op. Lives behind expect/actual because only platform code can touch
 * the notification tray.
 */
internal expect fun dismissTrayForChat(store: NemoVaultStore, convId: String)
