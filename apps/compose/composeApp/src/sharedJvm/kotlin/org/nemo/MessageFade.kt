package org.nemo

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp

/** Height of the top fade over the message list — gradual, keeps reading area clear. */
internal val MessageListFadeHeight = 48.dp

/** Gradient stops for the top fade: opaque theme background → fully transparent. */
internal fun messageListFadeColors(baseColor: Color): List<Color> = listOf(baseColor, baseColor.copy(alpha = 0f))

/**
 * Top gradient fade mask for the message list.
 *
 * Sits above scrolling messages but below the status bar / fixed header (callers
 * place it inside the Scaffold content, after the LazyColumn so it draws on top).
 * No clickable / pointerInput modifiers on purpose: hit-testing passes through
 * to the list below (Compose equivalent of pointer-events: none / IgnorePointer).
 */
@Composable
internal fun MessageListTopFade(baseColor: Color, modifier: Modifier = Modifier) {
    Box(
        modifier
            .fillMaxWidth()
            .height(MessageListFadeHeight)
            .background(Brush.verticalGradient(messageListFadeColors(baseColor)))
            .testTag("messageFade"),
    )
}
