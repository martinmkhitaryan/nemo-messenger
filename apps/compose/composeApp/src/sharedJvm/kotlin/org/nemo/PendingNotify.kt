package org.nemo

import uniffi.nemo.DisplayRow
import java.io.File

internal const val UNREAD_SEEN_FILE = "unread_seen"

/** An incoming row worth interrupting the user for (content never included). */
internal data class PendingNotify(val convId: String, val kind: String)

/** Kinds worth a sound/vibration. Everything else merges silently. */
internal fun notifiable(kind: String): Boolean = when (kind) {
    "",
    "call_invite",
    "call_ringing",
    -> true
    else -> false
}

/**
 * Kinds that increment the chat-list badge (option 2: user content + missed
 * calls). All other system rows (reactions, deletions, call end states,
 * disappear notices, lost/revoked/binding_conflict, …) update the preview
 * only and never badge.
 */
internal fun countableForBadge(kind: String): Boolean = when (kind) {
    "",
    "call_invite",
    "call_ringing",
    -> true
    else -> false
}

/**
 * Pure badge core: incoming countable rows for [convId] newer than the
 * per-conversation high-water mark in [lastRead].
 *
 * Skipped without counting: optimistic locals, own rows, hidden rows, and
 * already-read sequences. Non-countable system rows never increment but the
 * caller is expected to still advance [lastRead] past them (see
 * [markConversationRead]) so they never re-badge.
 */
internal fun unreadCount(convId: String, messages: List<DisplayRow>, lastRead: Map<String, ULong>): Int {
    val mark = lastRead[convId] ?: 0UL
    var n = 0
    for (row in messages) {
        if (row.convId != convId) continue
        if (row.fetchToken.startsWith("local:")) continue
        if (row.outgoing || row.hidden) continue
        if (row.convSeq <= mark) continue
        if (!countableForBadge(row.kind)) continue
        n++
    }
    return n
}

/** Highest sequence seen for [convId] (any kind, including systems). */
internal fun maxSeqForConv(messages: List<DisplayRow>, convId: String): ULong? =
    messages.filter { it.convId == convId }.maxOfOrNull { it.convSeq }

/** Advance [lastRead] past every row of [convId] (systems included). */
internal fun markConversationRead(lastRead: MutableMap<String, ULong>, messages: List<DisplayRow>, convId: String): Boolean {
    val max = maxSeqForConv(messages, convId) ?: return false
    if ((lastRead[convId] ?: 0UL) >= max) return false
    lastRead[convId] = max
    return true
}

/** True if [row] can start/contribute to the unread block (badge rule). */
internal fun isBadgeRow(row: DisplayRow): Boolean {
    if (row.fetchToken.startsWith("local:")) return false
    if (row.outgoing || row.hidden) return false
    return countableForBadge(row.kind)
}

/**
 * Oldest countable sequence for [convId] newer than [mark] (jump target /
 * divider anchor). Null when nothing unread.
 */
internal fun firstUnreadSeq(messages: List<DisplayRow>, convId: String, mark: ULong): ULong? = messages
    .filter { it.convId == convId && it.convSeq > mark && isBadgeRow(it) }
    .minOfOrNull { it.convSeq }

/**
 * Same as [firstUnreadSeq] but ignores rows newer than [maxSeq], so rows
 * arriving while the thread is open never raise the marker. They are still
 * counted by [unreadCount] until seen.
 */
internal fun firstUnreadSeqCapped(messages: List<DisplayRow>, convId: String, mark: ULong, maxSeq: ULong): ULong? = messages
    .filter { it.convId == convId && it.convSeq > mark && it.convSeq <= maxSeq && isBadgeRow(it) }
    .minOfOrNull { it.convSeq }

/**
 * Display index (newest-first order, marker sits right after the anchor
 * bubble) of the unread marker, or null when nothing unread within [maxSeq].
 * [messages] must be chronological (oldest first).
 */
