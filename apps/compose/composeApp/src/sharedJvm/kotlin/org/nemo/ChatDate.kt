package org.nemo

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.snapshots.SnapshotStateMap
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInWindow
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.distinctUntilChanged
import uniffi.nemo.DisplayRow
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

/** Hide delay after scrolling stops before the floating date fades out. */
internal const val FLOATING_DATE_HIDE_DELAY_MS = 700L

/** Fade/slide duration for floating date appear/disappear and date switches. */
internal const val FLOATING_DATE_ANIM_MS = 180

/** Window position of an in-list date header (window Y in px). */
internal data class ChatHeaderPos(val top: Float, val height: Float) {
    val bottom: Float get() = top + height
}

/**
 * Day key for [sentAt] (unix seconds, UTC) in the device timezone.
 * Null when unknown (sentAt == 0).
 */
internal fun chatDayKey(sentAt: ULong): Long? {
    if (sentAt == 0UL) return null
    return try {
        Instant.ofEpochSecond(sentAt.toLong())
            .atZone(ZoneId.systemDefault())
            .toLocalDate()
            .toEpochDay()
    } catch (_: Throwable) {
        null
    }
}

internal fun chatDayKeyForMillis(nowMillis: Long): Long = Instant.ofEpochMilli(nowMillis)
    .atZone(ZoneId.systemDefault())
    .toLocalDate()
    .toEpochDay()

/**
 * Chat day label: Today / Yesterday / "September 28" / "September 28, 2023".
 * Reuses the device locale like [formatTime]; timestamps are stored in UTC
 * and rendered in the system timezone.
 */
internal fun formatChatDate(sentAt: ULong, nowMillis: Long = System.currentTimeMillis()): String {
    if (sentAt == 0UL) return ""
    return try {
        val zone = ZoneId.systemDefault()
        val date = Instant.ofEpochSecond(sentAt.toLong()).atZone(zone).toLocalDate()
        val today = Instant.ofEpochMilli(nowMillis).atZone(zone).toLocalDate()
        when {
            date == today -> "Today"
            date == today.minusDays(1) -> "Yesterday"
            date.year == today.year ->
                DateTimeFormatter.ofPattern("MMMM d", Locale.getDefault()).format(date)
            else ->
                DateTimeFormatter.ofPattern("MMMM d, yyyy", Locale.getDefault()).format(date)
        }
    } catch (_: Throwable) {
        ""
    }
}

internal fun isSameChatDay(a: ULong, b: ULong): Boolean {
    if (a == 0UL || b == 0UL) return a == b
    return chatDayKey(a) == chatDayKey(b)
}

/**
 * True when [currSentAt] starts a new day section. Unknown timestamps never
 * start a section (no pill for undated rows).
 */
internal fun needsDateHeader(prevSentAt: ULong?, currSentAt: ULong): Boolean {
    if (currSentAt == 0UL) return false
    if (prevSentAt == null) return true
    return !isSameChatDay(prevSentAt, currSentAt)
}

/** Per-chrono header flags for a chronological message list. O(n), call once per list change. */
internal fun dateHeadersFor(messages: List<DisplayRow>): BooleanArray = BooleanArray(messages.size) { i ->
    needsDateHeader(messages.getOrNull(i - 1)?.sentAt, messages[i].sentAt)
}

/**
 * Pure active-date core: date of the message closest to the top of the
 * viewport (largest display index, since reverseLayout puts older rows higher).
 *
 * [messages] is chronological, [dividerAt] is the unread-marker display index
 * (message-only coordinates, see ChatThread), [visibleIndices] are Lazy display
 * indices. Returns the top message's sentAt, or null when empty/undated.
 * Only iterates visible items.
 */
internal fun activeDateForVisible(messages: List<DisplayRow>, dividerAt: Int?, visibleIndices: Collection<Int>): ULong? {
    var topDisplay: Int? = null
    for (d in visibleIndices) {
        if (dividerAt != null && d == dividerAt) continue
        if (topDisplay == null || d > topDisplay) topDisplay = d
    }
    val top = topDisplay ?: return null
    val newestIndex = if (dividerAt != null && top > dividerAt) top - 1 else top
    val chrono = messages.lastIndex - newestIndex
    val row = messages.getOrNull(chrono) ?: return null
    if (row.sentAt == 0UL) return null
    return row.sentAt
}

