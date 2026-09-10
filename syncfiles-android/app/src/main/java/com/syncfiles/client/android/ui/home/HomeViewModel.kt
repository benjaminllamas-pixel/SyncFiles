package com.syncfiles.client.android.ui.home

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import androidx.documentfile.provider.DocumentFile
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.syncfiles.client.android.SyncScheduler
import com.syncfiles.client.android.data.api.ApiClientFactory
import com.syncfiles.client.android.data.api.ApiResponse
import com.syncfiles.client.android.data.api.ChangeEntry
import com.syncfiles.client.android.data.api.DiffRequest
import com.syncfiles.client.android.data.api.SyncFilesApi
import com.syncfiles.client.android.data.api.UploadRequest
import com.syncfiles.client.android.data.local.LocalFileSyncStore
import com.syncfiles.client.android.data.local.SyncEngine
import com.syncfiles.client.android.data.local.SyncStatus
import com.syncfiles.client.android.data.local.SyncStatusStore
import com.syncfiles.client.android.data.storage.SessionStore
import com.syncfiles.client.android.data.storage.StoredSession
import com.syncfiles.client.android.data.util.Hashing
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import java.io.File
import java.util.UUID

sealed class HomeUiState {
    object Loading : HomeUiState()
    data class Active(val session: StoredSession, val checking: Boolean = false) : HomeUiState()
    data class Error(val message: String) : HomeUiState()
}

