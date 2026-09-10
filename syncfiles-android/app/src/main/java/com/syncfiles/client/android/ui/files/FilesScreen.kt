package com.syncfiles.client.android.ui.files

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.Image
import androidx.compose.material.icons.filled.InsertDriveFile
import androidx.compose.material.icons.filled.Movie
import androidx.compose.material.icons.filled.MusicNote
import androidx.compose.material.icons.filled.PictureAsPdf
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.syncfiles.client.android.data.api.FileListItem
import com.syncfiles.client.android.ui.theme.ElevatedCard
import com.syncfiles.client.android.ui.theme.StatusBadge
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
fun FilesScreen(
    viewModel: FilesViewModel,
    onBack: () -> Unit
) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()
    val snackbarHostState = remember { SnackbarHostState() }

    LaunchedEffect(state.lastMessage) {
        state.lastMessage?.let {
            snackbarHostState.showSnackbar(it)
            viewModel.clearMessage()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Archivos", fontWeight = FontWeight.SemiBold) },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = Color.Transparent
                ),
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Volver")
                    }
                },
                actions = {
                    IconButton(onClick = { viewModel.refresh() }) {
                        Icon(Icons.Filled.Refresh, contentDescription = "Refrescar")
                    }
                }
            )
        },
        snackbarHost = {
            SnackbarHost(snackbarHostState) { data ->
                Snackbar(snackbarData = data)
            }
        }
    ) { padding ->
        if (state.loading) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
                contentAlignment = Alignment.Center
            ) {
                CircularProgressIndicator()
            }
        } else if (state.files.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
                contentAlignment = Alignment.Center
            ) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Icon(
                        Icons.Filled.Folder,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(56.dp)
                    )
                    Spacer(Modifier.height(12.dp))
                    Text(
                        "Sin archivos aún",
                        style = MaterialTheme.typography.titleMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Text(
                        "Sube archivos desde la pantalla principal",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        } else {
            LazyColumn(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(padding),
                contentPadding = PaddingValues(
                    start = 20.dp, end = 20.dp,
                    top = 8.dp, bottom = 24.dp
                ),
                verticalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                items(state.files, key = { it.file_id }) { file ->
                    FileCard(
                        file = file,
                        downloading = state.downloadingId == file.file_id,
                        onDownload = { viewModel.download(file) }
                    )
                }
            }
        }
    }
}

@Composable
private fun FileCard(
    file: FileListItem,
    downloading: Boolean,
    onDownload: () -> Unit
) {
    ElevatedCard(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(14.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Icon(
                imageVector = iconForFile(file.relative_path),
                contentDescription = null,
                tint = tintForFile(file.relative_path),
                modifier = Modifier.size(32.dp)
            )
            Spacer(Modifier.size(14.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    file.relative_path,
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = FontWeight.Medium,
                    maxLines = 1
                )
                Text(
                    "${humanSize(file.size_bytes)} · ${formatTimestamp(file.modified_at)}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(Modifier.height(6.dp))
                StatusBadge(
                    text = statusLabel(file.status),
                    color = statusColor(file.status)
                )
            }
            FilledTonalIconButton(
                onClick = onDownload,
                enabled = !downloading,
                colors = IconButtonDefaults.filledTonalIconButtonColors(
                    containerColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.15f),
                    contentColor = MaterialTheme.colorScheme.primary
                )
            ) {
                if (downloading) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(18.dp),
                        strokeWidth = 2.dp
                    )
                } else {
                    Icon(Icons.Filled.Download, contentDescription = "Descargar")
                }
            }
        }
    }
}

private fun iconForFile(name: String): ImageVector = when {
    name.endsWith(".png", true) || name.endsWith(".jpg", true) ||
        name.endsWith(".jpeg", true) || name.endsWith(".gif", true) ||
        name.endsWith(".webp", true) -> Icons.Filled.Image
    name.endsWith(".mp4", true) || name.endsWith(".mkv", true) ||
        name.endsWith(".mov", true) || name.endsWith(".avi", true) -> Icons.Filled.Movie
    name.endsWith(".mp3", true) || name.endsWith(".wav", true) ||
        name.endsWith(".flac", true) || name.endsWith(".ogg", true) -> Icons.Filled.MusicNote
    name.endsWith(".pdf", true) -> Icons.Filled.PictureAsPdf
    else -> Icons.Filled.InsertDriveFile
}

@Composable
private fun tintForFile(name: String): Color = when {
    name.endsWith(".png", true) || name.endsWith(".jpg", true) ||
        name.endsWith(".jpeg", true) || name.endsWith(".gif", true) ||
        name.endsWith(".webp", true) -> Color(0xFF7FC7F4)
    name.endsWith(".mp4", true) || name.endsWith(".mkv", true) ||
        name.endsWith(".mov", true) || name.endsWith(".avi", true) -> Color(0xFFB392F0)
    name.endsWith(".mp3", true) || name.endsWith(".wav", true) ||
        name.endsWith(".flac", true) || name.endsWith(".ogg", true) -> Color(0xFFF2A8C0)
    name.endsWith(".pdf", true) -> Color(0xFFE57373)
    else -> MaterialTheme.colorScheme.onSurfaceVariant
}

private fun humanSize(bytes: Long): String {
    if (bytes < 0) return "-"
    val kb = 1024.0
    return when {
        bytes < kb -> "$bytes B"
        bytes < kb * kb -> String.format(Locale.US, "%.1f KB", bytes / kb)
        bytes < kb * kb * kb -> String.format(Locale.US, "%.1f MB", bytes / kb / kb)
        else -> String.format(Locale.US, "%.1f GB", bytes / kb / kb / kb)
    }
}

private fun statusLabel(status: String): String = when (status) {
    "synced" -> "Sincronizado"
    "pending" -> "Pendiente"
    "conflict" -> "Conflicto"
    "deleted" -> "Eliminado"
    else -> status
}

@Composable
private fun statusColor(status: String): Color = when (status) {
    "synced" -> Color(0xFF57B884)
    "pending" -> MaterialTheme.colorScheme.tertiary
    "conflict" -> MaterialTheme.colorScheme.error
    "deleted" -> MaterialTheme.colorScheme.onSurfaceVariant
    else -> MaterialTheme.colorScheme.onSurfaceVariant
}

private fun formatTimestamp(millis: Long): String {
    if (millis <= 0L) return "-"
    val fmt = SimpleDateFormat("dd/MM/yyyy HH:mm", Locale.getDefault())
    return fmt.format(Date(millis))
}
