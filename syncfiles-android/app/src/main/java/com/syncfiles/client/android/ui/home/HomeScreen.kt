package com.syncfiles.client.android.ui.home

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.material.icons.automirrored.filled.Logout
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Sync
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.syncfiles.client.android.R
import com.syncfiles.client.android.data.api.ChangeEntry
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@OptIn(androidx.compose.material3.ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(
    viewModel: HomeViewModel,
    onSessionExpired: () -> Unit
) {
    val state by viewModel.uiState.collectAsStateWithLifecycle()
    val changes by viewModel.changes.collectAsStateWithLifecycle()
    val syncInFlight by viewModel.syncInFlight.collectAsStateWithLifecycle()
    val uploadInFlight by viewModel.uploadInFlight.collectAsStateWithLifecycle()
    val lastMessage by viewModel.lastMessage.collectAsStateWithLifecycle()
    val snackbarHostState = remember { SnackbarHostState() }
    val context = LocalContext.current

    val pickFileLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument()
    ) { uri ->
        uri?.let { viewModel.uploadFromUri(it) }
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
                title = { Text("SyncFiles") },
                actions = {
                    OutlinedButton(
                        onClick = { viewModel.logout { } },
                        modifier = Modifier.padding(end = 8.dp)
                    ) {
                        Icon(Icons.AutoMirrored.Filled.Logout, contentDescription = null)
                        Spacer(Modifier.size(4.dp))
                        Text(stringResource(R.string.logout))
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
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            when (val s = state) {
                is HomeUiState.Loading -> {
                    CircularProgressIndicator(modifier = Modifier.align(Alignment.CenterHorizontally))
                }

                is HomeUiState.Active -> {
                    SessionCard(s)
                    ActionsRow(
                        syncInFlight = syncInFlight,
                        uploadInFlight = uploadInFlight,
                        onSync = { viewModel.syncNow() },
                        onPick = { pickFileLauncher.launch(arrayOf("*/*")) }
                    )
                    ChangesList(changes = changes)
                }

                is HomeUiState.Error -> {
                    Text(stringResource(R.string.session_expired))
                }
            }
        }
    }
}

@Composable
private fun SessionCard(s: HomeUiState.Active) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.primaryContainer
        )
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Text(
                    stringResource(R.string.session_active),
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onPrimaryContainer
                )
                if (s.checking) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(18.dp),
                        strokeWidth = 2.dp
                    )
                }
            }
            Spacer(Modifier.height(8.dp))
            Text(
                "${stringResource(R.string.user_label)}: ${s.session.userId}",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onPrimaryContainer
            )
            Text(
                "${stringResource(R.string.device_label)}: ${s.session.deviceId}",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onPrimaryContainer
            )
            Text(
                "Expira: ${formatTimestamp(s.session.expiresAt)}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onPrimaryContainer
            )
        }
    }
}

@Composable
private fun ActionsRow(
    syncInFlight: Boolean,
    uploadInFlight: Boolean,
    onSync: () -> Unit,
    onPick: () -> Unit
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Button(
            onClick = onSync,
            modifier = Modifier.weight(1f),
            enabled = !syncInFlight && !uploadInFlight
        ) {
            if (syncInFlight) {
                CircularProgressIndicator(
                    modifier = Modifier.size(18.dp),
                    strokeWidth = 2.dp
                )
            } else {
                Icon(Icons.Filled.Sync, contentDescription = null)
            }
            Spacer(Modifier.size(6.dp))
            Text(stringResource(R.string.sync_now))
        }
        Button(
            onClick = onPick,
            modifier = Modifier.weight(1f),
            enabled = !uploadInFlight && !syncInFlight
        ) {
            if (uploadInFlight) {
                CircularProgressIndicator(
                    modifier = Modifier.size(18.dp),
                    strokeWidth = 2.dp
                )
            } else {
                Icon(Icons.Filled.Add, contentDescription = null)
            }
            Spacer(Modifier.size(6.dp))
            Text("Subir")
        }
    }
}

@Composable
private fun ChangesList(changes: List<ChangeEntry>) {
    if (changes.isEmpty()) {
        Text(
            "Sin cambios. Pulsa \"Sincronizar ahora\" para obtener el estado del servidor.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        return
    }
    LazyColumn(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        items(changes, key = { it.file_id }) { change ->
            Card(modifier = Modifier.fillMaxWidth()) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Text(
                        change.relative_path ?: change.path_hash,
                        style = MaterialTheme.typography.bodyLarge
                    )
                    Text(
                        "op: ${change.operation} · ${change.size_bytes ?: 0} bytes",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Text(
                        "modificado: ${formatTimestamp(change.modified_at)}",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }
    }
}

private fun formatTimestamp(millis: Long): String {
    if (millis <= 0L) return "-"
    val fmt = SimpleDateFormat("dd/MM/yyyy HH:mm", Locale.getDefault())
    return fmt.format(Date(millis))
}
