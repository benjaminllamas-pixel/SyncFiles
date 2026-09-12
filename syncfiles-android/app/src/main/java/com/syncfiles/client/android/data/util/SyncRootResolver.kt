package com.syncfiles.client.android.data.util

import android.content.Context
import android.net.Uri
import android.os.Environment
import android.provider.DocumentsContract
import android.util.Log
import java.io.File
import java.util.UUID

/**
 * Resuelve la carpeta raíz de sincronización como una ruta real del
 * filesystem.
 *
 * El picker de carpetas (ACTION_OPEN_DOCUMENT_TREE) devuelve un tree URI de
 * SAF (`content://com.android.externalstorage.documents/tree/primary%3ACarpeta`)
 * que NO es una ruta de archivo: usar `DocumentFile.fromTreeUri(...).uri.path`
 * produce `/tree/primary:Carpeta/document/primary:Carpeta`, que no existe en
 * el sistema de archivos y hace fallar toda operación con [File].
 *
 * Esta clase convierte el tree URI a la ruta real
 * (`/storage/emulated/0/Carpeta`), verifica que sea escribible (archivo de
 * prueba) y, si no lo es (p.ej. Android 10 con scoped storage, o
 * proveedores sin ruta de archivo como Google Drive), cae a la carpeta
 * interna `filesDir/sync_root` — la misma que ya usaba SyncWorker — para que
 * la app nunca pierda archivos silenciosamente.
 */
object SyncRootResolver {

    private const val TAG = "SyncRootResolver"
    private const val PROBE_FILE = ".syncfiles-probe"
    private const val EXTERNAL_STORAGE_AUTHORITY = "com.android.externalstorage.documents"

    @Volatile
    private var cache: Pair<String, File>? = null

    /**
     * Raíz de sincronización como [File]. Nunca devuelve null: garantiza un
     * directorio escribible, con fallback a la carpeta interna de la app.
     *
     * El resultado se cachea por URI (par atómico único: evita que dos
     * hilos — UI, WorkManager worker, Dispatchers.IO — interleaveen la
     * escritura de uri y root por separado y crucen los valores) para no
     * repetir el sondeo de escriturabilidad en cada operación.
     */
    fun resolveRoot(context: Context, treeUriString: String?): File {
        val fallback = File(context.applicationContext.filesDir, "sync_root").apply { mkdirs() }
        if (treeUriString.isNullOrBlank()) return fallback

        cache?.let { (cachedUri, cachedRoot) ->
            if (cachedUri == treeUriString) return cachedRoot
        }

        val root = try {
            val uri = Uri.parse(treeUriString)
            val realPath = realPathFromTreeUri(uri)
            val dir = realPath?.let { File(it) }
            if (dir != null && isWritableDirectory(dir)) dir else fallback
        } catch (e: Exception) {
            Log.w(TAG, "Error resolviendo raíz de sync (uri=$treeUriString): ${e.message}")
            fallback
        }

        if (root == fallback) {
            Log.w(
                TAG,
                "Carpeta SAF no utilizable por ruta de archivo; usando carpeta interna " +
                    "(uri=$treeUriString)"
            )
        } else {
            Log.i(TAG, "Raíz de sync: ${root.absolutePath} (uri=$treeUriString)")
        }
        cache = treeUriString to root
        return root
    }

    /**
     * Ruta visible al usuario para la raíz: el nombre de la carpeta bajo
     * el volumen (p.ej. "Personal") o null si la raíz efectiva es la
     * carpeta interna de la app (fallback) — en cuyo caso la UI debería
     * indicar almacenamiento interno en vez del nombre elegido.
     */
    fun isInternalFallback(context: Context, treeUriString: String?): Boolean {
        if (treeUriString.isNullOrBlank()) return true
        cache?.let { (cachedUri, cachedRoot) ->
            if (cachedUri == treeUriString) {
                return cachedRoot == File(context.applicationContext.filesDir, "sync_root")
            }
        }
        // Sin caché: resolver (esto también llena la caché) y comparar.
        return resolveRoot(context, treeUriString) ==
            File(context.applicationContext.filesDir, "sync_root")
    }

    /**
     * Convierte un tree URI de ExternalStorageProvider en la ruta real del
     * filesystem:
     * `content://.../tree/primary%3ACarpeta` → `/storage/emulated/0/Carpeta`
     * `content://.../tree/1A2B-3C4D%3ACarpeta` → `/storage/1A2B-3C4D/Carpeta`
     *
     * Devuelve null para otros proveedores (Google Drive, etc.) o URIs
     * malformadas.
     */
    fun realPathFromTreeUri(treeUri: Uri): String? {
        if (treeUri.authority != EXTERNAL_STORAGE_AUTHORITY) return null
        val docId = try {
            DocumentsContract.getTreeDocumentId(treeUri)
        } catch (_: IllegalArgumentException) {
            return null
        }
        val colon = docId.indexOf(':')
        if (colon <= 0) return null
        val volume = docId.substring(0, colon)
        val subPath = docId.substring(colon + 1)
        // Un docId legítimo del ExternalStorageProvider nunca contiene
        // segmentos de traversal; si los trae, el URI no es de confianza.
        if (volume.isBlank() || volume.contains("..")) return null
        if (subPath.split('/').any { it == ".." || it == "." }) return null
        val base = if (volume == "primary") {
            @Suppress("DEPRECATION")
            Environment.getExternalStorageDirectory().absolutePath
        } else {
            "/storage/$volume"
        }
        return if (subPath.isBlank()) base else "$base/$subPath"
    }

    /**
     * true si el directorio existe (o se puede crear) y la app puede crear
     * un archivo propio en él. Con scoped storage, tener permiso SAF sobre
     * un tree NO implica poder escribir por ruta de archivo (depende de la
     * versión de Android y del proveedor), así que se verifica de verdad.
     *
     * Solo cuenta como escribible si [File.createNewFile] devuelve true
     * (creación O_EXCL real): un `.syncfiles-probe` preexistente colocado
     * por otra app o un symlink probaría nada, y borrarlo sería eliminar
     * un archivo ajeno. El nombre lleva sufijo aleatorio para evitar la
     * colisión con restos de sondeos previos.
     */
    private fun isWritableDirectory(dir: File): Boolean {
        return try {
            if (!dir.exists() && !dir.mkdirs()) return false
            val probe = File(dir, "$PROBE_FILE-${UUID.randomUUID()}")
            try {
                probe.createNewFile()
            } finally {
                probe.delete()
            }
        } catch (_: Exception) {
            false
        }
    }
}
