package com.syncfiles.client.android.data.local

import android.content.Context
import android.net.Uri
import android.os.FileObserver
import android.provider.OpenableColumns
import android.util.Log
import androidx.documentfile.provider.DocumentFile
import com.syncfiles.client.android.data.storage.SessionStore
import com.syncfiles.client.android.data.util.Hashing
import java.io.File
import java.util.UUID
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Observa la carpeta de sincronización elegida (SAF) y encola uploads/deletes
 * en [SyncQueueStore]. Tras encolar, dispara una sincronización inmediata.
 *
 * Nota: FileObserver solo funciona sobre rutas del filesystem; con SAF se usa
 * la ruta subyacente del DocumentFile cuando está disponible (la mayoría de
 * los gestores de archivos exponen /storage/emulated/0/...). Si la ruta no se
 * puede resolver, el watcher queda inactivo y la app usa el escaneo completo
 * al pulsar "Sincronizar ahora".
 */
class FileWatcher(
    private val context: Context,
    private val onEvent: (relativePath: String, isDelete: Boolean) -> Unit
) {

    companion object {
        private const val TAG = "FileWatcher"
        private const val MASK =
            FileObserver.MOVED_TO or FileObserver.MOVED_FROM or FileObserver.CREATE or
                FileObserver.DELETE or FileObserver.MODIFY or FileObserver.CLOSE_WRITE

        fun computeHash(file: File): String? =
            if (file.exists() && file.isFile) Hashing.sha256Hex(file.readBytes()) else null
    }

    private var observer: FileObserver? = null
    private val running = AtomicBoolean(false)

    fun startWatching() {
        stopWatching()
        val root = resolveRootFile() ?: return
        if (!root.exists() || !root.isDirectory) {
            Log.w(TAG, "No se puede observar ${root.absolutePath}: no es un directorio accesible")
            return
        }
        val newObserver = object : FileObserver(root.absolutePath, MASK) {
            override fun onEvent(event: Int, path: String?) {
                if (path == null) return
                val relative = path.trimStart('/')
                if (relative.isEmpty()) return
                when (event and MASK) {
                    FileObserver.DELETE, FileObserver.MOVED_FROM -> {
                        Log.i(TAG, "Evento delete: $relative")
                        onEvent(relative, true)
                    }
                    else -> {
                        Log.i(TAG, "Evento cambio: $relative")
                        onEvent(relative, false)
                    }
                }
            }
        }
        observer = newObserver
        newObserver.startWatching()
        running.set(true)
        Log.i(TAG, "Observando ${root.absolutePath}")
    }

    fun stopWatching() {
        if (running.compareAndSet(true, false)) {
            observer?.stopWatching()
            observer = null
            Log.i(TAG, "Watcher detenido")
        }
    }

    fun isRunning(): Boolean = running.get()

    private fun resolveRootFile(): File? {
        val sessionStore = SessionStore(context)
        val uriString = sessionStore.getSyncRootUri() ?: return null
        return try {
            val uri = Uri.parse(uriString)
            val doc = DocumentFile.fromTreeUri(context, uri)
            val path = doc?.uri?.path ?: return null
            val file = File(path)
            if (file.exists() || file.mkdirs()) file else null
        } catch (e: Exception) {
            Log.w(TAG, "No se pudo resolver la carpeta SAF: ${e.message}")
            null
        }
    }
}
