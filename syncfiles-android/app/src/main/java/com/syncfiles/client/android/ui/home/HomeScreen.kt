package com.syncfiles.client.android.ui.home

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Logout
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.CloudDone
import androidx.compose.material.icons.filled.CloudOff
import androidx.compose.material.icons.filled.CloudSync
import androidx.compose.material.icons.filled.CloudUpload
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.Folder
import androidx.compose.material.icons.filled.FolderOpen
import androidx.compose.material.icons.filled.PauseCircleOutline
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.syncfiles.client.android.R
import com.syncfiles.client.android.data.api.ChangeEntry
import com.syncfiles.client.android.data.local.SyncStatus
import com.syncfiles.client.android.ui.theme.ElevatedCard
import com.syncfiles.client.android.ui.theme.InitialsAvatar
import kotlinx.coroutines.flow.StateFlow
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(
    viewModel: HomeViewModel,
    onSessionExpired: () -> Unit,
    onOpenFiles: () -> Unit = {},
    onOpenConflicts: () -> Unit = {},
    onOpenSettings: () -> Unit = {}
) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()
    val changes by viewModel.changes.collectAsStateWithLifecycle()
    val syncInFlight by viewModel.syncInFlight.collectAsStateWithLifecycle()
    val uploadInFlight by viewModel.uploadInFlight.collectAsStateWithLifecycle()
    val lastMessage by viewModel.lastMessage.collectAsStateWithLifecycle()
    val syncStatus by viewModel.syncStatus.collectAsStateWithLifecycle()
    val snackbarHostState = remember { SnackbarHostState() }

    val pickFileLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument()
    ) { uri ->
        uri?.let { viewModel.uploadFromUri(it) }
    }

    val pickFolderLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocumentTree()
    ) { uri ->
        uri?.let { viewModel.onSyncRootPicked(it) }
    }

    LaunchedEffect(lastMessage) {
        viewModel.refreshSyncStatus()
    }

    LaunchedEffect(state) {
        if (state is HomeUiState.Error) {
            onSessionExpired()
        }
    }

    LaunchedEffect(lastMessage) {
        lastMessage?.let {
            snackbarHostState.showSnackbar(it)
            viewModel.clearMessage()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Text(
                        "SyncFiles",
                        fontWeight = FontWeight.SemiBold
                    )
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = Color.Transparent
                ),
                actions = {
                    IconButton(onClick = onOpenSettings) {
                        Icon(Icons.Filled.Settings, contentDescription = "Ajustes")
                    }
                    IconButton(onClick = { viewModel.logout { onSessionExpired() } }) {
                        Icon(Icons.AutoMirrored.Filled.Logout, contentDescription = stringResource(R.string.logout))
                    }
                }
            )
        },
        snackbarHost = {
            SnackbarHost(snackbarHostState) { data ->
                Snackbar(snackbarData = data)
            }
        },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                onClick = { pickFileLauncher.launch(arrayOf("*/*")) },
                icon = {
                    if (uploadInFlight) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(18.dp),
                            strokeWidth = 2.dp
                        )
                    } else {
                        Icon(Icons.Filled.Add, contentDescription = null)
                    }
                },
                text = { Text("Subir archivo") },
                containerColor = MaterialTheme.colorScheme.primary,
                contentColor = MaterialTheme.colorScheme.onPrimary
            )
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp)
        ) {
            when (val s = state) {
                is HomeUiState.Loading -> {
                    Box(
                        modifier = Modifier.fillMaxWidth(),
                        contentAlignment = Alignment.Center
                    ) {
                        CircularProgressIndicator(modifier = Modifier.padding(32.dp))
                    }
                }

                is HomeUiState.Active -> {
                    SessionHeader(s)
                    SyncHeroCard(
                        status = syncStatus,
                        syncInFlight = syncInFlight,
                        onSync = { viewModel.syncNow() },
                        onRefreshStatus = { viewModel.refreshSyncStatus() }
                    )
                    SyncRootCard(
                        name = viewModel.syncRootName,
                        onPick = { pickFolderLauncher.launch(null) }
                    )
                    NavigationRow(
                        onOpenFiles = onOpenFiles,
                        onOpenConflicts = onOpenConflicts
                    )
                    ChangesList(changes)
                    Spacer(Modifier.height(80.dp)) // espacio para el FAB
                }

                is HomeUiState.Error -> {
                    Text(stringResource(R.string.session_expired))
                }
            }
        }
    }
}

