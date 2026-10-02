package org.nemo

import uniffi.nemo.DisplayRow
import java.time.LocalDate
import java.time.ZoneId
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class ChatDateTest {
    @Test
    fun pillShowsOnlyForFingerScrolling() {
        // Programmatic animations (pin-to-bottom on send/receive,
        // jump-to-latest) raise isScrollInProgress too but must never summon
        // the pill; flings after a drag keep it (still user-driven).
        assertTrue(pillArmedForScroll(isScrollInProgress = true, autoScrolling = false))
        assertFalse(pillArmedForScroll(isScrollInProgress = true, autoScrolling = true))
        assertFalse(pillArmedForScroll(isScrollInProgress = false, autoScrolling = false))
        assertFalse(pillArmedForScroll(isScrollInProgress = false, autoScrolling = true))
    }

    @Test
    fun unknownTimestampHasNoDay() {
        assertNull(chatDayKey(0UL))
        assertEquals("", formatChatDate(0UL))
        assertFalse(needsDateHeader(null, 0UL))
        assertFalse(needsDateHeader(1UL, 0UL))
    }

    @Test
    fun todayAndYesterdayLabels() {
        val now = System.currentTimeMillis()
        val zone = ZoneId.systemDefault()
        fun noon(date: LocalDate): ULong {
            val millis = date.atTime(12, 0).atZone(zone).toInstant().toEpochMilli()
            return (millis / 1000L).toULong()
        }
        val today = LocalDate.now(zone)
        assertEquals("Today", formatChatDate(noon(today), now))
        assertEquals("Yesterday", formatChatDate(noon(today.minusDays(1)), now))
    }

    @Test
    fun absoluteDatesOmitYearWhenCurrent() {
        val zone = ZoneId.systemDefault()
        val today = LocalDate.now(zone)
        val other = today.minusDays(10)
        val sentAt = (other.atTime(12, 0).atZone(zone).toInstant().toEpochMilli() / 1000L).toULong()
        val label = formatChatDate(sentAt, System.currentTimeMillis())
        if (other.year == today.year) {
            assertFalse(label.contains(other.year.toString()))
            assertTrue(label.contains(other.dayOfMonth.toString()))
        } else {
            assertTrue(label.contains(other.year.toString()))
        }
        val old = (LocalDate.of(2020, 1, 15).atTime(12, 0).atZone(zone).toInstant().toEpochMilli() / 1000L).toULong()
        val oldLabel = formatChatDate(old, System.currentTimeMillis())
        if (today.year != 2020) {
            assertTrue(oldLabel.contains("2020"))
        }
    }

    @Test
    fun headersSplitAtMidnight() {
        val zone = ZoneId.systemDefault()
        val today = LocalDate.now(zone)
        fun secs(date: LocalDate, hour: Int): ULong = (date.atTime(hour, 0).atZone(zone).toInstant().toEpochMilli() / 1000L).toULong()
        val late = secs(today.minusDays(1), 23)
        val early = secs(today, 0)
        // 23:00 yesterday vs 00:00 today are different days even though close.
        if (chatDayKey(late) != chatDayKey(early)) {
            assertTrue(needsDateHeader(late, early))
            assertFalse(isSameChatDay(late, early))
        }
        assertTrue(needsDateHeader(null, early))
        assertFalse(needsDateHeader(early, secs(today, 12)))
    }

    @Test
    fun headerFlagsGroupSameDay() {
        val zone = ZoneId.systemDefault()
        val today = LocalDate.now(zone)
        fun secs(date: LocalDate, hour: Int): ULong = (date.atTime(hour, 0).atZone(zone).toInstant().toEpochMilli() / 1000L).toULong()
        val rows = listOf(
            row(sentAt = secs(today.minusDays(1), 10)),
            row(sentAt = secs(today.minusDays(1), 12)),
            row(sentAt = secs(today, 9)),
        )
        val flags = dateHeadersFor(rows)
        assertTrue(flags[0])
        assertFalse(flags[1])
        assertTrue(flags[2])
    }

    @Test
    fun singleDateStillHasOneHeader() {
        val rows = listOf(row(sentAt = 1_700_000_000UL), row(sentAt = 1_700_000_100UL))
        // Both may fall on the same day; exactly one header in that case.
        val flags = dateHeadersFor(rows)
        assertTrue(flags[0])
        if (isSameChatDay(rows[0].sentAt, rows[1].sentAt)) {
            assertFalse(flags[1])
        } else {
            assertTrue(flags[1])
        }
    }

    @Test
    fun activeDateIsTopmostVisible() {
        val rows = (1UL..5UL).map { row(convSeq = it, sentAt = 1_700_000_000UL + it * 1_000UL) }
        // Newest-first display 0..4, no marker: topmost (4) is oldest (seq 1).
        assertEquals(rows[0].sentAt, activeDateForVisible(rows, null, listOf(0, 1, 2, 3, 4)))
        assertEquals(rows[3].sentAt, activeDateForVisible(rows, null, listOf(0, 1)))
        assertNull(activeDateForVisible(rows, null, emptyList()))
        assertNull(activeDateForVisible(emptyList(), null, listOf(0)))
    }

    @Test
    fun activeDateSkipsUnreadMarker() {
        val rows = (1UL..5UL).map { row(convSeq = it, sentAt = 1_700_000_000UL + it) }
        // Marker at display 3: visible 2,3,4 → topmost message is display 4 (seq 2).
        assertEquals(rows[1].sentAt, activeDateForVisible(rows, 3, listOf(2, 3, 4)))
        // Only marker visible → null.
        assertNull(activeDateForVisible(rows, 3, listOf(3)))
        // Undated top row → null.
        val undated = listOf(row(convSeq = 1UL, sentAt = 0UL))
        assertNull(activeDateForVisible(undated, null, listOf(0)))
    }

    @Test
    fun pushUpWhenHeaderApproachesFromBelow() {
        // Floating 100..140, other-day header overlapping from below.
        val push = floatingPushFor(
            floatingTop = 100f,
            floatingBottom = 140f,
            activeDayKey = 1L,
            headers = listOf(Triple(130f, 160f, 2L)),
        )
        assertEquals(-10f, push)
    }

    @Test
    fun pushDownWhenHeaderEntersFromAbove() {
        val push = floatingPushFor(
            floatingTop = 100f,
            floatingBottom = 140f,
            activeDayKey = 2L,
            headers = listOf(Triple(80f, 110f, 1L)),
        )
        assertEquals(10f, push)
    }

    @Test
    fun sameDayHeaderNeverPushes() {
        val push = floatingPushFor(
            floatingTop = 100f,
            floatingBottom = 140f,
            activeDayKey = 1L,
            headers = listOf(Triple(130f, 160f, 1L)),
        )
        assertEquals(0f, push)
    }

    @Test
    fun noOverlapNoPush() {
        val push = floatingPushFor(
            floatingTop = 100f,
            floatingBottom = 140f,
            activeDayKey = 1L,
            headers = listOf(Triple(200f, 230f, 2L)),
        )
        assertEquals(0f, push)
        assertEquals(
            0f,
            floatingPushFor(Float.NaN, 140f, 1L, listOf(Triple(130f, 160f, 2L))),
        )
    }

    private fun row(convSeq: ULong = 1UL, sentAt: ULong = 1UL) = DisplayRow(
        convId = "ab".repeat(32),
        convSeq = convSeq,
        text = "hello",
        sentAt = sentAt,
        fileName = "",
        fileMime = "",
        fileBytes = byteArrayOf(),
        fetchToken = "",
        kind = "",
        emoji = "",
        target = 0UL,
        hidden = false,
        displayedAt = 1UL,
        outgoing = false,

        senderId = "",
        senderName = "",
    )
}