/**
 * Pure collision core: translation (px) that lets an overlapping in-list date
 * header push the floating pill away instead of text popping underneath it.
 *
 * Positive pushes down (header entering from above when scrolling to older),
 * negative pushes up (header approaching from below when scrolling to newer).
 * Headers for the active day never push. Returns 0 when nothing overlaps.
 */
internal fun floatingPushFor(
    floatingTop: Float,
    floatingBottom: Float,
    activeDayKey: Long?,
    headers: List<Triple<Float, Float, Long?>>,
): Float {
    if (floatingTop.isNaN() || floatingBottom.isNaN()) return 0f
    val floatingCenter = (floatingTop + floatingBottom) / 2f
    var best = 0f
    var bestAbs = Float.MAX_VALUE
    for ((top, bottom, dayKey) in headers) {
        if (activeDayKey != null && dayKey != null && dayKey == activeDayKey) continue
        if (top >= floatingBottom || bottom <= floatingTop) continue
        val center = (top + bottom) / 2f
        val candidate = if (center > floatingCenter) {
            top - floatingBottom
        } else {
            bottom - floatingTop
        }
        val abs = kotlin.math.abs(candidate)
        if (abs < bestAbs) {
            bestAbs = abs
            best = candidate
        }
    }
    return best
}

internal fun floatingPushForPos(floatingTop: Float, floatingHeight: Float, activeDayKey: Long?, headers: Map<Long, ChatHeaderPos>): Float {
    if (floatingTop.isNaN() || floatingHeight.isNaN()) return 0f
    val triples = headers.map { (day, pos) -> Triple(pos.top, pos.bottom, day as Long?) }
    return floatingPushFor(floatingTop, floatingTop + floatingHeight, activeDayKey, triples)
}

/**
 * Dedicated state for the floating date.
 *
 * Keeps ChatThread small: header flags are computed once per list change,
 * the active day only updates when the top visible day changes, and the
 * scroll callback only touches activeDay / scrolling / push offset.
 */
internal class FloatingDateUiState(
    val dateNeedsHeader: BooleanArray,
    val dateDayKeys: List<Long?>,
    val dayLabels: Map<Long, String>,
    internal val activeDay: MutableState<Long?>,
    internal val scrollingVisible: State<Boolean>,
    val headerPositions: SnapshotStateMap<Long, ChatHeaderPos>,
    internal val floatingTop: MutableState<Float>,
    internal val floatingHeight: MutableState<Float>,
) {
    val activeDayKey: Long? get() = activeDay.value
    val floatingVisibleRaw: Boolean get() = scrollingVisible.value
    val activeLabel: String? get() = activeDay.value?.let { dayLabels[it] }
    val floatingPush: Float
        get() {
            if (!scrollingVisible.value || activeDay.value == null) return 0f
            return floatingPushForPos(floatingTop.value, floatingHeight.value, activeDay.value, headerPositions)
        }

    fun onHeaderPositioned(dayKey: Long, top: Float, height: Float) {
        headerPositions[dayKey] = ChatHeaderPos(top, height)
    }

    fun onHeaderDisposed(dayKey: Long) {
        headerPositions.remove(dayKey)
    }

    fun onFloatingPositioned(top: Float, height: Float) {
        floatingTop.value = top
        floatingHeight.value = height
    }
}