internal fun unreadDividerAt(messages: List<DisplayRow>, convId: String, mark: ULong, maxSeq: ULong): Int? {
    val seq = firstUnreadSeqCapped(messages, convId, mark, maxSeq) ?: return null
    val chrono = messages.indexOfFirst { it.convId == convId && it.convSeq == seq }
    if (chrono < 0) return null
    return (messages.lastIndex - chrono) + 1
}

/**
 * Display index (newest-first order plus the marker slot) of the row at
 * [chronoIndex]. [lastIndex] is `messages.lastIndex`.
 */
internal fun displayIndexFor(chronoIndex: Int, lastIndex: Int, dividerAt: Int?): Int {
    val newestIndex = lastIndex - chronoIndex
    return if (dividerAt != null && newestIndex >= dividerAt) newestIndex + 1 else newestIndex
}

/**
 * Countable unread rows sitting below the viewport ([display][displayIndexFor]
 * below [firstVisible]). Drives the jump-to-latest overlay.
 */
internal fun belowUnreadCount(messages: List<DisplayRow>, convId: String, mark: ULong, dividerAt: Int?, firstVisible: Int): Int {
    if (firstVisible <= 0) return 0
    var n = 0
    for (chrono in messages.indices) {
        val row = messages[chrono]
        if (row.convId != convId) continue
        if (row.convSeq <= mark || !isBadgeRow(row)) continue
        if (displayIndexFor(chrono, messages.lastIndex, dividerAt) < firstVisible) n++
    }
    return n
}

/**
 * Bottom-edge scroll position that puts the marker at the top of a
 * [visibleSize]-row window with [contextAbove] seen rows above it.
 * Caller clamps so the anchor bubble stays on screen.
 */
internal fun markerTopTarget(dividerAt: Int, visibleSize: Int, contextAbove: Int): Int =
    (dividerAt + contextAbove - visibleSize.coerceAtLeast(1) + 1).coerceAtLeast(0)

/**
 * Per-vault badge watermark. Stored as `"<convId> <seq>"` lines in
 * `[vaultDir]/unread_seen` (one pane = one vault, so desktop Left/Right stay
 * independent). Returns null when no watermark exists yet — the caller should
 * then mark current history read instead of badging everything on first
 * unlock. Separate from `notify_seen`: clearing a badge must not re-arm a
 * notification and vice versa.
 */
internal fun loadUnreadSeen(vaultDir: File): Map<String, ULong>? {
    val file = File(vaultDir, UNREAD_SEEN_FILE)
    if (!file.isFile) return null
    return runCatching {
        file.readLines().mapNotNull { line ->
            val parts = line.split(' ', limit = 2)
            if (parts.size != 2) return@mapNotNull null
            val seq = parts[1].toULongOrNull() ?: return@mapNotNull null
            parts[0] to seq
        }.toMap()
    }.getOrNull() ?: emptyMap()
}

internal fun saveUnreadSeen(vaultDir: File, seen: Map<String, ULong>) {
    runCatching {
        vaultDir.mkdirs()
        val text = seen.entries.joinToString("\n") { (id, seq) -> "$id $seq" }
        File(vaultDir, UNREAD_SEEN_FILE).writeText(text)
    }
}

/**
 * Pure core of background sync: fold fetched [rows] into the per-conversation
 * high-water marks, returning chat notifications for new incoming rows.
 *
 * Skipped without advancing the mark: optimistic locals, own rows, hidden
 * rows, and already-seen sequences. Non-notifiable kinds (reactions,
 * deletions, call end states, …) still advance the mark so they never
 * re-trigger, but produce no notification.
 */
internal fun pendingNotifies(rows: List<DisplayRow>, seen: Map<String, ULong>): Pair<List<PendingNotify>, Map<String, ULong>> {
    val next = seen.toMutableMap()
    val out = mutableListOf<PendingNotify>()
    for (row in rows) {
        if (row.fetchToken.startsWith("local:")) continue
        if (row.outgoing || row.hidden) continue
        if (row.convSeq <= (next[row.convId] ?: 0UL)) continue
        next[row.convId] = maxOf(next[row.convId] ?: 0UL, row.convSeq)
        if (notifiable(row.kind)) out += PendingNotify(row.convId, row.kind)
    }
    return out to next
}
