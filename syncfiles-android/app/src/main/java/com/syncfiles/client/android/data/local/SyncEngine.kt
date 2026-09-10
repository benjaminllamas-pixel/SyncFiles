package com.syncfiles.client.android.data.local

import android.content.Context
import android.util.Log
import com.syncfiles.client.android.SyncScheduler
import com.syncfiles.client.android.data.storage.SessionStore
import java.io.File
import java.util.UUID

/**
 * Núcleo de sincronización en segundo plano de la app Android:
 * mantiene el [FileWatcher] activo sobre la carpeta elegida, encola los
 * eventos en [SyncQueueStore] y dispara [com.syncfiles.client.android.SyncWorker]
 * vía WorkManager (periódico + on-demand).
 */
class SyncEngine(context: Context) {

    private val appContext = context.applicationContext
    private val sessionStore = SessionStore(appContext)
    private val queue = SyncQueueStore(appContext)
    private val localStore = LocalFileSyncStore(appContext)

    private val watcher = FileWatcher(appContext) { relative, isDelete ->
        onFileEvent(relative, isDelete)
    }

    companion object {
        private const val TAG = "SyncEngine"
    }

    /** Debe llamarse al iniciar sesión o al elegir carpeta. */
    fun start() {
        if (sessionStore.getActiveSession() == null) return
        SyncScheduler.schedulePeriodic(appContext)
        watcher.startWatching()
        Log.i(TAG, "SyncEngine iniciado")
    }

    fun stop() {
        watcher.stopWatching()
        SyncScheduler.cancelPeriodic(appContext)
        Log.i(TAG, "SyncEngine detenido")
    }

    fun isWatching(): Boolean = watcher.isRunning()

    fun syncNow() {
        SyncScheduler.syncNow(appContext)
    }

    fun onSyncRootChanged() {
        watcher.startWatching()
        SyncScheduler.schedulePeriodic(appContext)
    }

    private fun onFileEvent(relativePath: String, isDelete: Boolean) {
        try {
            val pathHash = com.syncfiles.client.android.data.util.Hashing.sha256Hex(relativePath)
            val existing = localStore.findByPathHash(pathHash)
            val fileId = existing?.fileId ?: UUID.randomUUID().toString()

            if (isDelete) {
                if (existing != null && existing.status == "synced") {
                    queue.enqueue(
                        fileId = fileId,
                        relativePath = relativePath,
                        operation = "delete"
                    )
                }
            } else {
                queue.enqueue(
                    fileId = fileId,
                    relativePath = relativePath,
                    operation = "upload"
                )
            }
            SyncScheduler.syncNow(appContext)
        } catch (e: Exception) {
            Log.w(TAG, "Error procesando evento de watcher para $relativePath: ${e.message}")
        }
    }

    fun resolveRootFile(): File? {
        val uriString = sessionStore.getSyncRootUri() ?: return null
        return try {
            val uri = android.net.Uri.parse(uriString)
            val doc = androidx.documentfile.provider.DocumentFile.fromTreeUri(appContext, uri)
            val path = doc?.uri?.path ?: return null
            val file = File(path)
            if (file.exists() || file.mkdirs()) file else null
        } catch (e: Exception) {
            null
        }
    }
}
