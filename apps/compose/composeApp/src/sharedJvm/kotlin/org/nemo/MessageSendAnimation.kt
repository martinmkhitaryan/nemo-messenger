package org.nemo

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.AccessTime
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlin.math.roundToInt

/** Quint-style ease-out for the flight (fast start, long settle). */
private val FlightEasing = CubicBezierEasing(0.22f, 1f, 0.36f, 1f)

/** Quadratic ease-out for the delayed fade-in. */
private val BoxEasing = Easing { t -> 1f - (1f - t) * (1f - t) }

/**
 * Message send animation.
 *
 * Platform split — not the same effect on both platforms:
 * - Mobile: composer → bubble morph overlay ([MessageSendFlyOverlay]).
 * - Desktop: no morph. New bubbles appear in place (fade + scale + rise,
 *   250 ms cubic-bezier(.4,0,.2,1)). There is no source→target geometry on desktop.
 *
 * Desktop bubbles appear via MessageBubble's enter animation in Session.kt.
 */
internal data class MessageSendAnimation(
    val listKey: String,
    val text: String,
    val timeLabel: String,
    /** Composer field bounds in overlay-local coordinates. */
    val source: Rect,
    val clusteredAbove: Boolean = false,
    val clusteredBelow: Boolean = false,
)

internal data class PendingMessageSend(val text: String, val source: Rect)

/** Screen-space outgoing ribbon shared with [MessageBubble]. */
internal fun outgoingScreenBrush(stops: List<Color>, windowTopY: Float, rootHeightPx: Float): Brush {
    val h = rootHeightPx.coerceAtLeast(1f)
    return Brush.verticalGradient(
        colors = stops,
        startY = -windowTopY,
        endY = -windowTopY + h,
    )
}

/** Max bubble content width: 320.dp bubble minus 12.dp horizontal padding on each side. */
private val BubbleContentMaxWidth: Dp = 296.dp

/** Inline trailing time vs. corner-floated time, decided per message. */
private data class BubbleLayout(val inline: Boolean, val timeReserve: Dp)

/**
 * Shared bubble body: message text plus trailing time.
 *
 * The time is always pinned to the bubble's bottom-right corner: single-line
 * messages keep it trailing the text in the same row (which is the corner
 * there); taller messages reserve the time's width at the text end and float
 * the time over that space, so it shares the last line instead of growing a
 * new row. [measureWidth] is the content width the decision is measured
 * against — the bubble passes its max content width, the send overlay passes
 * the target bubble width so its final frame matches. [status] draws the
 * trailing tick (or pending clock); [metaAlpha] fades the time row in during
 * the send flight.
 */
@Composable
internal fun BubbleContent(
    text: String,
    timeLabel: String,
    bodyColor: Color,
    metaColor: Color,
    measureWidth: Dp = BubbleContentMaxWidth,
    hasStatus: Boolean = false,
    metaAlpha: Float = 1f,
    modifier: Modifier = Modifier,
    status: @Composable () -> Unit = {},
) {
    val measurer = rememberTextMeasurer()
    val density = LocalDensity.current
    val bodyStyle = MaterialTheme.typography.bodyLarge
    val timeStyle = MaterialTheme.typography.labelSmall
    val layout = remember(text, timeLabel, hasStatus, measureWidth) {
        val contentPx = with(density) { measureWidth.roundToPx() }.coerceAtLeast(1)
        val message = measurer.measure(
            text = text,
            style = bodyStyle,
            constraints = Constraints(maxWidth = contentPx),
        )
        val timeWidthPx = measurer.measure(text = timeLabel, style = timeStyle).size.width +
            with(density) { 8.dp.roundToPx() } +
            if (hasStatus) with(density) { 17.dp.roundToPx() } else 0
        BubbleLayout(
            inline = message.lineCount == 1 && message.size.width + timeWidthPx <= contentPx,
            timeReserve = with(density) { timeWidthPx.toDp() },
        )
    }
    if (layout.inline) {
        Row(
            modifier = modifier,
            // Bottom-, not center-aligned: timestamps sit low,
            // starting around the text's half height.
            verticalAlignment = Alignment.Bottom,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                text = text,
                style = bodyStyle,
                color = bodyColor,
                modifier = Modifier.weight(1f, fill = false),
            )
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(3.dp),
                modifier = Modifier.graphicsLayer { alpha = metaAlpha },
            ) {
                Text(text = timeLabel, style = timeStyle, color = metaColor)
                status()
            }
        }
    } else {
        // corner float: the text reserves the time's width at
        // its end, and the time sits over that space at the bottom-right, on
        // the last line instead of a new row. No overlap is possible because
        // no text line extends into the reserved zone.
        Box(modifier = modifier) {
            // No maxLines/ellipsis: the overlay final frame must match the bubble exactly,
            // otherwise long messages pop on handoff. Overflow is clipped by the morphing
            // shape while the overlay rect is still small.
            Text(
                text = text,
                style = bodyStyle,
                color = bodyColor,
                maxLines = Int.MAX_VALUE,
                overflow = TextOverflow.Clip,
                modifier = Modifier.padding(end = layout.timeReserve),
            )
            Row(
                modifier = Modifier.align(Alignment.BottomEnd).graphicsLayer {
                    alpha = metaAlpha
                },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(3.dp),
            ) {
                Text(text = timeLabel, style = timeStyle, color = metaColor)
                status()
            }
        }
    }
}

