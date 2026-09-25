package org.nemo

import uniffi.nemo.NemoClient

/**
 * Call-audio hooks need the UniFFI [NemoClient] type, which lives in sharedJvm.
 * Expects stay here; actuals remain in androidMain / desktopMain.
 */
expect fun startCallAudio(client: NemoClient)

expect fun stopCallAudio()
