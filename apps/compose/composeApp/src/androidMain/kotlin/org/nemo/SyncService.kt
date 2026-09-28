package org.nemo

import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/**
 * Foreground service keeping the process alive while backgrounded.
 * Android mandates the ongoing notification this service posts — that is
 * the visible price of real-time delivery without a push provider.
 *
 * Data ownership lives in [NemoVaultStore]: it runs the single
 * waitWakeup/poll loops and publishes [NemoVaultStore.messagesFlow] state.
 * This service only ensures the store runs and diffs that replayed state
 * against the persisted notify mark, so late subscribers never miss a
 * notification (fresh-row events alone would be lossy). Suppression honors
 * [NemoVaultStore.visibleChatId] + [NemoVaultStore.appForeground]. It never
 * calls `fetchNow()` itself, so it cannot steal rows from the open chat
 * list. Before unlock it idles with an "unlock to resume" note until
 * [publishClient] hands it a client.
 */
internal class SyncService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var loop: Job? = null

    override fun onBind(intent: Intent?): IBinder? = null

    private class StaleIdentity : Exception()

    override fun onCreate() {
        super.onCreate()
        ensureChannels(this)
        startForegroundLocked(false)
        loop = scope.launch {
            while (isActive) {
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
                // Clearing the tray row when its chat is opened: the user sees
                // the thread, so the alert must not linger and silence the
                // next message (per-conversation row, see messageNotifyId).
                val svcCtx = this@SyncService
                val clearJob = launch {
                    store.visibleChatId.collect { id ->
                        if (stamp != LocalClient.generation) throw StaleIdentity()
                        if (id != null) {
                            runCatching {
                                NotificationManagerCompat.from(svcCtx).cancel(messageNotifyId(store, id))
                            }
                        }
                    }
                }
                try {
                    // State, not events: messagesFlow replays the latest snapshot,
                    // so a collector that subscribes late still diffs correctly.
                    // Use the service itself as Context: it is valid even if no
                    // activity has run yet in this process.
                    // The timeout re-resolves the registry: if the store was ever
                    // replaced/closed under us, this collect would otherwise park
                    // forever on a dead flow (checks only run on emission).
                    kotlinx.coroutines.withTimeout(30_000) {
                        store.messagesFlow.collect { messages ->
                            if (stamp != LocalClient.generation) throw StaleIdentity()
                            val live = LocalClient.client
                            if (live == null || VaultStores.findByClient(live) !== store) {
                                throw StaleIdentity()
                            }
                            if (messages.isEmpty()) return@collect
                            notifyMessagesState(svcCtx, store, messages)
                        }
                    }
                } catch (e: StaleIdentity) {
                    continue
                } catch (_: kotlinx.coroutines.TimeoutCancellationException) {
                    // Idle (or wedged) with no emission for 30 s: loop around
                    // and re-resolve client/store. Re-collect replays state.
                    continue
                } catch (_: Throwable) {
                    delay(2_000)
                } finally {
                    clearJob.cancel()
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
