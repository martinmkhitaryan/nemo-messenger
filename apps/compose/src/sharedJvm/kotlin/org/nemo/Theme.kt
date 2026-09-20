package org.nemo

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

internal val NemoTeal = Color(0xFF0F766E)
internal val NemoTealBright = Color(0xFF2DD4BF)
internal val NemoOutgoingLight = Color(0xFFD1FAE5)
internal val NemoOutgoingDark = Color(0xFF115E59)
internal val NemoIncomingDark = Color(0xFF1F2C34)
internal val NemoChatLight = Color(0xFFE7EDE9)
internal val NemoChatDark = Color(0xFF0B141A)
internal val NemoListLight = Color(0xFFF7F8FA)

private val LightColors = lightColorScheme(
    primary = NemoTeal,
    onPrimary = Color.White,
    primaryContainer = Color(0xFFCCFBF1),
    onPrimaryContainer = Color(0xFF134E4A),
    secondary = Color(0xFF0EA5E9),
    surface = Color.White,
    onSurface = Color(0xFF0F172A),
    surfaceVariant = Color(0xFFEEF2F0),
    onSurfaceVariant = Color(0xFF5B6770),
    background = NemoListLight,
    onBackground = Color(0xFF0F172A),
    outline = Color(0xFFD0D5DD),
    error = Color(0xFFB42318),
)

private val DarkColors = darkColorScheme(
    primary = NemoTealBright,
    onPrimary = Color(0xFF042F2E),
    primaryContainer = Color(0xFF115E59),
    onPrimaryContainer = Color(0xFFCCFBF1),
    secondary = Color(0xFF38BDF8),
    surface = Color(0xFF152028),
    onSurface = Color(0xFFE8EEF2),
    surfaceVariant = Color(0xFF1F2C34),
    onSurfaceVariant = Color(0xFFAEB8C2),
    background = NemoChatDark,
    onBackground = Color(0xFFE8EEF2),
    outline = Color(0xFF2A3942),
    error = Color(0xFFF97066),
)

@Composable
internal fun NemoTheme(content: @Composable () -> Unit) {
    val dark = isSystemInDarkTheme()
    MaterialTheme(
        colorScheme = if (dark) DarkColors else LightColors,
        content = content,
    )
}
