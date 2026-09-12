package com.syncfiles.client.android.data.local

import android.content.Context
import android.util.Log
import com.syncfiles.client.android.SyncScheduler
import com.syncfiles.client.android.data.storage.SessionStore
import com.syncfiles.client.android.data.util.ShareNaming
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

    /**
     * Guarda un texto compartido desde otra app (ACTION_SEND, p.ej. un URL
     * de una pestaña de Chrome) como `shared/<fileName>` en la carpeta
     * sincronizada y lo encola para subida. El contenido se escribe tal cual
     * llegó. Devuelve la ruta relativa creada o null si no hay sesión/carpeta
     * o el nombre queda vacío tras sanitizar.
     */
    fun ingestNamedShare(content: String, fileName: String): String? {
        if (sessionStore.getActiveSession() == null) return null
        if (content.isBlank()) return null
        val root = resolveRootFile() ?: return null

        return try {
            val safeName = ShareNaming.ensureTxtExtension(
                ShareNaming.sanitizeFileName(fileName)
            )
            if (safeName.isBlank()) return null
            val relativePath = "shared/$safeName"

            val target = File(root, relativePath)
            target.parentFile?.mkdirs()
            target.writeText(content)

            val pathHash = com.syncfiles.client.android.data.util.Hashing.sha256Hex(relativePath)
            val contentBytes = target.readBytes()
            localStore.upsert(
                LocalFile(
                    fileId = UUID.randomUUID().toString(),
                    relativePath = relativePath,
                    pathHash = pathHash,
                    checksum = com.syncfiles.client.android.data.util.Hashing.sha256Hex(contentBytes),
                    sizeBytes = contentBytes.size.toLong(),
                    modifiedAt = target.lastModified(),
                    status = "local",
                    lastSyncVersion = 0
                )
            )
            queue.enqueue(
                fileId = localStore.findByPathHash(pathHash)?.fileId ?: UUID.randomUUID().toString(),
                relativePath = relativePath,
                operation = "upload"
            )
            SyncScheduler.syncNow(appContext)
            Log.i(TAG, "Share guardado en $relativePath")
            relativePath
        } catch (e: Exception) {
            Log.w(TAG, "Error guardando share: ${e.message}")
            null
        }
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
