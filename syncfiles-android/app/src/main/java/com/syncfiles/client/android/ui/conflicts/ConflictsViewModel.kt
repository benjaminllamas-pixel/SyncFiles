package com.syncfiles.client.android.ui.conflicts

import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.syncfiles.client.android.data.api.ApiClientFactory
import com.syncfiles.client.android.data.api.ConflictItem
import com.syncfiles.client.android.data.api.ResolveConflictRequest
import com.syncfiles.client.android.data.api.SyncFilesApi
import com.syncfiles.client.android.data.storage.SessionStore
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class ConflictsUiState(
    val loading: Boolean = false,
    val conflicts: List<ConflictItem> = emptyList(),
    val resolvingId: String? = null,
    val lastMessage: String? = null
)

class ConflictsViewModel(
    private val appContext: Context,
    private val store: SessionStore
) : ViewModel() {

    private val _uiState = MutableStateFlow(ConflictsUiState())
    val uiState: StateFlow<ConflictsUiState> = _uiState.asStateFlow()

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
                val resp = api.conflicts()
                _uiState.value = _uiState.value.copy(
                    loading = false,
                    conflicts = resp.conflicts,
                    lastMessage = null
                )
            } catch (e: Exception) {
                _uiState.value = _uiState.value.copy(
                    loading = false,
                    lastMessage = "Error al cargar conflictos: ${e.message}"
                )
            }
        }
    }

    fun resolveConflict(conflict: ConflictItem, decision: String, preserveAlternative: Boolean) {
        val session = store.getActiveSession() ?: return
        val serverConfig = store.getServerConfig() ?: return
        _uiState.value = _uiState.value.copy(resolvingId = conflict.conflict_id)
        viewModelScope.launch {
            try {
                val api: SyncFilesApi = ApiClientFactory.create(serverConfig.baseUrl) {
                    session.sessionId
                }
                val response = api.resolveConflict(
                    ResolveConflictRequest(
                        session_id = session.sessionId,
                        device_id = session.deviceId,
                        conflict_id = conflict.conflict_id,
                        decision = decision,
                        preserve_alternative = preserveAlternative,
                        new_name = null
                    )
                )
                if (response.accepted) {
                    _uiState.value = _uiState.value.copy(
                        resolvingId = null,
                        lastMessage = "Conflicto resuelto"
                    )
                } else {
                    _uiState.value = _uiState.value.copy(
                        resolvingId = null,
                        lastMessage = "Rechazado: ${response.error?.message ?: "sin detalle"}"
                    )
                }
                refresh()
            } catch (e: Exception) {
                _uiState.value = _uiState.value.copy(
                    resolvingId = null,
                    lastMessage = "Error al resolver: ${e.message}"
                )
            }
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
            return ConflictsViewModel(appContext, store) as T
        }
    }
}
