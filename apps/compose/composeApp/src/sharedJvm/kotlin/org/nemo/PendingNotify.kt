package org.nemo

import uniffi.nemo.DisplayRow

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
