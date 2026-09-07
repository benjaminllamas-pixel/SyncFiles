package com.syncfiles.client.android.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val LightColors = lightColorScheme(
    primary = Color(0xFF0055A4),
    onPrimary = Color.White,
    primaryContainer = Color(0xFFD4E3FF),
    onPrimaryContainer = Color(0xFF001C3A),
    secondary = Color(0xFF535F71),
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFD7E3F8),
    onSecondaryContainer = Color(0xFF101C2B),
    error = Color(0xFFBA1A1A),
    onError = Color.White,
    surface = Color(0xFFF9F9FF),
    onSurface = Color(0xFF191C20)
)

private val DarkColors = darkColorScheme(
    primary = Color(0xFFA7C8FF),
    onPrimary = Color(0xFF00315E),
    primaryContainer = Color(0xFF004784),
    onPrimaryContainer = Color(0xFFD4E3FF),
    secondary = Color(0xFFBBC7DB),
    onSecondary = Color(0xFF25313F),
    secondaryContainer = Color(0xFF3C4857),
    onSecondaryContainer = Color(0xFFD7E3F8),
    error = Color(0xFFFFB4AB),
    onError = Color(0xFF690005),
    surface = Color(0xFF111318),
    onSurface = Color(0xFFE1E2E8)
)

@Composable
fun SyncFilesTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit
) {
    MaterialTheme(
        colorScheme = if (darkTheme) DarkColors else LightColors,
        content = content
    )
}
