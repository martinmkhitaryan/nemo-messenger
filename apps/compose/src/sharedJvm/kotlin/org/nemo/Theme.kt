package org.nemo

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.VisibilityThreshold
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.sp

/** Telegram-like brand blue (not WhatsApp green). */
internal val NemoBlue = Color(0xFF3390EC)
internal val NemoBlueBright = Color(0xFF5CA8F5)
/** Soft sky outgoing bubble (flat fallback). */
internal val NemoOutgoingLight = Color(0xFFD5E8F7)
internal val NemoOutgoingDark = Color(0xFF2B5278)
/**
 * Screen-space outgoing bubble gradient (Telegram-style).
 * Colors are sampled by window Y so stacked bubbles share one continuous ribbon.
 * Brush.verticalGradient: first stop = top of window, last = bottom.
 */
/** Light: soft magenta (top) → soft blue (bottom). */
internal val NemoOutgoingGradientLight = listOf(
    Color(0xFFE8C8F0), // soft magenta — top of screen
    Color(0xFFB8D9F8), // soft blue — bottom of screen
)
/** Dark: magenta (top) → blue (bottom). */
internal val NemoOutgoingGradientDark = listOf(
    Color(0xFFB04FC8), // magenta — top of screen
    Color(0xFF2A6BB5), // blue — bottom of screen
)
internal val NemoIncomingLight = Color(0xFFFFFFFF)
internal val NemoIncomingDark = Color(0xFF182533)
internal val NemoChatLight = Color(0xFFD9E3EC)
internal val NemoChatDark = Color(0xFF0E1621)
internal val NemoListLight = Color(0xFFF0F4F8)
internal val NemoBubbleShadowLight = Color(0x1A000000)
internal val NemoBubbleShadowDark = Color(0x40000000)
internal val NemoPatternDotLight = Color(0x14000000)
internal val NemoPatternDotDark = Color(0x14FFFFFF)

internal enum class NemoThemeMode {
    System,
    Light,
    Dark,
    ;

    fun resolveDark(systemDark: Boolean): Boolean = when (this) {
        System -> systemDark
        Light -> false
        Dark -> true
    }

    val label: String
        get() = when (this) {
            System -> "System"
            Light -> "Light"
            Dark -> "Dark"
        }
}

internal val LocalThemeMode = staticCompositionLocalOf { NemoThemeMode.System }
internal val LocalOnThemeModeChange = staticCompositionLocalOf<(NemoThemeMode) -> Unit> { {} }

internal expect fun loadThemeMode(): NemoThemeMode

internal expect fun saveThemeMode(mode: NemoThemeMode)

/** Soft entrance for new bubbles — not snappy. */
internal val MessageFadeSpec = tween<Float>(durationMillis = 320)
internal val MessagePlacementSpec = spring(
    dampingRatio = Spring.DampingRatioNoBouncy,
    stiffness = Spring.StiffnessLow,
    visibilityThreshold = IntOffset.VisibilityThreshold,
)

private val NemoTypography = Typography(
    titleLarge = TextStyle(fontWeight = FontWeight.SemiBold, fontSize = 22.sp, lineHeight = 28.sp),
    titleMedium = TextStyle(fontWeight = FontWeight.SemiBold, fontSize = 16.sp, lineHeight = 22.sp),
    bodyLarge = TextStyle(fontWeight = FontWeight.Normal, fontSize = 16.sp, lineHeight = 22.sp),
    bodyMedium = TextStyle(fontWeight = FontWeight.Normal, fontSize = 14.sp, lineHeight = 20.sp),
    labelSmall = TextStyle(fontWeight = FontWeight.Medium, fontSize = 11.sp, lineHeight = 14.sp),
)

/** Cool blue-gray surfaces — avoid greenish Material defaults on menus/FABs. */
private val LightColors = lightColorScheme(
    primary = NemoBlue,
    onPrimary = Color.White,
    primaryContainer = Color(0xFFD6EBFF),
    onPrimaryContainer = Color(0xFF0B3A66),
    secondary = Color(0xFF4FA3E3),
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFD6EBFF),
    onSecondaryContainer = Color(0xFF0B3A66),
    tertiary = Color(0xFF5B8DEF),
    onTertiary = Color.White,
    tertiaryContainer = Color(0xFFE0E8FF),
    onTertiaryContainer = Color(0xFF1A2F5C),
    surface = Color.White,
    onSurface = Color(0xFF0F172A),
    surfaceVariant = Color(0xFFE8EEF4),
    onSurfaceVariant = Color(0xFF5B6770),
    background = NemoListLight,
    onBackground = Color(0xFF0F172A),
    outline = Color(0xFFC5D0DB),
    error = Color(0xFFB42318),
    surfaceContainerLowest = Color.White,
    surfaceContainerLow = Color(0xFFF5F8FB),
    surfaceContainer = Color(0xFFF0F4F8),
    surfaceContainerHigh = Color(0xFFE8EEF4),
    surfaceContainerHighest = Color(0xFFDEE6EF),
)

private val DarkColors = darkColorScheme(
    primary = NemoBlueBright,
    onPrimary = Color(0xFF062033),
    primaryContainer = Color(0xFF1E4F7A),
    onPrimaryContainer = Color(0xFFD6EBFF),
    secondary = Color(0xFF7EC8F8),
    onSecondary = Color(0xFF062033),
    secondaryContainer = Color(0xFF1E4F7A),
    onSecondaryContainer = Color(0xFFD6EBFF),
    tertiary = Color(0xFF9BB5F5),
    onTertiary = Color(0xFF0E1A33),
    tertiaryContainer = Color(0xFF2A3A5C),
    onTertiaryContainer = Color(0xFFD6E0FF),
    surface = Color(0xFF17212B),
    onSurface = Color(0xFFE8EEF2),
    surfaceVariant = Color(0xFF242F3D),
    onSurfaceVariant = Color(0xFFAEB8C2),
    background = NemoChatDark,
    onBackground = Color(0xFFE8EEF2),
    outline = Color(0xFF2F3C4A),
    error = Color(0xFFF97066),
    surfaceContainerLowest = Color(0xFF0B141A),
    surfaceContainerLow = Color(0xFF152028),
    surfaceContainer = Color(0xFF17212B),
    surfaceContainerHigh = Color(0xFF1F2C38),
    surfaceContainerHighest = Color(0xFF242F3D),
)

@Composable
internal fun nemoDarkTheme(): Boolean =
    LocalThemeMode.current.resolveDark(isSystemInDarkTheme())

@Composable
internal fun NemoTheme(
    mode: NemoThemeMode,
    onModeChange: (NemoThemeMode) -> Unit,
    content: @Composable () -> Unit,
) {
    val dark = mode.resolveDark(isSystemInDarkTheme())
    CompositionLocalProvider(
        LocalThemeMode provides mode,
        LocalOnThemeModeChange provides onModeChange,
    ) {
        MaterialTheme(
            colorScheme = if (dark) DarkColors else LightColors,
            typography = NemoTypography,
            content = content,
        )
    }
}
