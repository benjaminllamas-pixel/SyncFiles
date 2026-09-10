package com.syncfiles.client.android.ui.files

import android.content.Context
import android.util.Base64
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.syncfiles.client.android.data.api.ApiClientFactory
import com.syncfiles.client.android.data.api.DownloadRequest
import com.syncfiles.client.android.data.api.FileListItem
import com.syncfiles.client.android.data.api.SyncFilesApi
import com.syncfiles.client.android.data.local.LocalFileSyncStore
import com.syncfiles.client.android.data.local.LocalFile
import com.syncfiles.client.android.data.storage.SessionStore
import com.syncfiles.client.android.data.util.Hashing
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

data class FilesUiState(
    val loading: Boolean = false,
    val files: List<FileListItem> = emptyList(),
    val downloadingId: String? = null,
    val downloadedPath: String? = null,
    val lastMessage: String? = null
)

class FilesViewModel(
    private val appContext: Context,
    private val store: SessionStore,
    private val localStore: LocalFileSyncStore = LocalFileSyncStore(appContext)
) : ViewModel() {

    private val _uiState = MutableStateFlow(FilesUiState())
    val uiState: StateFlow<FilesUiState> = _uiState.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        val session = store.getActiveSession() ?: return
        val serverConfig = store.getServerConfig() ?: return
        _uiState.value = _uiState.value.copy(loading = true)
        viewModelScope.launch {
            try {
                val api: SyncFilesApi = ApiClientFactory.create(serverConfig.baseUrl) {
                    session.sessionId
                }
                val resp = api.filesList()
                _uiState.value = _uiState.value.copy(
                    loading = false,
                    files = resp.files.filter { it.status != "deleted" },
                    lastMessage = null
                )
            } catch (e: Exception) {
                _uiState.value = _uiState.value.copy(
                    loading = false,
                    lastMessage = "Error al listar: ${e.message}"
                )
            }
        }
    }

    fun download(file: FileListItem) {
        val session = store.getActiveSession() ?: return
        val serverConfig = store.getServerConfig() ?: return
        _uiState.value = _uiState.value.copy(downloadingId = file.file_id)
        viewModelScope.launch {
            try {
                val api: SyncFilesApi = ApiClientFactory.create(serverConfig.baseUrl) {
                    session.sessionId
                }
                val resp = api.download(
                    DownloadRequest(
                        session_id = session.sessionId,
                        device_id = session.deviceId,
                        file_id = file.file_id,
                        path_hash = file.path_hash,
                        idempotency_key = ApiClientFactory.newIdempotencyKey()
                    )
                )
                val savedPath = withContext(Dispatchers.IO) {
                    saveDownloadedFile(file, resp.content, resp.checksum)
                }
                _uiState.value = _uiState.value.copy(
                    downloadingId = null,
                    downloadedPath = savedPath,
                    lastMessage = "Descargado en la carpeta sincronizada"
                )
            } catch (e: Exception) {
                _uiState.value = _uiState.value.copy(
                    downloadingId = null,
                    lastMessage = "Error al descargar: ${e.message}"
                )
            }
        }
    }

    private fun saveDownloadedFile(file: FileListItem, contentB64: String, checksum: String): String? {
        return try {
            val bytes = Base64.decode(contentB64, Base64.NO_WRAP)
            val root = resolveRootFile() ?: File(appContext.filesDir, "sync_root").apply { mkdirs() }
            val target = File(root, file.relative_path)
            target.parentFile?.mkdirs()
            target.writeBytes(bytes)
            localStore.upsert(
                LocalFile(
                    fileId = file.file_id,
                    relativePath = file.relative_path,
                    pathHash = file.path_hash,
                    checksum = checksum,
                    sizeBytes = bytes.size.toLong(),
                    modifiedAt = System.currentTimeMillis(),
                    status = "synced",
                    lastSyncVersion = 0
                )
            )
            target.absolutePath
        } catch (e: Exception) {
            null
        }
    }

    private fun resolveRootFile(): File? {
        val uriString = store.getSyncRootUri() ?: return null
        return try {
            val uri = android.net.Uri.parse(uriString)
            val doc = androidx.documentfile.provider.DocumentFile.fromTreeUri(appContext, uri)
            val path = doc?.uri?.path ?: return null
            val f = File(path)
            if (f.exists() || f.mkdirs()) f else null
        } catch (e: Exception) {
            null
        }
    }

    fun clearMessage() {
        _uiState.value = _uiState.value.copy(lastMessage = null)
    }

    class Factory(
        private val appContext: Context,
        private val store: SessionStore
    ) : ViewModelProvider.Factory {
        @Suppress("UNCHECKED_CAST")
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            return FilesViewModel(appContext, store) as T
        }
    }
}