@Composable
private fun SyncHeroCard(
    status: SyncStatus,
    syncInFlight: Boolean,
    onSync: () -> Unit,
    onRefreshStatus: () -> Unit
) {
    val (icon, title, subtitle, colors) = when (status.state) {
        SyncStatus.STATE_IN_PROGRESS -> HeroStyle(
            Icons.Filled.CloudSync, "Sincronizando…",
            status.message ?: "Transferiendo cambios",
            listOf(MaterialTheme.colorScheme.primary, MaterialTheme.colorScheme.tertiary)
        )
        SyncStatus.STATE_UP_TO_DATE -> HeroStyle(
            Icons.Filled.CloudDone, "Todo al día",
            "Última sincronización: ${formatTimestamp(status.updatedAt)}",
            listOf(Color(0xFF2E7D5B), Color(0xFF57B884))
        )
        SyncStatus.STATE_ERROR -> HeroStyle(
            Icons.Filled.ErrorOutline, "Error de sincronización",
            status.message ?: "Revisa la conexión y reintenta",
            listOf(Color(0xFF9E2B2B), Color(0xFFD64545))
        )
        SyncStatus.STATE_PAUSED -> HeroStyle(
            Icons.Filled.PauseCircleOutline, "Sincronización pausada",
            "Reanuda desde ajustes",
            listOf(Color(0xFF5A5D72), Color(0xFF8A8DA3))
        )
        else -> HeroStyle(
            Icons.Filled.CloudOff, "Sin sincronizar aún",
            "Pulsa sincronizar para empezar",
            listOf(Color(0xFF3A4152), Color(0xFF5C6779))
        )
    }

    val syncing = status.state == SyncStatus.STATE_IN_PROGRESS || syncInFlight
    val rotation = if (syncing) {
        val transition = rememberInfiniteTransition(label = "sync-spin")
        transition.animateFloat(
            initialValue = 0f,
            targetValue = 360f,
            animationSpec = infiniteRepeatable(tween(1200, easing = LinearEasing)),
            label = "sync-angle"
        ).value
    } else 0f

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .background(
                brush = Brush.linearGradient(colors),
                shape = RoundedCornerShape(24.dp)
            )
            .padding(20.dp)
    ) {
        Column {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Icon(
                    imageVector = icon,
                    contentDescription = null,
                    tint = Color.White,
                    modifier = Modifier
                        .size(40.dp)
                        .graphicsLayer { rotationZ = rotation }
                )
                Spacer(Modifier.size(14.dp))
                Column {
                    Text(
                        title,
                        style = MaterialTheme.typography.titleLarge,
                        fontWeight = FontWeight.SemiBold,
                        color = Color.White
                    )
                    Text(
                        subtitle,
                        style = MaterialTheme.typography.bodySmall,
                        color = Color.White.copy(alpha = 0.85f),
                        maxLines = 1
                    )
                }
            }
            Spacer(Modifier.height(16.dp))
            Button(
                onClick = onSync,
                enabled = !syncing && !syncInFlight,
                shape = RoundedCornerShape(14.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = Color.White.copy(alpha = 0.18f),
                    contentColor = Color.White
                ),
                modifier = Modifier.fillMaxWidth()
            ) {
                if (syncing) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(18.dp),
                        strokeWidth = 2.dp,
                        color = Color.White
                    )
                } else {
                    Icon(Icons.Filled.CloudSync, contentDescription = null)
                }
                Spacer(Modifier.size(8.dp))
                Text(stringResource(R.string.sync_now))
            }
        }
    }
    LaunchedEffect(status) { if (!syncing) onRefreshStatus() }
}