/**
 * Mobile-only overlay that morphs from the composer Surface into the list bubble.
 *
 * [liveTarget] must be the current screen-space bubble rect (updated while scrolling).
 * The list row stays concealed until [onFinished].
 *
 * Separate tracks: a single 460 ms eased flight carries the block diagonally from the
 * composer center to the bubble center. The box reveals right-anchored from narrow to full
 * width early in the flight with full-size content from the first frame — nothing reflows
 * or squashes, it only grows while traveling. Transparency ramps fast alongside the reveal
 * before settling on the target. No exaggerated bounce.
 * Desktop never uses this — see file header.
 */
@Composable
internal fun MessageSendFlyOverlay(
    animation: MessageSendAnimation,
    liveTarget: Rect?,
    overlayWindowOrigin: Offset,
    rootHeightPx: Float,
    dark: Boolean,
    onFinished: () -> Unit,
) {
    val density = LocalDensity.current
    val flight = remember(animation.listKey) { Animatable(0f) }
    var finished by remember(animation.listKey) { mutableStateOf(false) }
    val flightMs = messageSendFlyDurationMs()

    LaunchedEffect(animation.listKey, liveTarget == null) {
        if (liveTarget == null) return@LaunchedEffect
        // Hold at source until the list lays out the optimistic row, then fly.
        if (!finished && flight.value < 1f) {
            flight.animateTo(1f, tween(flightMs, easing = FlightEasing))
        }
        if (!finished) {
            finished = true
            onFinished()
        }
    }
    LaunchedEffect(animation.listKey) {
        delay(flightMs + 400L)
        if (!finished) {
            finished = true
            onFinished()
        }
    }

    // One diagonal: source center → target center on a single eased clock. The box reveals
    // right-anchored from narrow to full width early in the flight, with full-size content
    // from the first frame — nothing reflows or squashes, it only grows while traveling.
    val to = liveTarget ?: animation.source
    val t = if (liveTarget == null) 0f else flight.value
    val revealT = BoxEasing.transform((t / 0.5f).coerceIn(0f, 1f))
    val startWidth = (to.width * 0.35f).coerceAtLeast(48f)
    val width = androidx.compose.ui.util.lerp(startWidth, to.width, revealT)
    val right = androidx.compose.ui.util.lerp(animation.source.right, to.right, t)
    val centerY = androidx.compose.ui.util.lerp(animation.source.center.y, to.center.y, t)
    val rect = Rect(
        left = right - width,
        top = centerY - to.height / 2f,
        right = right,
        bottom = centerY + to.height / 2f,
    )

    // Fast early fade with the reveal; time fades on the same track.
    val boxAlpha = revealT

    // Mono overlay morphs into the inverse bubble; blue branches unchanged.
    val palette = LocalNemoPalette.current
    val bodyColor = when {
        palette.mono && dark -> Color(0xFF000000)
        palette.mono -> Color(0xFFFFFFFF)
        dark -> Color(0xFFE8F4FF)
        else -> MaterialTheme.colorScheme.onSurface
    }
    val metaColor = when {
        palette.mono && dark -> Color(0xFF000000).copy(alpha = 0.65f)
        palette.mono -> Color(0xFFFFFFFF).copy(alpha = 0.75f)
        dark -> Color(0xFFB8D4E8).copy(alpha = 0.9f)
        else -> MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.85f)
    }

    val padH = 12.dp
    val padV = 6.dp
    val shadowElev = 2.dp

    val corner = 18.dp
    val tight = 6.dp
    // Outgoing tail stays clear of the content rect.
    val tail = 4.dp
    val endTopEnd = if (animation.clusteredAbove) tight else corner
    val endBottomEnd = if (animation.clusteredBelow) tight else tail
    val shape = RoundedCornerShape(
        topStart = corner,
        topEnd = endTopEnd,
        bottomStart = corner,
        bottomEnd = endBottomEnd,
    )
    val windowTopY = overlayWindowOrigin.y + rect.top
    val brush = outgoingScreenBrush(palette.outgoingGradient, windowTopY, rootHeightPx)
    val shadow = palette.bubbleShadow

    Box(
        Modifier
            .offset { IntOffset(rect.left.roundToInt(), rect.top.roundToInt()) }
            .width(with(density) { rect.width.toDp().coerceAtLeast(48.dp) })
            .height(with(density) { rect.height.toDp().coerceAtLeast(32.dp) })
            .graphicsLayer {
                alpha = boxAlpha
            }
            .shadow(shadowElev, shape, ambientColor = shadow, spotColor = shadow)
            .clip(shape),
    ) {
        Box(Modifier.fillMaxSize().background(brush))
        // Fixed full-width content pinned right: as the outer box grows leftward it reveals
        // the content, which never reflows. Measured against the target width so the
        // inline/stacked decision matches the final frame.
        val targetWidthDp = with(density) { to.width.toDp() }
        Box(
            Modifier
                .width(targetWidthDp)
                .align(Alignment.CenterEnd)
                .fillMaxHeight()
                .padding(horizontal = padH, vertical = padV),
        ) {
            BubbleContent(
                text = animation.text,
                timeLabel = animation.timeLabel,
                bodyColor = bodyColor,
                metaColor = metaColor,
                measureWidth = (targetWidthDp - 24.dp).coerceAtLeast(64.dp),
                hasStatus = true,
                metaAlpha = boxAlpha,
                status = {
                    // Pending clock: the bubble shows AccessTime while the send is in flight.
                    // Without it the final frame mismatches and the ticks visibly pop in.
                    Icon(
                        Icons.Filled.AccessTime,
                        contentDescription = null,
                        modifier = Modifier.size(14.dp),
                        tint = metaColor,
                    )
                },
            )
        }
    }
}
