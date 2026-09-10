package com.syncfiles.client.android.ui.settings

import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.syncfiles.client.android.data.api.ApiClientFactory
import com.syncfiles.client.android.data.api.SyncFilesApi
import com.syncfiles.client.android.data.local.SyncEngine
import com.syncfiles.client.android.data.storage.ServerConfig
import com.syncfiles.client.android.data.storage.SessionStore
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class SettingsUiState(
    val serverUrl: String = "",
    val syncRootName: String? = null,
    val intervalMinutes: Long = SessionStore.DEFAULT_SYNC_INTERVAL_MINUTES,
    val saving: Boolean = false,
    val saved: Boolean = false,
    val lastMessage: String? = null
)

class SettingsViewModel(
    private val appContext: Context,
    private val store: SessionStore,
    private val syncEngine: SyncEngine
) : ViewModel() {

    private val _uiState = MutableStateFlow(
        SettingsUiState(
            serverUrl = store.getServerConfig()?.baseUrl ?: "",
            syncRootName = store.getSyncRootName(),
            intervalMinutes = store.syncIntervalMinutes
        )
    )
    val uiState: StateFlow<SettingsUiState> = _uiState.asStateFlow()

    fun updateServerUrl(url: String) {
        _uiState.value = _uiState.value.copy(serverUrl = url, saved = false)
    }

    fun updateInterval(minutes: Long) {
        _uiState.value = _uiState.value.copy(intervalMinutes = minutes, saved = false)
    }

    fun onSyncRootPicked(uri: android.net.Uri) {
        val takeFlags = android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION or
            android.content.Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        appContext.contentResolver.takePersistableUriPermission(uri, takeFlags)
        val document = androidx.documentfile.provider.DocumentFile.fromTreeUri(appContext, uri)
        val displayName = document?.name ?: uri.lastPathSegment
        store.saveSyncRoot(uri.toString(), displayName)
        _uiState.value = _uiState.value.copy(
            syncRootName = displayName,
            saved = false,
            lastMessage = "Carpeta actualizada: $displayName"
        )
        syncEngine.onSyncRootChanged()
    }

    fun save() {
        val url = _uiState.value.serverUrl.trim().trimEnd('/')
        if (url.isEmpty()) {
            _uiState.value = _uiState.value.copy(lastMessage = "La URL del servidor no puede estar vacía")
            return
        }
        _uiState.value = _uiState.value.copy(saving = true)
        viewModelScope.launch {
            try {
                val session = store.getActiveSession()
                if (session != null) {
                    val api: SyncFilesApi = ApiClientFactory.create(url) { session.sessionId }
                    api.sessionStatus()
                }
                store.saveServerConfig(ServerConfig(url))
                store.syncIntervalMinutes = _uiState.value.intervalMinutes
                syncEngine.onSyncRootChanged()
                _uiState.value = _uiState.value.copy(saving = false, saved = true)
            } catch (e: Exception) {
                _uiState.value = _uiState.value.copy(
                    saving = false,
                    lastMessage = "No se pudo validar el servidor: ${e.message} (guardado igualmente)"
                )
                store.saveServerConfig(ServerConfig(url))
                store.syncIntervalMinutes = _uiState.value.intervalMinutes
            }
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
                        // Logout best-effort
                    } finally {
                        syncEngine.stop()
                        store.clearSession()
                        onLoggedOut()
                    }
                }
                return
            }
        }
        syncEngine.stop()
        store.clearSession()
        onLoggedOut()
    }

    fun clearMessage() {
        _uiState.value = _uiState.value.copy(lastMessage = null)
    }

    class Factory(
        private val appContext: Context,
        private val store: SessionStore,
        private val syncEngine: SyncEngine
    ) : ViewModelProvider.Factory {
        @Suppress("UNCHECKED_CAST")
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            return SettingsViewModel(appContext, store, syncEngine) as T
        }
    }
}
