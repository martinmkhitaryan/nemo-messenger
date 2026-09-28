package org.nemo

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters

/**
 * Fallback delivery when the persistent connection is off: one fetch per
 * run, notifying for anything new since the last run. No client (vault
 * locked) means nothing to do — success so WorkManager keeps the schedule.
 *
 * Uses the single [NemoVaultStore] owner when present so the fetch cannot
 * steal rows from the open list; falls back to direct [syncNow] otherwise.
 */
internal class PollWorker(private val workerCtx: Context, params: WorkerParameters) : CoroutineWorker(workerCtx, params) {
    override suspend fun doWork(): Result {
        val client = LocalClient.client ?: return Result.success()
        val store = VaultStores.findByClient(client)
        // Use the worker's own context: after a system restart WorkManager can
        // run before any activity sets the static appContext.
        val ctx = applicationContext
        if (store != null) {
            runCatching { syncStoreAndNotify(store, ctx) }
        } else {
            runCatching { syncNow(client) }
        }
        return Result.success()
    }
}
