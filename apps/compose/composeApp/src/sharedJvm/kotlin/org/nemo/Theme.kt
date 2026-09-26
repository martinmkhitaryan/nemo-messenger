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

/** Brand blue. */
internal val NemoBlue = Color(0xFF3390EC)
internal val NemoBlueBright = Color(0xFF5CA8F5)

/** Soft sky outgoing bubble (flat fallback). */
internal val NemoOutgoingLight = Color(0xFFD5E8F7)
internal val NemoOutgoingDark = Color(0xFF2B5278)
/**
 * Screen-space outgoing bubble gradient.
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

/** Soft atmosphere for unlock / create identity (cool blues only). */
internal val NemoOnboardGradientLight = listOf(
    Color(0xFFEAF3FA),
    NemoListLight,
    Color(0xFFD6EBFF),
)
internal val NemoOnboardGradientDark = listOf(
    Color(0xFF0B141A),
    NemoChatDark,
    Color(0xFF152838),
)

internal val LocalThemeMode = staticCompositionLocalOf { NemoThemeMode.System }
internal val LocalOnThemeModeChange = staticCompositionLocalOf<(NemoThemeMode) -> Unit> { {} }

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
internal fun nemoDarkTheme(): Boolean = LocalThemeMode.current.resolveDark(isSystemInDarkTheme())

/** Mono (black-and-white) supplements. Original blue constants above are untouched. */
private val MonoOutgoingGradientLight = listOf(
    Color(0xFF3D3D3D), // charcoal — top of screen
    Color(0xFF000000), // black — bottom of screen
)

private val MonoOutgoingGradientDark = listOf(
    Color(0xFFFFFFFF), // white — top of screen
    Color(0xFFD4D4D4), // silver — bottom of screen
)

private val MonoIncomingLight = Color(0xFFFFFFFF)
private val MonoIncomingDark = Color(0xFF171717)
private val MonoChatLight = Color(0xFFE9E7E0)
private val MonoChatDark = Color(0xFF000000)
private val MonoListLight = Color(0xFFF5F3EC)

private val MonoOnboardGradientLight = listOf(
    Color(0xFFF5F3EC),
    MonoListLight,
    Color(0xFFE7E4D9),
)

private val MonoOnboardGradientDark = listOf(
    Color(0xFF000000),
    MonoChatDark,
    Color(0xFF101010),
)

/** Original avatar colors, also referenced by the blue palettes. */
private val AvatarPaletteColors = listOf(
    Color(0xFF3390EC),
    Color(0xFF0369A1),
    Color(0xFF7C3AED),
    Color(0xFFB45309),
    Color(0xFFBE185D),
    Color(0xFF15803D),
    Color(0xFF1D4ED8),
    Color(0xFF0E7490),
)

private val MonoAvatars = listOf(
    Color(0xFF000000),
    Color(0xFF222222),
    Color(0xFF333333),
    Color(0xFF444444),
    Color(0xFF555555),
    Color(0xFF666666),
    Color(0xFF777777),
    Color(0xFF888888),
)

private val MonoLightColors = lightColorScheme(
    primary = Color(0xFF000000),
    onPrimary = Color.White,
    primaryContainer = Color(0xFFE2E2E2),
    onPrimaryContainer = Color(0xFF000000),
    secondary = Color(0xFF404040),
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFE2E2E2),
    onSecondaryContainer = Color(0xFF000000),
    tertiary = Color(0xFF606060),
    onTertiary = Color.White,
    tertiaryContainer = Color(0xFFE8E8E8),
    onTertiaryContainer = Color(0xFF000000),
    surface = Color.White,
    onSurface = Color(0xFF111111),
    surfaceVariant = Color(0xFFEDEDED),
    onSurfaceVariant = Color(0xFF5A5A5A),
    background = MonoListLight,
    onBackground = Color(0xFF111111),
    outline = Color(0xFFD4D4D4),
    error = Color(0xFFB42318),
    surfaceContainerLowest = Color.White,
    surfaceContainerLow = Color(0xFFF5F5F5),
    surfaceContainer = Color(0xFFF0F0F0),
    surfaceContainerHigh = Color(0xFFE8E8E8),
    surfaceContainerHighest = Color(0xFFDEDEDE),
)