@Composable
internal fun rememberFloatingDateUiState(
    messages: List<DisplayRow>,
    chatId: String,
    listState: LazyListState,
    dividerAt: Int?,
): FloatingDateUiState {
    val dateNeedsHeader = remember(messages) { dateHeadersFor(messages) }
    val dateDayKeys = remember(messages) { messages.map { chatDayKey(it.sentAt) } }
    val dayLabels = remember(messages) {
        messages.mapNotNull { row ->
            val dk = chatDayKey(row.sentAt) ?: return@mapNotNull null
            dk to formatChatDate(row.sentAt)
        }.toMap()
    }
    val activeDay = remember(chatId) { mutableStateOf<Long?>(null) }
    val scrollingVisible = remember(chatId) { mutableStateOf(false) }
    val headerPositions = remember(chatId) { mutableStateMapOf<Long, ChatHeaderPos>() }
    val floatingTop = remember(chatId) { mutableFloatStateOf(Float.NaN) }
    val floatingHeight = remember(chatId) { mutableFloatStateOf(Float.NaN) }

    LaunchedEffect(chatId, listState.isScrollInProgress) {
        if (listState.isScrollInProgress) {
            scrollingVisible.value = true
        } else {
            delay(FLOATING_DATE_HIDE_DELAY_MS)
            scrollingVisible.value = false
        }
    }
    LaunchedEffect(listState, dividerAt, messages.size, chatId) {
        snapshotFlow {
            val visible = listState.layoutInfo.visibleItemsInfo
            if (visible.isEmpty()) return@snapshotFlow null
            var topDisplay: Int? = null
            for (v in visible) {
                val d = v.index
                if (dividerAt != null && d == dividerAt) continue
                if (topDisplay == null || d > topDisplay) topDisplay = d
            }
            val top = topDisplay ?: return@snapshotFlow null
            val newestIndex = if (dividerAt != null && top > dividerAt) top - 1 else top
            val chrono = messages.lastIndex - newestIndex
            val row = messages.getOrNull(chrono) ?: return@snapshotFlow null
            if (row.sentAt == 0UL) return@snapshotFlow null
            chatDayKey(row.sentAt)
        }.distinctUntilChanged().collect { activeDay.value = it }
    }
    return remember(messages, chatId, dividerAt, listState) {
        FloatingDateUiState(
            dateNeedsHeader = dateNeedsHeader,
            dateDayKeys = dateDayKeys,
            dayLabels = dayLabels,
            activeDay = activeDay,
            scrollingVisible = scrollingVisible,
            headerPositions = headerPositions,
            floatingTop = floatingTop,
            floatingHeight = floatingHeight,
        )
    }
}

/** Shared pill colors: reuse the app surface like the chat header pill. */
@Composable
private fun datePillColors(dark: Boolean) = MaterialTheme.colorScheme.surface.copy(
    alpha = if (dark) 0.82f else 0.92f,
)

/**
 * In-list day separator: centered pill over the wallpaper.
 * Reports its window position so the floating pill can be pushed by it.
 */
@Composable
internal fun ChatDateSeparator(
    label: String,
    dayKey: Long?,
    modifier: Modifier = Modifier,
    onPositioned: ((top: Float, height: Float) -> Unit)? = null,
    onDisposed: (() -> Unit)? = null,
) {
    val dark = nemoDarkTheme()
    DisposableEffect(dayKey) {
        onDispose { onDisposed?.invoke() }
    }
    Box(
        modifier
            .fillMaxWidth()
            .padding(top = 8.dp, bottom = 4.dp),
        contentAlignment = Alignment.Center,
    ) {
        Surface(
            shape = RoundedCornerShape(16.dp),
            color = datePillColors(dark),
            shadowElevation = 2.dp,
            tonalElevation = 0.dp,
            modifier = Modifier
                .testTag("date-separator-$label")
                .then(
                    if (onPositioned != null) {
                        Modifier.onGloballyPositioned { coords ->
                            onPositioned(
                                coords.positionInWindow().y,
                                coords.size.height.toFloat(),
                            )
                        }
                    } else {
                        Modifier
                    },
                ),
        ) {
            Text(
                label,
                style = MaterialTheme.typography.labelLarge,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                modifier = Modifier.padding(horizontal = 14.dp, vertical = 6.dp),
            )
        }
    }
}

/**
 * Floating date overlay.
 *
 * Overlay only (callers place it in a Box above the LazyColumn, no layout space
 * reserved). Fade + small vertical slide, no scale/bounce. Date switches via
 * AnimatedContent so the pill itself is stable.
 */
@Composable
internal fun FloatingChatDate(
    dateText: String?,
    visible: Boolean,
    offsetYPx: Float,
    modifier: Modifier = Modifier,
    onPositioned: ((top: Float, height: Float) -> Unit)? = null,
) {
    val dark = nemoDarkTheme()
    androidx.compose.animation.AnimatedVisibility(
        visible = visible && dateText != null,
        enter = fadeIn(tween(FLOATING_DATE_ANIM_MS)) +
            slideInVertically(tween(FLOATING_DATE_ANIM_MS)) { -it / 4 },
        exit = fadeOut(tween(FLOATING_DATE_ANIM_MS)) +
            slideOutVertically(tween(FLOATING_DATE_ANIM_MS)) { -it / 4 },
        modifier = modifier.graphicsLayer {
            translationY = if (offsetYPx.isNaN()) 0f else offsetYPx
        },
    ) {
        Surface(
            shape = RoundedCornerShape(16.dp),
            color = datePillColors(dark),
            shadowElevation = 2.dp,
            tonalElevation = 0.dp,
            modifier = Modifier
                .testTag("floating-date")
                .then(
                    if (onPositioned != null) {
                        Modifier.onGloballyPositioned { coords ->
                            onPositioned(
                                coords.positionInWindow().y,
                                coords.size.height.toFloat(),
                            )
                        }
                    } else {
                        Modifier
                    },
                ),
        ) {
            AnimatedContent(
                targetState = dateText.orEmpty(),
                transitionSpec = {
                    (
                        fadeIn(tween(FLOATING_DATE_ANIM_MS)) +
                            slideInVertically(tween(FLOATING_DATE_ANIM_MS)) { it / 4 }
                        ) togetherWith
                        (
                            fadeOut(tween(FLOATING_DATE_ANIM_MS)) +
                                slideOutVertically(tween(FLOATING_DATE_ANIM_MS)) { -it / 4 }
                            )
                },
                label = "floatingDateText",
            ) { text ->
                Text(
                    text,
                    style = MaterialTheme.typography.labelLarge,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    modifier = Modifier.padding(horizontal = 14.dp, vertical = 6.dp),
                )
            }
        }
    }
}