class HomeViewModel(
    private val appContext: Context,
    private val store: SessionStore,
    private val localStore: LocalFileSyncStore
) : ViewModel() {

    private val syncStatusStore = SyncStatusStore(appContext)

    private val _uiState = MutableStateFlow<HomeUiState>(HomeUiState.Loading)
    val uiState: StateFlow<HomeUiState> = _uiState.asStateFlow()

    private val _changes = MutableStateFlow<List<ChangeEntry>>(emptyList())
    val changes: StateFlow<List<ChangeEntry>> = _changes.asStateFlow()

    private val _syncInFlight = MutableStateFlow(false)
    val syncInFlight: StateFlow<Boolean> = _syncInFlight.asStateFlow()

    private val _uploadInFlight = MutableStateFlow(false)
    val uploadInFlight: StateFlow<Boolean> = _uploadInFlight.asStateFlow()

    private val _lastMessage = MutableStateFlow<String?>(null)
    val lastMessage: StateFlow<String?> = _lastMessage.asStateFlow()

    private val _syncRootName = MutableStateFlow<String?>(null)
    val syncRootName: StateFlow<String?> = _syncRootName.asStateFlow()

    private val _syncStatus = MutableStateFlow(syncStatusStore.get())
    val syncStatus: StateFlow<SyncStatus> = _syncStatus.asStateFlow()

    init {
        checkSession()
        _syncRootName.value = store.getSyncRootName()
        refreshSyncStatus()
    }

    fun refreshSyncStatus() {
        _syncStatus.value = syncStatusStore.get()
    }

    fun checkSession() {
        val session = store.getActiveSession()
        if (session == null) {
            _uiState.value = HomeUiState.Error("expired")
            return
        }
        _uiState.value = HomeUiState.Active(session, checking = true)

        viewModelScope.launch {
            try {
                val serverConfig = store.getServerConfig()
                if (serverConfig == null) {
                    _uiState.value = HomeUiState.Error("missing server config")
                    return@launch
                }
                val api: SyncFilesApi = ApiClientFactory.create(serverConfig.baseUrl) {
                    session.sessionId
                }
                api.sessionStatus()
                _uiState.value = HomeUiState.Active(session, checking = false)
            } catch (e: Exception) {
                store.clearSession()
                _uiState.value = HomeUiState.Error(e.message ?: "session invalid")
            }
        }
    }

    fun syncNow() {
        val active = _uiState.value as? HomeUiState.Active ?: return
        val serverConfig = store.getServerConfig() ?: return
        _syncInFlight.value = true
        viewModelScope.launch {
            try {
                val api: SyncFilesApi = ApiClientFactory.create(serverConfig.baseUrl) {
                    active.session.sessionId
                }
                val resp = api.diff(
                    DiffRequest(
                        since = store.lastServerSeq,
                        device_id = active.session.deviceId,
                        session_id = active.session.sessionId,
                        request_id = ApiClientFactory.newRequestId()
                    )
                )
                _changes.value = resp.changes
                store.lastServerSeq = resp.server_seq
                _lastMessage.value = "Sincronización: ${resp.changes.size} cambios"
                // Dispara también el worker (cola + descargas al sync root)
                SyncScheduler.syncNow(appContext)
            } catch (e: Exception) {
                _lastMessage.value = "Error al sincronizar: ${e.message}"
            } finally {
                _syncInFlight.value = false
                refreshSyncStatus()
            }
        }
    }

    fun uploadFromUri(uri: Uri) {
        val active = _uiState.value as? HomeUiState.Active ?: return
        val serverConfig = store.getServerConfig() ?: return
        _uploadInFlight.value = true
        viewModelScope.launch {
            try {
                val displayName = queryDisplayName(uri) ?: "upload-${System.currentTimeMillis()}"
                val bytes = appContext.contentResolver.openInputStream(uri)?.use { it.readBytes() }
                    ?: throw IllegalStateException("No se pudo leer el archivo")
                val content = android.util.Base64.encodeToString(bytes, android.util.Base64.NO_WRAP)
                val relativePath = displayName
                val pathHash = Hashing.sha256Hex(relativePath)
                val checksum = Hashing.sha256Hex(bytes)

                val api: SyncFilesApi = ApiClientFactory.create(serverConfig.baseUrl) {
                    active.session.sessionId
                }
                val response: ApiResponse<Unit> = api.upload(
                    UploadRequest(
                        session_id = active.session.sessionId,
                        device_id = active.session.deviceId,
                        file_id = UUID.randomUUID().toString(),
                        relative_path = relativePath,
                        path_hash = pathHash,
                        checksum = checksum,
                        size_bytes = bytes.size.toLong(),
                        modified_at = System.currentTimeMillis(),
                        idempotency_key = ApiClientFactory.newIdempotencyKey(),
                        content = content
                    )
                )
                if (response.accepted) {
                    _lastMessage.value = "Subido: $relativePath"
                } else {
                    _lastMessage.value = "Rechazado: ${response.error?.message ?: "sin detalle"}"
                }
            } catch (e: Exception) {
                _lastMessage.value = "Error al subir: ${e.message}"
            } finally {
                _uploadInFlight.value = false
            }
        }
    }

    fun clearMessage() {
        _lastMessage.value = null
    }

    fun onSyncRootPicked(uri: android.net.Uri) {
        val takeFlags = android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION or android.content.Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        appContext.contentResolver.takePersistableUriPermission(uri, takeFlags)

        val document = androidx.documentfile.provider.DocumentFile.fromTreeUri(appContext, uri)
        val displayName = document?.name ?: uri.lastPathSegment

        store.saveSyncRoot(uri.toString(), displayName)
        _syncRootName.value = displayName
        _lastMessage.value = "Carpeta sincronizada: $displayName"

        // Reinicia el watcher sobre la nueva carpeta
        (appContext as? com.syncfiles.client.android.SyncFilesApplication)
            ?.syncEngine?.onSyncRootChanged()

        viewModelScope.launch {
            try {
                val pathFile = resolveRootFile(uri)
                localStore.scanAndUpsert(pathFile)
                _lastMessage.value = "Carpeta escaneada: ${localStore.listAll().size} archivos"
            } catch (e: Exception) {
                _lastMessage.value = "Carpeta seleccionada: $displayName"
            }
        }
    }

    private fun resolveRootFile(uri: android.net.Uri): File {
        val document = androidx.documentfile.provider.DocumentFile.fromTreeUri(appContext, uri)
        val path = document?.uri?.path
        return if (path.isNullOrEmpty()) {
            File(appContext.filesDir, "sync_root").apply { mkdirs() }
        } else {
            File(path)
        }
    }

    fun logout(onLoggedOut: () -> Unit) {
        val session = store.getActiveSession()
        if (session != null) {
            val serverConfig = store.getServerConfig()
            if (serverConfig != null) {
                viewModelScope.launch {
                    try {
                        val api: SyncFilesApi = ApiClientFactory.create(serverConfig.baseUrl) {
                            session.sessionId
                        }
                        api.logout(emptyMap())
                    } catch (_: Exception) {
                        // Logout is best-effort; clear locally regardless
                    } finally {
                        store.clearSession()
                        onLoggedOut()
                    }
                }
                return
            }
        }
        store.clearSession()
        onLoggedOut()
    }

    private fun queryDisplayName(uri: Uri): String? {
        val cursor = appContext.contentResolver.query(uri, null, null, null, null) ?: return null
        return cursor.use {
            val nameIndex = it.getColumnIndex(OpenableColumns.DISPLAY_NAME)
            if (it.moveToFirst() && nameIndex >= 0) it.getString(nameIndex) else null
        }
    }

    class Factory(
        private val appContext: Context,
        private val store: SessionStore,
        private val localStore: LocalFileSyncStore
    ) : ViewModelProvider.Factory {
        @Suppress("UNCHECKED_CAST")
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            return HomeViewModel(appContext, store, localStore) as T
        }
    }
}
