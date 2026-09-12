package com.syncfiles.client.android.ui.share

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Estado en memoria para el share "in-flight" (recién compartido, aún no
 * confirmado). Vive solo en el proceso: si el proceso muere antes de
 * confirmar, el share se pierde (los confirmados como pendientes viven en
 * [com.syncfiles.client.android.data.local.PendingSharesStore]).
 */
object ShareInbox {

    private val inFlightContent = MutableStateFlow<String?>(null)

    /** Contenido compartido esperando ser mostrado en la pantalla de guardado. */
    val inFlight: StateFlow<String?> = inFlightContent.asStateFlow()

    /**
     * Evita re-navegar a la pantalla de pendientes cada vez que Home se
     * re-compone (p.ej. al volver de la propia pantalla de guardado con
     * back). Se resetea al iniciar sesión o al recibir un nuevo pendiente.
     */
    @Volatile
    var pendingAutoNavDone: Boolean = false

    fun publish(content: String) {
        inFlightContent.value = content
    }

    /** Consume el contenido dado (atómico; no pisa un share más nuevo). */
    fun consume(content: String) {
        inFlightContent.compareAndSet(content, null)
    }
}
