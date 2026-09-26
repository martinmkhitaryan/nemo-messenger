package org.nemo

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.EaseOutCubic
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
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
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlin.math.PI
import kotlin.math.roundToInt
import kotlin.math.sin

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
internal fun outgoingScreenBrush(dark: Boolean, windowTopY: Float, rootHeightPx: Float): Brush {
    val stops = if (dark) NemoOutgoingGradientDark else NemoOutgoingGradientLight
    val h = rootHeightPx.coerceAtLeast(1f)
    return Brush.verticalGradient(
        colors = stops,
        startY = -windowTopY,
        endY = -windowTopY + h,
    )
}

/**
 * Mobile-only overlay that morphs from the composer Surface into the list bubble.
 *
 * [liveTarget] must be the current screen-space bubble rect (updated while scrolling).
 * The list row stays concealed until [onFinished].
 *
 * Physical fly-out with a slight upward arc and a subtle 0.96 → 1.0 settle scale,
 * ~480 ms, EaseOutCubic. No exaggerated bounce.
 * Desktop/web never uses this — see file header.
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
    val progress = remember(animation.listKey) { Animatable(0f) }
    var finished by remember(animation.listKey) { mutableStateOf(false) }
    val durationMs = messageSendFlyDurationMs()

    LaunchedEffect(animation.listKey, liveTarget == null) {
        if (liveTarget == null) return@LaunchedEffect
        // Hold at source until the list lays out the optimistic row, then morph.
        if (progress.value == 0f) {
            progress.animateTo(
                targetValue = 1f,
                animationSpec = tween(durationMillis = durationMs, easing = EaseOutCubic),
            )
        }
        if (!finished) {
            finished = true
            onFinished()
        }
    }
    LaunchedEffect(animation.listKey) {
        delay(durationMs + 400L)
        if (!finished) {
            finished = true
            onFinished()
        }
    }

    // Live target: if the thread scrolls mid-flight, keep synchronizing to the real bubble.
    val to = liveTarget ?: animation.source
    val t = if (liveTarget == null) 0f else progress.value
    val baseRect = androidx.compose.ui.geometry.lerp(animation.source, to, t)
    // Arc lift mid-flight so the travel reads as a physical send, not a straight slide.
    val arcPx = with(density) { 20.dp.toPx() }
    val arcLift = sin(t * PI).toFloat() * arcPx
    val rect = Rect(
        left = baseRect.left,
        top = baseRect.top - arcLift,
        right = baseRect.right,
        bottom = baseRect.bottom - arcLift,
    )

    // Shape/color morph slightly ahead of position so it never reads as a finished bubble sliding.
    val morph = FastOutSlowInEasing.transform(t)
    val metaT = (morph * 1.35f - 0.35f).coerceIn(0f, 1f)
    // Android tracks currentScale separately from progress; iOS settles without bounce.
    val scale = androidx.compose.ui.util.lerp(0.96f, 1f, EaseOutCubic.transform(t))

    val composerBg = MaterialTheme.colorScheme.surfaceVariant.copy(
        alpha = if (dark) 0.55f else 0.85f,
    )
    val bodyFrom = MaterialTheme.colorScheme.onSurface
    val bodyTo = if (dark) Color(0xFFE8F4FF) else MaterialTheme.colorScheme.onSurface
    val bodyColor = androidx.compose.ui.graphics.lerp(bodyFrom, bodyTo, morph)
    val metaColor = if (dark) {
        Color(0xFFB8D4E8).copy(alpha = 0.9f * metaT)
    } else {
        MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.85f * metaT)
    }

    val padH = androidx.compose.ui.unit.lerp(16.dp, 12.dp, morph)
    val padV = androidx.compose.ui.unit.lerp(12.dp, 7.dp, morph)
    val shadowElev = androidx.compose.ui.unit.lerp(0.dp, 2.dp, morph)

    val corner = 18f
    val tight = 6f
    // Outgoing tail stays clear of the content rect, like iOS clipping inside [1, width-7].
    val tail = 4f
    val endTopEnd = if (animation.clusteredAbove) tight else corner
    val endBottomEnd = if (animation.clusteredBelow) tight else tail
    val shape = RoundedCornerShape(
        topStart = androidx.compose.ui.util.lerp(22f, corner, morph).dp,
        topEnd = androidx.compose.ui.util.lerp(22f, endTopEnd, morph).dp,
        bottomStart = androidx.compose.ui.util.lerp(22f, corner, morph).dp,
        bottomEnd = androidx.compose.ui.util.lerp(22f, endBottomEnd, morph).dp,
    )
    val windowTopY = overlayWindowOrigin.y + rect.top
    val brush = outgoingScreenBrush(dark, windowTopY, rootHeightPx)
    val shadow = if (dark) NemoBubbleShadowDark else NemoBubbleShadowLight
    val bodyStyle = MaterialTheme.typography.bodyLarge
    val fontSize = androidx.compose.ui.unit.lerp(16.sp, bodyStyle.fontSize, morph)
    val endLineHeight = bodyStyle.lineHeight.let { lh ->
        if (lh == TextUnit.Unspecified) 22.sp else lh
    }
    val lineHeight = androidx.compose.ui.unit.lerp(22.sp, endLineHeight, morph)

    Box(
        Modifier
            .offset { IntOffset(rect.left.roundToInt(), rect.top.roundToInt()) }
            .width(with(density) { rect.width.toDp().coerceAtLeast(48.dp) })
            .height(with(density) { rect.height.toDp().coerceAtLeast(36.dp) })
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
                transformOrigin = androidx.compose.ui.graphics.TransformOrigin(1f, 1f)
            }
            .shadow(shadowElev, shape, ambientColor = shadow, spotColor = shadow)
            .clip(shape),
    ) {
        Box(
            Modifier
                .fillMaxSize()
                .background(composerBg)
                .graphicsLayer { alpha = 1f - morph },
        )
        Box(
            Modifier
                .fillMaxSize()
                .background(brush)
                .graphicsLayer { alpha = morph },
        )
        Column(
            Modifier
                .fillMaxSize()
                .padding(horizontal = padH, vertical = padV),
        ) {
            // No maxLines/ellipsis: the final frame must match MessageBubble exactly,
            // otherwise long messages pop when the overlay hands off. Overflow is clipped
            // by the morphing shape while the rect is still small.
            Text(
                animation.text,
                style = bodyStyle.copy(fontSize = fontSize, lineHeight = lineHeight),
                color = bodyColor,
                maxLines = Int.MAX_VALUE,
                overflow = TextOverflow.Clip,
                modifier = Modifier.weight(1f, fill = false),
            )
            Row(
                Modifier
                    .align(Alignment.End)
                    .padding(top = 2.dp)
                    .graphicsLayer { alpha = metaT },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(3.dp),
            ) {
                Text(
                    animation.timeLabel,
                    style = MaterialTheme.typography.labelSmall,
                    color = metaColor,
                )
                // Pending clock: MessageBubble shows AccessTime while the send is in flight.
                // Without it the final frame mismatches and the ticks visibly pop in.
                Icon(
                    Icons.Filled.AccessTime,
                    contentDescription = null,
                    modifier = Modifier.size(14.dp),
                    tint = metaColor,
                )
            }
        }
    }
}
