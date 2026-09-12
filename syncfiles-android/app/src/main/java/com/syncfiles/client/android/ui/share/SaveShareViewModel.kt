package com.syncfiles.client.android.ui.share

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.syncfiles.client.android.data.local.PendingSharesStore
import com.syncfiles.client.android.data.local.SyncEngine
import com.syncfiles.client.android.data.storage.SessionStore
import com.syncfiles.client.android.data.util.ShareNaming
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.util.Date

data class SaveShareUiState(
    val content: String = "",
    val suggestedName: String = "",
    val name: String = "",
    val previewPath: String? = null,
    val saving: Boolean = false,
    val lastMessage: String? = null,
    val isPending: Boolean = false,
    val pendingRemaining: Int = 0,
    val finished: Boolean = false
)

/**
 * ViewModel de la pantalla "Guardar compartido": muestra el contenido
 * recibido por ACTION_SEND, permite nombrar el archivo y confirmar/descartar.
 * Procesa también los shares pendientes almacenados en SQLite.
 */
class SaveShareViewModel(
    private val store: SessionStore,
    private val engine: SyncEngine,
    private val pendingStore: PendingSharesStore
) : ViewModel() {

    private var pendingId: Long? = null
    private var fallbackPendingId: Long? = null

    private val _uiState = MutableStateFlow(SaveShareUiState())
    val uiState: StateFlow<SaveShareUiState> = _uiState.asStateFlow()

    /** Carga un share in-flight (recién compartido, vive solo en memoria). */
    fun loadInFlight(content: String) {
        if (content.isBlank()) {
            _uiState.value = SaveShareUiState(finished = true)
            return
        }
        loadContent(content, isPending = false, pendingRowId = null, message = null)
    }

    /** Carga el share pendiente más antiguo de SQLite. Devuelve false si no hay. */
    fun loadOldestPending(): Boolean {
        val oldest = pendingStore.oldest() ?: return false
        loadContent(oldest.content, isPending = true, pendingRowId = oldest.id, message = null)
        return true
    }

    private fun loadContent(content: String, isPending: Boolean, pendingRowId: Long?, message: String?) {
        val suggestion = ShareNaming.ensureTxtExtension(
            ShareNaming.sanitizeFileName(ShareNaming.suggestFileName(content, Date()))
        )
        pendingId = pendingRowId
        fallbackPendingId = null
        _uiState.value = SaveShareUiState(
            content = content,
            suggestedName = suggestion,
            name = suggestion,
            previewPath = previewPathFor(suggestion),
            isPending = isPending,
            pendingRemaining = pendingStore.count(),
            lastMessage = message
        )
    }

    fun updateName(raw: String) {
        val state = _uiState.value
        if (state.saving || state.finished) return
        _uiState.value = state.copy(
            name = raw,
            previewPath = previewPathFor(raw)
        )
    }

    private fun previewPathFor(name: String): String? {
        if (name.isBlank()) return null
        val rootName = store.getSyncRootName() ?: "carpeta-sync"
        val safe = ShareNaming.ensureTxtExtension(ShareNaming.sanitizeFileName(name))
        return "$rootName/shared/$safe"
    }

    fun save() {
        val state = _uiState.value
        if (state.saving || state.finished || state.content.isBlank()) return
        val content = state.content
        val rawName = state.name
        _uiState.value = state.copy(saving = true)

        viewModelScope.launch {
            val finalName = withContext(Dispatchers.IO) {
                val root = engine.resolveRootFile() ?: return@withContext null
                val sharedDir = File(root, "shared").apply { mkdirs() }
                val name = ShareNaming.ensureTxtExtension(
                    ShareNaming.sanitizeFileName(rawName)
                )
                ShareNaming.dedupeFileName(sharedDir, name)
            }

            val savedPath = if (finalName == null) null
            else withContext(Dispatchers.IO) { engine.ingestNamedShare(content, finalName) }

            // Si llegó un nuevo share a mitad del guardado, la pantalla fue
            // reemplazada: no pisar su estado.
            if (_uiState.value.content != content) return@launch

            if (savedPath != null) {
                pendingId?.let { pendingStore.delete(it) }
                fallbackPendingId?.let { pendingStore.delete(it) }
                pendingId = null
                fallbackPendingId = null
                val message = "Guardado en $savedPath"
                if (!loadNextPending(message)) {
                    _uiState.value = _uiState.value.copy(
                        saving = false,
                        lastMessage = message,
                        finished = true
                    )
                }
            } else {
                // Fallback: si vino in-flight, persistirlo como pendiente
                // (una sola vez) para no perderlo.
                if (!_uiState.value.isPending && fallbackPendingId == null) {
                    fallbackPendingId = withContext(Dispatchers.IO) {
                        pendingStore.insert(content)
                    }
                }
                _uiState.value = _uiState.value.copy(
                    saving = false,
                    pendingRemaining = pendingStore.count(),
                    lastMessage = "No se pudo guardar: queda como pendiente — inicia sesión y elige una carpeta"
                )
            }
        }
    }

    fun discard() {
        val state = _uiState.value
        if (state.saving || state.finished) return
        pendingId?.let { pendingStore.delete(it) }
        fallbackPendingId?.let { pendingStore.delete(it) }
        pendingId = null
        fallbackPendingId = null
        if (!loadNextPending("Descartado")) {
            _uiState.value = _uiState.value.copy(lastMessage = "Descartado", finished = true)
        }
    }

    /**
     * Carga el siguiente pendiente en la misma pantalla conservando el
     * mensaje del action recién ejecutado. Devuelve false si no quedan.
     */
    private fun loadNextPending(message: String): Boolean {
        val oldest = pendingStore.oldest() ?: return false
        loadContent(oldest.content, isPending = true, pendingRowId = oldest.id, message = message)
        return true
    }

    fun clearMessage() {
        _uiState.value = _uiState.value.copy(lastMessage = null)
    }

    class Factory(
        private val store: SessionStore,
        private val engine: SyncEngine,
        private val pendingStore: PendingSharesStore
    ) : ViewModelProvider.Factory {
        @Suppress("UNCHECKED_CAST")
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            return SaveShareViewModel(store, engine, pendingStore) as T
        }
    }
}
