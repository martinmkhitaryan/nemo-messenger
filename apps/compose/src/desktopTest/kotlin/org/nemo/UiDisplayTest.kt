package org.nemo

import uniffi.nemo.DisplayRow
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class UiDisplayTest {
    @Test
    fun createCopyForbidsRecoveryAndExport() {
        assertTrue(CANNOT_RECOVER.contains("cannot be recovered or exported"))
    }

    @Test
    fun previewAndBubbleCoverProtocolKinds() {
        assertEquals("Message expired", previewLine(row(kind = "expired", hidden = true)))
        assertEquals("Expired", bubbleText(row(kind = "expired", hidden = true)))
        assertEquals("Message deleted", previewLine(row(kind = "deleted", hidden = true)))
        assertEquals("Deleted", bubbleText(row(kind = "deleted")))
        assertEquals("Reacted 👍", previewLine(row(kind = "reaction", emoji = "👍")))
        assertEquals("Incoming call", previewLine(row(kind = "call_invite")))
        assertEquals("Ringing", previewLine(row(kind = "call_ringing")))
        assertEquals("Call answered", previewLine(row(kind = "call_answer")))
        assertEquals("Answered", bubbleText(row(kind = "call_answer")))
        assertEquals("Declined", previewLine(row(kind = "call_reject")))
        assertEquals("Cancelled", previewLine(row(kind = "call_cancel")))
        assertEquals("Call ended", previewLine(row(kind = "call_end")))
        assertEquals("Messages lost", previewLine(row(kind = "lost")))
        assertEquals("Identity revoked", previewLine(row(kind = "revoked")))
        assertEquals("Home-server binding conflict", previewLine(row(kind = "binding_conflict")))
        assertEquals("📎 note.txt", previewLine(row(fileName = "note.txt")))
        assertEquals("hello", previewLine(row(text = "hello")))
        assertEquals("Disappearing messages: 30s", bubbleText(row(kind = "disappear", text = "30")))
    }

    @Test
    fun saveMenuOnlyWhenFileBytesPresent() {
        assertTrue(canSaveAttachment(row(fileName = "note.txt", fileBytes = byteArrayOf(1))))
        assertTrue(!canSaveAttachment(row(fileName = "note.txt")))
        assertTrue(!canSaveAttachment(row(fileName = "note.txt", fileBytes = byteArrayOf(1), hidden = true)))
        assertTrue(!canSaveAttachment(row(text = "hello")))
    }

    @Test
    fun hideMarksMatchingMessageAndDoesNotDuplicate() {
        val messages = mutableListOf(row(kind = "", convSeq = 3UL, text = "hello"))
        applyIncoming(
            messages,
            listOf(row(kind = "deleted", convSeq = 9UL, target = 3UL, text = "")),
        )
        assertEquals(1, messages.count { it.convSeq == 3UL && it.kind != "reaction" })
        assertTrue(messages[0].hidden)
        assertEquals("deleted", messages[0].kind)
        assertEquals(2, messages.size)
        applyIncoming(messages, listOf(messages[0]))
        assertEquals(2, messages.size)
    }

    @Test
    fun shareCardBytesReadsHexPayload() {
        val raw = byteArrayOf(0x01, 0xAB.toByte())
        val hex = raw.joinToString("") { "%02x".format(it.toInt() and 0xFF) }
        assertTrue(shareCardBytes("nemo:1:$hex#token").contentEquals(raw))
        assertNull(shareCardBytes("https://example"))
        assertNull(shareCardBytes("nemo:1:zz"))
    }

    private fun row(
        kind: String = "",
        text: String = "hello",
        hidden: Boolean = false,
        fileName: String = "",
        fileBytes: ByteArray = byteArrayOf(),
        emoji: String = "",
        convSeq: ULong = 1UL,
        target: ULong = 0UL,
    ) = DisplayRow(
        convId = "ab".repeat(32),
        convSeq = convSeq,
        text = text,
        sentAt = 1UL,
        fileName = fileName,
        fileMime = "",
        fileBytes = fileBytes,
        fetchToken = "",
        kind = kind,
        emoji = emoji,
        target = target,
        hidden = hidden,
        displayedAt = 1UL,
        outgoing = false,
    )
}
