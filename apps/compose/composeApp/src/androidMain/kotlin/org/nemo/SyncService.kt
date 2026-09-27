package org.nemo

import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import androidx.core.app.NotificationCompat
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
 * Foreground service holding the home-server wake socket while the app is
 * backgrounded. Android mandates the ongoing notification this service
 * posts — that is the visible price of real-time delivery without a push
 * provider. Runs only after unlock; before that it idles with an
 * "unlock to resume" note until [publishClient] hands it a client.
 */
internal class SyncService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private var loop: Job? = null

    override fun onBind(intent: Intent?): IBinder? = null

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
                try {
                    client.waitWakeup()
                    syncNow(client)
                } catch (_: Throwable) {
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