private val MonoDarkColors = darkColorScheme(
    primary = Color(0xFFFFFFFF),
    onPrimary = Color(0xFF000000),
    primaryContainer = Color(0xFF2E2E2E),
    onPrimaryContainer = Color(0xFFFFFFFF),
    secondary = Color(0xFFD4D4D4),
    onSecondary = Color(0xFF000000),
    secondaryContainer = Color(0xFF2E2E2E),
    onSecondaryContainer = Color(0xFFFFFFFF),
    tertiary = Color(0xFFBDBDBD),
    onTertiary = Color(0xFF000000),
    tertiaryContainer = Color(0xFF2A2A2A),
    onTertiaryContainer = Color(0xFFFFFFFF),
    surface = Color(0xFF0A0A0A),
    onSurface = Color(0xFFF5F5F5),
    surfaceVariant = Color(0xFF1C1C1C),
    onSurfaceVariant = Color(0xFFA3A3A3),
    background = MonoChatDark,
    onBackground = Color(0xFFF5F5F5),
    outline = Color(0xFF2A2A2A),
    error = Color(0xFFF97066),
    surfaceContainerLowest = Color(0xFF000000),
    surfaceContainerLow = Color(0xFF0A0A0A),
    surfaceContainer = Color(0xFF111111),
    surfaceContainerHigh = Color(0xFF181818),
    surfaceContainerHighest = Color(0xFF212121),
)

private val BlueLightPalette = NemoPalette(
    dark = false,
    mono = false,
    scheme = LightColors,
    outgoingGradient = NemoOutgoingGradientLight,
    incoming = NemoIncomingLight,
    chat = NemoChatLight,
    list = NemoListLight,
    onboardGradient = NemoOnboardGradientLight,
    patternDot = NemoPatternDotLight,
    bubbleShadow = NemoBubbleShadowLight,
    avatars = AvatarPaletteColors,
    mineBody = Color(0xFF0F172A),
    mineMeta = Color(0xFF5B6770).copy(alpha = 0.85f),
)

private val BlueDarkPalette = NemoPalette(
    dark = true,
    mono = false,
    scheme = DarkColors,
    outgoingGradient = NemoOutgoingGradientDark,
    incoming = NemoIncomingDark,
    chat = NemoChatDark,
    list = NemoChatDark,
    onboardGradient = NemoOnboardGradientDark,
    patternDot = NemoPatternDotDark,
    bubbleShadow = NemoBubbleShadowDark,
    avatars = AvatarPaletteColors,
    mineBody = Color(0xFFE8F4FF),
    mineMeta = Color(0xFFB8D4E8).copy(alpha = 0.9f),
)

private val MonoLightPalette = NemoPalette(
    dark = false,
    mono = true,
    scheme = MonoLightColors,
    outgoingGradient = MonoOutgoingGradientLight,
    incoming = MonoIncomingLight,
    chat = MonoChatLight,
    list = MonoListLight,
    onboardGradient = MonoOnboardGradientLight,
    patternDot = NemoPatternDotLight,
    bubbleShadow = NemoBubbleShadowLight,
    avatars = MonoAvatars,
    mineBody = Color(0xFFFFFFFF),
    mineMeta = Color(0xFFFFFFFF).copy(alpha = 0.75f),
)

private val MonoDarkPalette = NemoPalette(
    dark = true,
    mono = true,
    scheme = MonoDarkColors,
    outgoingGradient = MonoOutgoingGradientDark,
    incoming = MonoIncomingDark,
    chat = MonoChatDark,
    list = MonoChatDark,
    onboardGradient = MonoOnboardGradientDark,
    patternDot = NemoPatternDotDark,
    bubbleShadow = NemoBubbleShadowDark,
    avatars = MonoAvatars,
    mineBody = Color(0xFF000000),
    mineMeta = Color(0xFF000000).copy(alpha = 0.65f),
)

internal val LocalNemoPalette = staticCompositionLocalOf { BlueLightPalette }

@Composable
internal fun NemoTheme(mode: NemoThemeMode, onModeChange: (NemoThemeMode) -> Unit, content: @Composable () -> Unit) {
    val dark = mode.resolveDark(isSystemInDarkTheme())
    // Blue entries reference the original constants/objects, so Light/Dark
    // render exactly as before; mono is purely additive.
    val palette = when {
        mode.isMono && dark -> MonoDarkPalette
        mode.isMono -> MonoLightPalette
        dark -> BlueDarkPalette
        else -> BlueLightPalette
    }
    CompositionLocalProvider(
        LocalThemeMode provides mode,
        LocalOnThemeModeChange provides onModeChange,
        LocalNemoPalette provides palette,
    ) {
        MaterialTheme(
            colorScheme = palette.scheme,
            typography = NemoTypography,
            content = content,
        )
    }
}
