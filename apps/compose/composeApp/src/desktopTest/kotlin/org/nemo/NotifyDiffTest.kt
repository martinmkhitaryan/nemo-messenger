package org.nemo

import uniffi.nemo.DisplayRow
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class NotifyDiffTest {
    @Test
    fun incomingTextNotifiesAndAdvancesSeen() {
        val (pending, next) = pendingNotifies(listOf(row(convSeq = 4UL)), emptyMap())
        assertEquals(listOf(PendingNotify(CONV, "")), pending)
        assertEquals(mapOf(CONV to 4UL), next)
    }

    @Test
    fun secondRunWithSameRowsStaysSilent() {
        val first = pendingNotifies(listOf(row(convSeq = 4UL)), emptyMap()).second
        val (pending, next) = pendingNotifies(listOf(row(convSeq = 4UL)), first)
        assertTrue(pending.isEmpty())
        assertEquals(first, next)
    }

    @Test
    fun ownAndHiddenRowsNeverNotifyAndNeverAdvanceSeen() {
        val rows = listOf(
            row(convSeq = 5UL, outgoing = true),
            row(convSeq = 6UL, hidden = true),
            row(convSeq = 7UL, fetchToken = "local:1"),
        )
        val (pending, next) = pendingNotifies(rows, emptyMap())
        assertTrue(pending.isEmpty())
        assertTrue(next.isEmpty())
    }

    @Test
    fun systemRowsAdvanceSeenButStaySilent() {
        val rows = listOf(
            row(convSeq = 2UL, kind = "reaction"),
            row(convSeq = 3UL, kind = "deleted"),
            row(convSeq = 4UL, kind = "call_end"),
            row(convSeq = 5UL, kind = "disappear"),
        )
        val (pending, next) = pendingNotifies(rows, emptyMap())
        assertTrue(pending.isEmpty())
        assertEquals(mapOf(CONV to 5UL), next)
    }

    @Test
    fun incomingCallNotifiesWithoutContent() {
        val (pending, _) = pendingNotifies(listOf(row(convSeq = 1UL, kind = "call_invite")), emptyMap())
        assertEquals(listOf(PendingNotify(CONV, "call_invite")), pending)
    }

    @Test
    fun staleSequencesAreIgnored() {
        val (pending, next) = pendingNotifies(listOf(row(convSeq = 2UL)), mapOf(CONV to 9UL))
        assertTrue(pending.isEmpty())
        assertEquals(mapOf(CONV to 9UL), next)
    }

    @Test
    fun conversationsTrackedIndependently() {
        val rows = listOf(
            row(convId = "aa", convSeq = 1UL),
            row(convId = "bb", convSeq = 7UL, kind = "call_ringing"),
        )
        val (pending, next) = pendingNotifies(rows, mapOf("aa" to 1UL))
        assertEquals(listOf(PendingNotify("bb", "call_ringing")), pending)
        assertEquals(mapOf("aa" to 1UL, "bb" to 7UL), next)
    }

    private fun row(
        convId: String = CONV,
        convSeq: ULong = 1UL,
        kind: String = "",
        outgoing: Boolean = false,
        hidden: Boolean = false,
        fetchToken: String = "",
    ) = DisplayRow(
        convId = convId,
        convSeq = convSeq,
        text = "hello",
        sentAt = 1UL,
        fileName = "",
        fileMime = "",
        fileBytes = byteArrayOf(),
        fetchToken = fetchToken,
        kind = kind,
        emoji = "",
        target = 0UL,
        hidden = hidden,
        displayedAt = 1UL,
        outgoing = outgoing,

        senderId = "",
        senderName = "",
    )

    private companion object {
        val CONV = "ab".repeat(32)
    }
}
