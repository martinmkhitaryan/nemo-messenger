package org.nemo

import androidx.compose.material3.ColorScheme
import androidx.compose.ui.graphics.Color

/**
 * One self-contained theme: a Material3 scheme plus Nemo's custom chat colors.
 *
 * Families: blue ("Light"/"Dark", the original Nemo look) and monochrome
 * ("Mono Light"/"Mono Dark", black-and-white to match the N logo).
 * Blue palettes reference the original constants, so the original
 * themes render exactly as before.
 */
internal data class NemoPalette(
    val dark: Boolean,
    val mono: Boolean,
    val scheme: ColorScheme,
    /** Outgoing bubble gradient stops, sampled by [outgoingScreenBrush]. */
    val outgoingGradient: List<Color>,
    val incoming: Color,
    /** Chat thread wallpaper base. */
    val chat: Color,
    /** Chat list background. */
    val list: Color,
    /** Onboarding atmosphere gradient. */
    val onboardGradient: List<Color>,
    val patternDot: Color,
    val bubbleShadow: Color,
    val avatars: List<Color>,
    /** Own-bubble text: must contrast [outgoingGradient], not the surface. */
    val mineBody: Color,
    val mineMeta: Color,
)
