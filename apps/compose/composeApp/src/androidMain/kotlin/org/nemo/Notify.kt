package org.nemo

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.platform.LocalContext
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import org.nemo.shared.R
import uniffi.nemo.NemoClient
import java.io.File

private const val SYNC_CHANNEL = "nemo_sync"
private const val MSG_CHANNEL = "nemo_messages"
private const val SEEN_FILE = "notify_seen"

/**
 * One wake-up fetch from the home server, notifying for new incoming rows.
 * Shared by the foreground service (real-time) and the periodic worker
 * (fallback). Never surfaces message content: title is the chat, body is a
 * generic label. System rows (reactions, receipts, deletions, revocations)
 * stay silent.
 */
internal fun syncNow(client: NemoClient) {
    val ctx = appContext ?: return
    val rows = runCatching { client.fetchNow() }.getOrNull() ?: return
    val seen = loadSeen(ctx)
    val (pending, next) = pendingNotifies(rows, seen)
    if (next != seen) saveSeen(ctx, next)
    if (pending.isEmpty()) return
    val titles = conversationTitles(client)
    for (item in pending) {
        notifyMessage(ctx, titles[item.convId] ?: shortId(item.convId), item.kind)
    }
}

private fun notifyMessage(ctx: Context, title: String, kind: String) {
    ensureChannels(ctx)
    if (ContextCompat.checkSelfPermission(ctx, Manifest.permission.POST_NOTIFICATIONS) !=
        PackageManager.PERMISSION_GRANTED
    ) {
        return
    }
    val open = PendingIntent.getActivity(
        ctx,
        0,
        Intent(ctx, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP),
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
    )
    val text = when (kind) {
        "call_invite", "call_ringing" -> "Incoming call"
        else -> "New message"
    }
    val notification = NotificationCompat.Builder(ctx, MSG_CHANNEL)
        .setSmallIcon(R.drawable.ic_nemo_notify)
        .setLargeIcon(BitmapFactory.decodeResource(ctx.resources, R.drawable.nemo_brand_round))
        .setContentTitle(title)
        .setContentText(text)
        .setOnlyAlertOnce(true)
        .setAutoCancel(true)
        .setContentIntent(open)
        .build()
    NotificationManagerCompat.from(ctx).notify(title.hashCode(), notification)
}

internal fun ensureChannels(ctx: Context) {
    val manager = ctx.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
    if (manager.getNotificationChannel(SYNC_CHANNEL) == null) {
        manager.createNotificationChannel(
            NotificationChannel(
                SYNC_CHANNEL,
                "Background sync",
                NotificationManager.IMPORTANCE_LOW,
            ),
        )
    }
    if (manager.getNotificationChannel(MSG_CHANNEL) == null) {
        manager.createNotificationChannel(
            NotificationChannel(
                MSG_CHANNEL,
                "Messages",
                NotificationManager.IMPORTANCE_DEFAULT,
            ),
        )
    }
}

internal fun syncChannelId(): String = SYNC_CHANNEL

internal actual fun areNotificationsAllowed(): Boolean {
    val ctx = appContext ?: return true
    return NotificationManagerCompat.from(ctx).areNotificationsEnabled()
}

internal actual fun openSystemNotificationSettings() {
    val ctx = appContext ?: return
    val intent = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
        Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, ctx.packageName)
    } else {
        Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", ctx.packageName, null))
    }.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    runCatching { ctx.startActivity(intent) }
}

@androidx.compose.runtime.Composable
internal actual fun rememberEnsureNotifications(onReady: () -> Unit): () -> Unit {
    val latest = androidx.compose.runtime.rememberUpdatedState(onReady)
    val ctx = LocalContext.current
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {
        latest.value.invoke()
    }
    return {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(ctx, Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED
        ) {
            latest.value.invoke()
        } else {
            launcher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }
}

private fun conversationTitles(client: NemoClient): Map<String, String> {
    val titles = mutableMapOf<String, String>()
    runCatching { client.listContacts() }.getOrNull()?.forEach {
        titles[it.identityId] = it.nickname
    }
    runCatching { client.listGroups() }.getOrNull()?.forEach {
        titles[it.groupId] = it.nickname.ifBlank { "Group" }
    }
    return titles
}

private fun loadSeen(ctx: Context): Map<String, ULong> {
    val file = File(ctx.filesDir, SEEN_FILE)
    if (!file.isFile) return emptyMap()
    return file.readLines().mapNotNull { line ->
        val parts = line.split(' ', limit = 2)
        if (parts.size != 2) return@mapNotNull null
        val seq = parts[1].toULongOrNull() ?: return@mapNotNull null
        parts[0] to seq
    }.toMap()
}

private fun saveSeen(ctx: Context, seen: Map<String, ULong>) {
    val text = seen.entries.joinToString("\n") { (id, seq) -> "$id $seq" }
    runCatching { File(ctx.filesDir, SEEN_FILE).writeText(text) }
}