private data class HeroStyle(
    val icon: ImageVector,
    val title: String,
    val subtitle: String,
    val colors: List<Color>
)

@Composable
private fun SessionHeader(s: HomeUiState.Active) {
    ElevatedCard(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            InitialsAvatar(
                text = s.session.userId,
                modifier = Modifier.size(44.dp)
            )
            Spacer(Modifier.size(14.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    stringResource(R.string.session_active),
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.SemiBold
                )
                Text(
                    "Dispositivo: ${shorten(s.session.deviceId)}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            if (s.checking) {
                CircularProgressIndicator(
                    modifier = Modifier.size(18.dp),
                    strokeWidth = 2.dp
                )
            }
        }
    }
}

private fun shorten(id: String, max: Int = 12): String =
    if (id.length <= max) id else id.take(max) + "…"

@Composable
private fun NavigationRow(
    onOpenFiles: () -> Unit,
    onOpenConflicts: () -> Unit
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        ElevatedCard(
            modifier = Modifier.weight(1f),
            onClick = onOpenFiles,
            content = {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(16.dp),
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    Icon(
                        Icons.Filled.Folder,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.size(28.dp)
                    )
                    Spacer(Modifier.height(6.dp))
                    Text(
                        "Archivos",
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.Medium
                    )
                }
            }
        )
        ElevatedCard(
            modifier = Modifier.weight(1f),
            onClick = onOpenConflicts,
            content = {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(16.dp),
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    Icon(
                        Icons.Filled.ErrorOutline,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.tertiary,
                        modifier = Modifier.size(28.dp)
                    )
                    Spacer(Modifier.height(6.dp))
                    Text(
                        "Conflictos",
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.Medium
                    )
                }
            }
        )
    }
}

@Composable
private fun SyncRootCard(
    name: StateFlow<String?>,
    onPick: () -> Unit
) {
    val rootName by name.collectAsStateWithLifecycle()
    ElevatedCard(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Icon(
                Icons.Filled.FolderOpen,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(Modifier.size(14.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = stringResource(R.string.sync_root_label),
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.Medium
                )
                Text(
                    text = rootName ?: stringResource(R.string.sync_root_not_set),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            TextButton(onClick = onPick) {
                Text(stringResource(R.string.sync_root_pick))
            }
        }
    }
}

@Composable
private fun ChangesList(changes: List<ChangeEntry>) {
    if (changes.isEmpty()) return
    Text(
        "Actividad reciente",
        style = MaterialTheme.typography.titleSmall,
        fontWeight = FontWeight.SemiBold,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(top = 4.dp)
    )
    LazyColumn(
        modifier = Modifier
            .fillMaxWidth()
            .height(240.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        contentPadding = PaddingValues(bottom = 8.dp)
    ) {
        items(changes, key = { it.file_id }) { change ->
            ElevatedCard(modifier = Modifier.fillMaxWidth()) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(14.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(
                        imageVector = if (change.operation == "delete") Icons.Filled.CloudOff
                        else Icons.Filled.CloudUpload,
                        contentDescription = null,
                        tint = if (change.operation == "delete") MaterialTheme.colorScheme.error
                        else MaterialTheme.colorScheme.primary,
                        modifier = Modifier.size(20.dp)
                    )
                    Spacer(Modifier.size(12.dp))
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            change.relative_path ?: change.path_hash,
                            style = MaterialTheme.typography.bodyMedium,
                            maxLines = 1
                        )
                        Text(
                            "${change.operation} · ${formatTimestamp(change.modified_at)}",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }
            }
        }
    }
}

private fun formatTimestamp(millis: Long): String {
    if (millis <= 0L) return "-"
    val fmt = SimpleDateFormat("dd/MM HH:mm", Locale.getDefault())
    return fmt.format(Date(millis))
}
