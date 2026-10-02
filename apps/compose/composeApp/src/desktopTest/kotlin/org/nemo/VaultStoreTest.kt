package org.nemo

import uniffi.nemo.DisplayRow
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class VaultStoreTest {
    @Test
    fun mergeDedupsByConvSeqKind() {
        val base = listOf(row(convSeq = 1UL), row(convSeq = 2UL))
        val merged = mergeDisplayRows(base, listOf(row(convSeq = 2UL), row(convSeq = 3UL)))
        assertEquals(3, merged.size)
        // Re-applying the full snapshot (UI collect race) must not duplicate.
        assertEquals(3, mergeDisplayRows(merged, base).size)
        assertEquals(3, mergeDisplayRows(merged, merged).size)
    }

    @Test
    fun mergeHidesDeleteTarget() {
        val base = listOf(row(convSeq = 3UL, text = "hello"))
        val merged = mergeDisplayRows(base, listOf(row(kind = "deleted", convSeq = 9UL, target = 3UL, text = "")))
        assertEquals(2, merged.size)
        val hidden = merged.first { it.convSeq == 3UL }
        assertTrue(hidden.hidden)
        assertEquals("deleted", hidden.kind)
    }

    @Test
    fun mergeKeepsConversationsIndependent() {
        val merged = mergeDisplayRows(
            listOf(row(convId = "aa", convSeq = 1UL)),
            listOf(row(convId = "bb", convSeq = 1UL)),
        )
        assertEquals(2, merged.size)
    }

    @Test
    fun visibleChatSuppressesTrayExceptCalls() {
        val open = "ab".repeat(32)
        val other = "cd".repeat(32)
        // List page: everything notifies.
        assertTrue(shouldNotifyRow(open, "", null, true))
        // Other chat while inside a thread: notifies.
        assertTrue(shouldNotifyRow(other, "", open, true))
        // Same chat foreground: silent, user already sees it.
        assertFalse(shouldNotifyRow(open, "", open, true))
        // Calls always ring, even in the open thread.
        assertTrue(shouldNotifyRow(open, "call_invite", open, true))
        assertTrue(shouldNotifyRow(open, "call_ringing", open, true))
    }

    @Test
    fun backgroundAlwaysNotifies() {
        val open = "ab".repeat(32)
        // Backgrounded with a thread still selected: tray must fire.
        assertTrue(shouldNotifyRow(open, "", open, false))
        assertTrue(shouldNotifyRow(open, "", null, false))
    }

    @Test
    fun stateDiffCoversMissedEvents() {
        // Service diffs the replayed full snapshot, so a late subscriber that
        // missed freshRows still notifies exactly once via the seen mark.
        val conv = "ab".repeat(32)
        val full = listOf(row(convId = conv, convSeq = 1UL), row(convId = conv, convSeq = 2UL))
        val (firstPending, seen) = pendingNotifies(full, emptyMap())
        assertEquals(2, firstPending.size)
        val (secondPending, _) = pendingNotifies(full, seen)
        assertTrue(secondPending.isEmpty())
    }

    private fun row(
        convId: String = "ab".repeat(32),
        convSeq: ULong = 1UL,
        kind: String = "",
        text: String = "hello",
        target: ULong = 0UL,
    ) = DisplayRow(
        convId = convId,
        convSeq = convSeq,
        text = text,
        sentAt = 1UL,
        fileName = "",
        fileMime = "",
        fileBytes = byteArrayOf(),
        fetchToken = "",
        kind = kind,
        emoji = "",
        target = target,
        hidden = false,
        displayedAt = 1UL,
        outgoing = false,

        senderId = "",
        senderName = "",
    )
}
