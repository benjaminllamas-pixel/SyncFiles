package com.syncfiles.client.android.ui.login

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.syncfiles.client.android.data.api.ApiClientFactory
import com.syncfiles.client.android.data.api.LoginRequest
import com.syncfiles.client.android.data.api.SyncFilesApi
import com.syncfiles.client.android.data.storage.ServerConfig
import com.syncfiles.client.android.data.storage.SessionStore
import com.syncfiles.client.android.data.storage.StoredSession
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

sealed class LoginUiState {
    object Idle : LoginUiState()
    object Loading : LoginUiState()
    data class Error(val message: String) : LoginUiState()
    data class Success(val session: StoredSession) : LoginUiState()
}

class LoginViewModel(
    private val store: SessionStore
) : ViewModel() {

    private val _uiState = MutableStateFlow<LoginUiState>(LoginUiState.Idle)
    val uiState: StateFlow<LoginUiState> = _uiState.asStateFlow()

    fun login(serverUrl: String, email: String, password: String) {
        val trimmedServer = serverUrl.trim().trimEnd('/')
        if (trimmedServer.isEmpty() || email.isBlank() || password.isBlank()) {
            _uiState.value = LoginUiState.Error("Completa servidor, correo y contraseña")
            return
        }

        _uiState.value = LoginUiState.Loading

        viewModelScope.launch {
            try {
                val api: SyncFilesApi = ApiClientFactory.create(trimmedServer) { null }

                val response = api.login(
                    LoginRequest(
                        email = email.trim(),
                        password = password,
                        device_id = store.deviceId
                    )
                )

                val session = StoredSession(
                    sessionId = response.session_id,
                    userId = response.user_id,
                    deviceId = response.device_id,
                    expiresAt = response.expires_at
                )

                store.saveServerConfig(ServerConfig(trimmedServer))
                store.saveSession(session)

                _uiState.value = LoginUiState.Success(session)
            } catch (e: Exception) {
                _uiState.value = LoginUiState.Error(
                    e.message ?: "Error de red o servidor no disponible"
                )
            }
        }
    }

    fun reset() {
        _uiState.value = LoginUiState.Idle
    }

    class Factory(private val store: SessionStore) : ViewModelProvider.Factory {
        @Suppress("UNCHECKED_CAST")
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            return LoginViewModel(store) as T
        }
    }
}
