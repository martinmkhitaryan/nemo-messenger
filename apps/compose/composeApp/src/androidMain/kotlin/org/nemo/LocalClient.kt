package org.nemo

import android.content.Context
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import uniffi.nemo.NemoClient
import java.util.concurrent.TimeUnit

/**
 * Process-global handle on the unlocked vault client for background sync.
 * Set on unlock/create, cleared on lock/wipe. After a reboot there is no
 * client until the user unlocks once — the vault passphrase lives only in
 * the UI process memory, so background sync cannot start sooner.
 */
internal object LocalClient {
    @Volatile
    var client: NemoClient? = null
}

internal actual fun publishClient(client: NemoClient?) {
    LocalClient.client = client
    if (client == null) {
        // Vault locked or wiped: nothing to sync with. The service idles
        // until the next unlock; the worker no-ops without a client.
        appContext?.let { SyncService.refresh(it) }
    } else {
        appContext?.let { applyNotifyMode(loadNotifyMode()) }
    }
}

internal actual fun applyNotifyMode(mode: NemoNotifyMode) {
    val ctx = appContext ?: return
    when (mode) {
        NemoNotifyMode.Foreground -> {
            cancelPoll(ctx)
            SyncService.start(ctx)
        }
        NemoNotifyMode.Poll -> {
            SyncService.stop(ctx)
            schedulePoll(ctx)
        }
    }
}

private const val POLL_WORK = "nemo-poll"

private fun schedulePoll(ctx: Context) {
    val req = PeriodicWorkRequestBuilder<PollWorker>(15, TimeUnit.MINUTES)
        .setConstraints(
            Constraints.Builder()
                .setRequiredNetworkType(NetworkType.CONNECTED)
                .build(),
        )
        .build()
    WorkManager.getInstance(ctx).enqueueUniquePeriodicWork(
        POLL_WORK,
        ExistingPeriodicWorkPolicy.KEEP,
        req,
    )
}

private fun cancelPoll(ctx: Context) {
    WorkManager.getInstance(ctx).cancelUniqueWork(POLL_WORK)
}
