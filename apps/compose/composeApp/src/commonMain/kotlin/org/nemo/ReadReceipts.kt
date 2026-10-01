package org.nemo

/**
 * Read receipts (human-viewed ticks) as a device preference. Independent of
 * the vault: wiping or relocking the vault keeps the user's choice.
 *
 * Enabled by default. When off, the shell never sends `mark_read` and caps
 * displayed ticks at delivered — mirroring the usual convention where
 * hiding your own receipts also hides others'.
 */
internal expect fun loadReadReceiptsEnabled(): Boolean

internal expect fun saveReadReceiptsEnabled(enabled: Boolean)
