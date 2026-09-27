package org.nemo

import uniffi.nemo.NemoClient

/**
 * Hand the unlocked client to the platform background-sync machinery, or null
 * when the vault is locked/wiped. Declared here (not commonMain) because only
 * sharedJvm and below can see [NemoClient]. Desktop is a no-op.
 */
internal expect fun publishClient(client: NemoClient?)
