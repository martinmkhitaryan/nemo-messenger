package org.nemo

import uniffi.nemo.DisplayRow
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/** One-line chat-list preview. Never surfaces raw engine strings.
 * Reaction pills and transient remove markers never become previews. */
internal fun previewLine(row: DisplayRow): String = when {
    row.kind == "reaction" || row.kind == "reaction_removed" || row.kind == "removed" -> ""
    row.hidden && row.kind == "expired" -> "Message expired"
    // Legacy vaults / old peers may still carry soft-delete tombstones.
    row.hidden || row.kind == "deleted" -> "Message deleted"
    row.kind == "call_invite" -> "Incoming call"
    row.kind == "call_ringing" -> "Ringing"
    row.kind == "call_answer" -> "Call answered"
    row.kind == "call_reject" -> "Declined"
    row.kind == "call_cancel" -> "Cancelled"
    row.kind == "call_end" -> "Call ended"
    row.kind == "lost" -> "Messages lost"
    row.kind == "revoked" -> "Identity revoked"
    row.kind == "binding_conflict" -> "Home-server binding conflict"
    row.fileName.isNotEmpty() -> "📎 ${row.fileName}"
    isJoinRequestUri(row.text) -> "Group join request"
    isGroupInviteUri(row.text) -> "Group invite"
    else -> row.text
}

/** Bubble body for system rows; user content passes through untouched.
 * Reaction pills render under the target bubble, not as bubbles. */
internal fun bubbleText(row: DisplayRow): String = when {
    row.hidden && row.kind == "expired" -> "Expired"
    // Legacy tombstones from old vaults/peers.
    row.hidden || row.kind == "deleted" -> "Deleted"
    row.kind == "disappear" -> "Disappearing messages: ${row.text}s"
    row.kind == "call_invite" -> "Incoming call"
    row.kind == "call_ringing" -> "Ringing"
    row.kind == "call_answer" -> "Answered"
    row.kind == "call_reject" -> "Declined"
    row.kind == "call_cancel" -> "Cancelled"
    row.kind == "call_end" -> "Call ended"
    row.kind == "lost" -> "Messages lost"
    row.kind == "revoked" -> "Identity revoked"
    row.kind == "binding_conflict" -> "Home-server binding conflict"
    row.fileName.isNotEmpty() -> "📎 ${row.fileName}"
    else -> row.text
}

/** True for short emoji-only texts, rendered large with no bubble. */
internal fun isBigEmojiMessage(row: DisplayRow): Boolean {
    if (row.kind != "" || row.fileName.isNotEmpty() || row.replyTo != 0UL) return false
    if (isJoinRequestUri(row.text) || isGroupInviteUri(row.text)) return false
    val text = row.text.trim()
    if (text.isEmpty()) return false
    val cps = text.codePoints().toArray()
    if (cps.isEmpty() || cps.size > 3) return false
    return cps.all(::isEmojiCodePoint)
}

private fun isEmojiCodePoint(cp: Int): Boolean {
    if (cp == 0x200D || cp in 0xFE00..0xFE0F) return true
    if (cp > 0xFFFF) return true
    if (cp in 0x2190..0x2BFF || cp in 0x2300..0x23FF) return true
    val type = Character.getType(cp)
    return type == Character.OTHER_SYMBOL.toInt() || type == Character.MODIFIER_SYMBOL.toInt()
}

internal fun formatTime(sentAt: ULong): String {
    if (sentAt == 0UL) return ""
    return try {
        SimpleDateFormat("HH:mm", Locale.getDefault()).format(Date(sentAt.toLong() * 1000))
    } catch (_: Throwable) {
        ""
    }
}

/** Join-request / invite URIs travel as plain 1:1 text in the auto-relay
 * variant; the manual copy-paste variant stays. Recognition is prefix-only —
 * validation happens in `admitJoin`. */
internal fun isJoinRequestUri(text: String): Boolean = text.trimStart().startsWith("nemo-j:1:")

internal fun isGroupInviteUri(text: String): Boolean = text.trimStart().startsWith("nemo-g:1:")

/** Single-message cap shared with the engine (`TEXT_MAX_BYTES`). Bytes, not
 * chars — emoji and non-Latin scripts weigh more. */
internal const val TEXT_MAX_BYTES: Int = 8192

internal fun draftByteSize(text: String): Int = text.toByteArray().size

internal fun isOverTextLimit(text: String): Boolean = draftByteSize(text.trim()) > TEXT_MAX_BYTES

/** Short identity label: first 6 chars plus ellipsis when long. */
internal fun shortId(id: String) = if (id.length <= 10) id else "${id.take(6)}…"

/** True when a bubble offers Save: an unhidden file row with payload bytes. */
internal fun canSaveAttachment(row: DisplayRow): Boolean = row.fileName.isNotEmpty() && row.fileBytes.isNotEmpty() && !row.hidden
