package org.nemo

import uniffi.nemo.DisplayRow

/** Rows that render as chat bubbles. Reaction pills, transient remove markers
 * and legacy soft-delete tombstones never render as bubbles. */
internal fun isBubbleRow(row: DisplayRow): Boolean = when (row.kind) {
    "reaction", "reaction_removed", "removed", "deleted" -> false
    else -> true
}

/** Last bubble row for chat-list preview / unread counts. Skips pills and markers. */
internal fun lastBubble(messages: List<DisplayRow>, convId: String): DisplayRow? =
    messages.lastOrNull { it.convId == convId && isBubbleRow(it) }

/** One author's reaction entry for the author sheet (multi-emoji style:
 * one user may hold multiple different emojis on the same message). */
internal data class ReactionAuthor(val emoji: String, val displayName: String, val mine: Boolean)

/** Aggregated pill entry: one emoji + count + whether I reacted + authors. */
internal data class ReactionPill(val emoji: String, val count: Int, val mine: Boolean, val authors: List<ReactionAuthor>)

private fun reactionDisplayName(row: DisplayRow, contacts: Map<String, String> = emptyMap()): String {
    if (row.outgoing) return "You"
    if (row.senderName.isNotBlank()) return row.senderName
    val id = row.senderId.ifBlank { row.convId }
    contacts[id]?.takeIf { it.isNotBlank() }?.let { return it }
    return shortId(id).ifBlank { "They" }
}

/** Grouping key for reactions: strip variation selectors and skin-tone
 * modifiers so the same emoji in different normalizations (bare vs VS16,
 * with/without tone) aggregates into one pill instead of splitting. */
internal fun reactionGroupKey(emoji: String): String {
    val sb = StringBuilder()
    var i = 0
    while (i < emoji.length) {
        val cp = emoji.codePointAt(i)
        i += Character.charCount(cp)
        if (cp == 0xFE0E || cp == 0xFE0F) continue
        if (cp in 0x1F3FB..0x1F3FF) continue
        sb.appendCodePoint(cp)
    }
    return sb.toString().ifEmpty { emoji }
}

/** All reactions anchored to [target] in [messages], grouped by emoji. */
internal fun reactionsFor(
    messages: List<DisplayRow>,
    convId: String,
    target: ULong,
    contacts: Map<String, String> = emptyMap(),
): List<ReactionPill> {
    if (target == 0UL) return emptyList()
    val rows = messages.filter {
        it.convId == convId && it.kind == "reaction" && it.target == target
    }
    if (rows.isEmpty()) return emptyList()
    return rows.groupBy { reactionGroupKey(it.emoji) }.map { (_, group) ->
        val emoji = group.first().emoji
        val authors = group.map { r ->
            ReactionAuthor(
                emoji = emoji,
                displayName = reactionDisplayName(r, contacts),
                mine = r.outgoing,
            )
        }
        ReactionPill(
            emoji = emoji,
            count = group.size,
            mine = group.any { it.outgoing },
            authors = authors,
        )
    }.sortedByDescending { it.count }
}

/** Quoted original for a reply. Null when none or when the original was
 * hard-deleted (quote hidden per spec, never tombstoned). */
internal fun replyOriginal(messages: List<DisplayRow>, row: DisplayRow): DisplayRow? {
    if (row.replyTo == 0UL) return null
    return messages.firstOrNull {
        it.convId == row.convId && it.convSeq == row.replyTo && isBubbleRow(it)
    }
}

internal fun shortQuoteText(text: String, max: Int = 80): String {
    val single = text.replace("\n", " ").trim()
    return if (single.length <= max) single else single.take(max - 1) + "…"
}

/** Quick-pick row. Full picker is the system emoji keyboard via draft. */
internal val QUICK_REACTIONS = listOf("❤️", "👍", "🔥", "🎉", "😂", "😮", "😢", "🙏")