/**
 * One message bubble bundled with its day header (if it starts a new day).
 * Headers stay in the same Lazy item so unread-marker indices never shift.
 */
@Composable
internal fun DatedMessageItem(
    row: DisplayRow,
    chronoIndex: Int,
    messages: List<DisplayRow>,
    outgoing: Map<String, Boolean>,
    outgoingStatus: Map<String, OutgoingStatus>,
    floatingDate: FloatingDateUiState,
    seedDone: Boolean,
    knownKeys: MutableSet<String>,
    useFlyMorph: Boolean,
    flyingKeys: Set<String>,
    overlayRoot: LayoutCoordinates?,
    flyTargets: MutableMap<String, Rect>,
    onReact: (DisplayRow) -> Unit,
    onDelete: (DisplayRow) -> Unit,
    onSave: (DisplayRow) -> Unit,
) {
    val key = outgoingMapKey(row)
    val mine = row.outgoing || outgoing[key] == true
    val prevMine = messages.getOrNull(chronoIndex - 1)?.let { prev ->
        prev.outgoing || outgoing[outgoingMapKey(prev)] == true
    }
    val nextMine = messages.getOrNull(chronoIndex + 1)?.let { next ->
        next.outgoing || outgoing[outgoingMapKey(next)] == true
    }
    val showDateHeader = floatingDate.dateNeedsHeader.getOrNull(chronoIndex) == true
    val dayKey = floatingDate.dateDayKeys.getOrNull(chronoIndex)
    val dayLabel = dayKey?.let { floatingDate.dayLabels[it] }
    val nextStartsNewDay = messages.getOrNull(chronoIndex + 1)?.let { next ->
        needsDateHeader(row.sentAt, next.sentAt)
    } == true
    val clusteredAbove = prevMine == mine && !showDateHeader
    val clusteredBelow = nextMine == mine && !nextStartsNewDay
    val gap = if (showDateHeader) {
        2.dp
    } else if (clusteredAbove) {
        2.dp
    } else {
        8.dp
    }
    val listKey = messageListKey(row)
    val isFlying = useFlyMorph && listKey in flyingKeys
    val animateEnter = remember(listKey) {
        val neu = seedDone && listKey !in knownKeys
        if (neu) knownKeys.add(listKey)
        neu
    }
    Column(Modifier.fillMaxWidth()) {
        if (showDateHeader && dayKey != null && dayLabel != null) {
            ChatDateSeparator(
                label = dayLabel,
                dayKey = dayKey,
                onPositioned = { top, h -> floatingDate.onHeaderPositioned(dayKey, top, h) },
                onDisposed = { floatingDate.onHeaderDisposed(dayKey) },
            )
        }
        MessageBubble(
            row = row,
            mine = mine,
            status = if (mine) outgoingStatus[key] else null,
            clusteredAbove = clusteredAbove,
            clusteredBelow = clusteredBelow,
            animateEnter = animateEnter && !isFlying,
            conceal = isFlying,
            onBubbleCoords = if (isFlying) {
                { coords ->
                    overlayRoot?.let { parent ->
                        flyTargets[listKey] = boundsInParent(parent, coords)
                    }
                }
            } else {
                null
            },
            onReact = { onReact(row) },
            onDelete = { onDelete(row) },
            onSave = { onSave(row) },
            // No animateItem: with reverseLayout, a new message shifts every
            // visible index and placement animation makes the thread shake.
            modifier = Modifier.padding(top = gap),
        )
    }
}
