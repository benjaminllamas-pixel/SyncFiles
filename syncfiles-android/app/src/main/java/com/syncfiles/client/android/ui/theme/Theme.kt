package com.syncfiles.client.android.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

// Paleta "Midnight" — dark elegante con acento indigo/cian
private val IndigoLight = Color(0xFFB4C5FF)
private val IndigoDark = Color(0xFF4A5FE8)
private val CyanAccent = Color(0xFF6FD3FF)

private val DarkColors = darkColorScheme(
    primary = IndigoLight,
    onPrimary = Color(0xFF0B1A78),
    primaryContainer = IndigoDark,
    onPrimaryContainer = Color(0xFFDEE5FF),
    secondary = Color(0xFFC3C5D9),
    onSecondary = Color(0xFF2D2F42),
    secondaryContainer = Color(0xFF434558),
    onSecondaryContainer = Color(0xFFDFE1F9),
    tertiary = CyanAccent,
    onTertiary = Color(0xFF00344F),
    tertiaryContainer = Color(0xFF004B71),
    onTertiaryContainer = Color(0xFFCCE5FF),
    error = Color(0xFFFFB4AB),
    onError = Color(0xFF690005),
    errorContainer = Color(0xFF93000A),
    onErrorContainer = Color(0xFFFFDAD6),
    background = Color(0xFF0B0E13),
    onBackground = Color(0xFFE3E1E9),
    surface = Color(0xFF0B0E13),
    onSurface = Color(0xFFE3E1E9),
    surfaceVariant = Color(0xFF131820),
    onSurfaceVariant = Color(0xFFA6ABB8),
    outline = Color(0xFF565B68),
    surfaceContainer = Color(0xFF11151C),
    surfaceContainerHigh = Color(0xFF191E27),
    surfaceContainerHighest = Color(0xFF22272F),
    surfaceContainerLow = Color(0xFF0E1218),
    surfaceContainerLowest = Color(0xFF080B0F)
)

@Composable
fun SyncFilesTheme(
    content: @Composable () -> Unit
) {
    MaterialTheme(
        colorScheme = DarkColors,
        content = content
    )
}
