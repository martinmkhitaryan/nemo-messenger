package org.nemo

import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import kotlinx.coroutines.CoroutineExceptionHandler
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.takeWhile
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlin.coroutines.cancellation.CancellationException

/**
 * Foreground service keeping the process alive while backgrounded.
 * Android mandates the ongoing notification this service posts — that is
 * the visible price of real-time delivery without a push provider.
 *
 * Data ownership lives in [NemoVaultStore]: it runs the single
 * waitWakeup/poll loops and publishes [NemoVaultStore.messagesFlow] state.
 * This service only ensures the store runs and diffs that replayed state
 * against the persisted notify mark, so late subscribers never miss a
 * notification (fresh-row events alone would be lossy). Suppression at post
 * time honors [NemoVaultStore.visibleChatId] + [NemoVaultStore.appForeground]
 * (see `postPending`). It never calls `fetchNow()` itself, so it cannot
 * steal rows from the open chat list.
 *
 * Decoupled by design: the service never observes UI state. Clearing a
 * conversation's tray row when its chat opens is owned by the UI
 * ([dismissTrayForChat] from `BindVaultStore`). Collecting UI flows here
 * used to force-quit the app via an uncaught child exception on every
 * identity switch — that coupling is gone.
 *
 * The loop below is exception-safe: any unexpected failure is logged and
 * retried, never propagated. A service loop must not be able to take down
 * the process; only teardown cancellation escapes.
 */
internal class SyncService : Service() {
    private val scope = CoroutineScope(
        SupervisorJob() + Dispatchers.IO +
            CoroutineExceptionHandler { _, e -> Log.e(TAG, "SyncService uncaught", e) },
    )
    private var loop: Job? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        ensureChannels(this)
        startForegroundLocked(false)
        loop = scope.launch {
            while (isActive) {
                try {
                    val client = LocalClient.client
                    if (client == null) {
                        startForegroundLocked(false)
                        delay(5_000)
                        continue
                    }
                    startForegroundLocked(true)
                    val store = VaultStores.findByClient(client)
                    if (store == null) {
                        // No owner yet (unlock race) or legacy caller: fall back to
                        // direct fetch so delivery never stalls. Production always
                        // has a store; tests exercise this path via syncNow.
                        val stamp = LocalClient.generation
                        val woke = runCatching { client.waitWakeup() }
                        try {
                            // Vault wiped/recreated mid-wait: this wake belongs to a
                            // dead identity. Drop it; the next iteration serves the
                            // new client immediately.
                            if (stamp != LocalClient.generation) continue
                            woke.getOrThrow()
                            LocalClient.client?.let { syncNow(it) }
                        } catch (_: Throwable) {
                            delay(2_000)
                        }
                        continue
                    }
                    store.start()
                    val stamp = LocalClient.generation
                    // State, not events: messagesFlow replays the latest snapshot,
                    // so a collector that subscribes late still diffs correctly.
                    // Use the service itself as Context: it is valid even if no
                    // activity has run yet in this process.
                    // The timeout re-resolves the registry: if the store was ever
                    // replaced/closed under us, this collect would otherwise park
                    // forever on a dead flow (checks only run on emission).
                    // Identity switches end the flow normally via takeWhile (no
                    // exceptions for control flow); the loop re-resolves below.
                    val svcCtx = this@SyncService
                    try {
                        kotlinx.coroutines.withTimeout(30_000) {
                            store.messagesFlow
                                .takeWhile {
                                    val live = LocalClient.client
                                    stamp == LocalClient.generation &&
                                        live != null &&
                                        VaultStores.findByClient(live) === store
                                }
                                .collect { messages ->
                                    if (messages.isEmpty()) return@collect
                                    notifyMessagesState(svcCtx, store, messages)
                                }
                        }
                    } catch (_: kotlinx.coroutines.TimeoutCancellationException) {
                        // Idle (or wedged) with no emission for 30 s: loop around
                        // and re-resolve client/store. Re-collect replays state.
                    }
                } catch (_: kotlinx.coroutines.TimeoutCancellationException) {
                    // Backstop for timeout paths without their own handler.
                    continue
                } catch (e: CancellationException) {
                    // Service torn down (scope cancelled): must propagate so
                    // onDestroy actually stops the loop.
                    throw e
                } catch (t: Throwable) {
                    // Seatbelt: a service loop must never take down the
                    // process. Log and retry; teardown still escapes above.
                    Log.e(TAG, "SyncService loop failed, restarting", t)
                    delay(2_000)
                }
            }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_REFRESH) {
            startForegroundLocked(LocalClient.client != null)
        }
        return START_STICKY
    }

    override fun onDestroy() {
        loop?.cancel()
        scope.cancel()
        super.onDestroy()
    }

    private fun startForegroundLocked(connected: Boolean) {
        val text = if (connected) {
            "Listening for messages"
        } else {
            "Unlock Nemo to resume sync"
        }
        val notification = NotificationCompat.Builder(this, syncChannelId())
            .setSmallIcon(org.nemo.shared.R.drawable.ic_nemo_notify)
            .setLargeIcon(
                android.graphics.BitmapFactory.decodeResource(
                    resources,
                    org.nemo.shared.R.drawable.nemo_brand_round,
                ),
            )
            .setContentTitle("Nemo")
            .setContentText(text)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .build()
        ServiceCompat.startForeground(
            this,
            SYNC_NOTIFICATION_ID,
            notification,
            ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC,
        )
    }

    companion object {
        private const val TAG = "SyncService"
        private const val SYNC_NOTIFICATION_ID = 1
        private const val ACTION_REFRESH = "org.nemo.REFRESH_SYNC"

        fun start(ctx: Context) {
            ContextCompat.startForegroundService(ctx, Intent(ctx, SyncService::class.java))
        }

        fun stop(ctx: Context) {
            ctx.stopService(Intent(ctx, SyncService::class.java))
        }

        /** Re-render the sticky note (e.g. vault locked/unlocked). */
        fun refresh(ctx: Context) {
            ctx.startService(Intent(ctx, SyncService::class.java).setAction(ACTION_REFRESH))
        }
    }
}
