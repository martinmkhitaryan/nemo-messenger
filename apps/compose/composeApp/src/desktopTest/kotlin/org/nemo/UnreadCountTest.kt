package org.nemo

import uniffi.nemo.DisplayRow
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class UnreadCountTest {
    @Test
    fun textAndFilesCount() {
        val rows = listOf(
            row(convSeq = 1UL, kind = ""),
            row(convSeq = 2UL, kind = "", fileName = "note.txt"),
        )
        assertEquals(2, unreadCount(CONV, rows, emptyMap()))
    }

    @Test
    fun missedCallsCount() {
        val rows = listOf(
            row(convSeq = 1UL, kind = "call_invite"),
            row(convSeq = 2UL, kind = "call_ringing"),
        )
        assertEquals(2, unreadCount(CONV, rows, emptyMap()))
    }

    @Test
    fun systemRowsNeverCount() {
        val rows = listOf(
            row(convSeq = 1UL, kind = "reaction"),
            row(convSeq = 2UL, kind = "deleted"),
            row(convSeq = 3UL, kind = "expired"),
            row(convSeq = 4UL, kind = "disappear"),
            row(convSeq = 5UL, kind = "call_answer"),
            row(convSeq = 6UL, kind = "call_reject"),
            row(convSeq = 7UL, kind = "call_cancel"),
            row(convSeq = 8UL, kind = "call_end"),
            row(convSeq = 9UL, kind = "lost"),
            row(convSeq = 10UL, kind = "revoked"),
            row(convSeq = 11UL, kind = "binding_conflict"),
        )
        assertEquals(0, unreadCount(CONV, rows, emptyMap()))
        // ... but they still advance the mark when opening the chat.
        val lastRead = mutableMapOf<String, ULong>()
        assertTrue(markConversationRead(lastRead, rows, CONV))
        assertEquals(11UL, lastRead[CONV])
        assertEquals(0, unreadCount(CONV, rows, lastRead))
    }

    @Test
    fun ownHiddenAndLocalRowsNeverCount() {
        val rows = listOf(
            row(convSeq = 5UL, outgoing = true),
            row(convSeq = 6UL, hidden = true),
            row(convSeq = 7UL, fetchToken = "local:1"),
        )
        assertEquals(0, unreadCount(CONV, rows, emptyMap()))
    }

    @Test
    fun readMarkHidesOldRowsOnly() {
        val rows = listOf(
            row(convSeq = 1UL),
            row(convSeq = 2UL),
            row(convSeq = 3UL),
        )
        assertEquals(2, unreadCount(CONV, rows, mapOf(CONV to 1UL)))
        assertEquals(0, unreadCount(CONV, rows, mapOf(CONV to 3UL)))
    }

    @Test
    fun conversationsTrackedIndependently() {
        val rows = listOf(
            row(convId = "aa", convSeq = 1UL),
            row(convId = "bb", convSeq = 7UL),
        )
        assertEquals(0, unreadCount("aa", rows, mapOf("aa" to 1UL)))
        assertEquals(1, unreadCount("bb", rows, mapOf("aa" to 1UL)))
    }

    @Test
    fun markReadAdvancesPastSystems() {
        val rows = listOf(
            row(convSeq = 1UL, kind = ""),
            row(convSeq = 2UL, kind = "reaction"),
        )
        val lastRead = mutableMapOf<String, ULong>()
        assertTrue(markConversationRead(lastRead, rows, CONV))
        assertEquals(2UL, lastRead[CONV])
        assertFalse(markConversationRead(lastRead, rows, CONV))
    }

    @Test
    fun firstUnreadIsOldestCountable() {
        val rows = listOf(
            row(convSeq = 1UL, kind = ""),
            row(convSeq = 2UL, kind = "reaction"),
            row(convSeq = 3UL, kind = "call_ringing"),
            row(convSeq = 4UL, kind = ""),
        )
        assertEquals(1UL, firstUnreadSeq(rows, CONV, 0UL))
        assertEquals(3UL, firstUnreadSeq(rows, CONV, 1UL))
        assertEquals(4UL, firstUnreadSeq(rows, CONV, 3UL))
        assertEquals(null, firstUnreadSeq(rows, CONV, 4UL))
    }

    @Test
    fun cappedFirstUnreadIgnoresLaterRows() {
        val rows = listOf(
            row(convSeq = 1UL, kind = ""),
            row(convSeq = 2UL, kind = ""),
        )
        assertEquals(1UL, firstUnreadSeqCapped(rows, CONV, 0UL, 2UL))
        assertEquals(null, firstUnreadSeqCapped(rows, CONV, 0UL, 0UL))
        assertEquals(null, firstUnreadSeqCapped(rows, CONV, 2UL, 2UL))
    }

    @Test
    fun dividerSitsAfterAnchorBubble() {
        // Chronological 1..5, mark 2 → anchor seq 3 at chrono 2, newest-first
        // index 2, divider at 3.
        val rows = (1UL..5UL).map { row(convSeq = it) }
        assertEquals(3, unreadDividerAt(rows, CONV, 2UL, 5UL))
        assertEquals(null, unreadDividerAt(rows, CONV, 5UL, 5UL))
        assertEquals(null, unreadDividerAt(rows, CONV, 2UL, 2UL))
    }

    @Test
    fun markerTopTargetLeavesContextAbove() {
        // Divider 20 in a 10-row window → bottom edge 14, marker with 3
        // context rows above it.
        assertEquals(14, markerTopTarget(20, 10, 3))
        // Everything fits → clamp to bottom.
        assertEquals(0, markerTopTarget(4, 10, 3))
    }

    @Test
    fun belowViewportUnreadDrivesButton() {
        // Chronological 1..5, nothing read, viewport bottom at display 1:
        // rows at display 0 only (seq 5) sit below it.
        val rows = (1UL..5UL).map { row(convSeq = it) }
        assertEquals(1, belowUnreadCount(rows, CONV, 0UL, null, 1))
        assertEquals(0, belowUnreadCount(rows, CONV, 0UL, null, 0))
        assertEquals(0, belowUnreadCount(rows, CONV, 5UL, null, 1))
        // Marker slot shifts rows at/above it up by one.
        assertEquals(1, belowUnreadCount(rows, CONV, 0UL, 5, 1))
    }

    private fun row(
        convId: String = CONV,
        convSeq: ULong = 1UL,
        kind: String = "",
        outgoing: Boolean = false,
        hidden: Boolean = false,
        fetchToken: String = "",
        fileName: String = "",
    ) = DisplayRow(
        convId = convId,
        convSeq = convSeq,
        text = "hello",
        sentAt = 1UL,
        fileName = fileName,
        fileMime = "",
        fileBytes = byteArrayOf(),
        fetchToken = fetchToken,
        kind = kind,
        emoji = "",
        target = 0UL,
        hidden = hidden,
        displayedAt = 1UL,
        outgoing = outgoing,
    )

    private companion object {
        val CONV = "ab".repeat(32)
    }
}
