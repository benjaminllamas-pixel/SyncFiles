package com.syncfiles.client.android

import android.content.Context
import android.util.Base64
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.syncfiles.client.android.data.api.ApiClientFactory
import com.syncfiles.client.android.data.api.ApiResponse
import com.syncfiles.client.android.data.api.DeleteRequest
import com.syncfiles.client.android.data.api.SyncFilesApi
import com.syncfiles.client.android.data.api.UploadRequest
import com.syncfiles.client.android.data.local.LocalFileSyncStore
import com.syncfiles.client.android.data.local.LocalFile
import com.syncfiles.client.android.data.local.SyncQueueStore
import com.syncfiles.client.android.data.local.SyncStatusStore
import com.syncfiles.client.android.data.local.SyncStatus
import com.syncfiles.client.android.data.storage.SessionStore
import com.syncfiles.client.android.data.storage.StoredSession
import com.syncfiles.client.android.data.util.Hashing
import com.syncfiles.client.android.data.util.SyncRootResolver
import java.io.File
import java.util.UUID

class SyncWorker(
    context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    private val sessionStore = SessionStore(context)
    private val queue = SyncQueueStore(context)
    private val statusStore = SyncStatusStore(context)
    private val localStore = LocalFileSyncStore(context)
    private val tag = "SyncWorker"

    override suspend fun doWork(): Result {
        val session = sessionStore.getActiveSession()
            ?: run {
                statusStore.set(SyncStatus.STATE_IDLE, "Sin sesión activa")
                return Result.success()
            }
        val serverConfig = sessionStore.getServerConfig()
            ?: run {
                statusStore.set(SyncStatus.STATE_IDLE, "Sin servidor configurado")
                return Result.success()
            }

        statusStore.set(SyncStatus.STATE_IN_PROGRESS, "Sincronizando...")
        val api: SyncFilesApi = ApiClientFactory.create(serverConfig.baseUrl) { session.sessionId }

        var anyFailure = false
        val errors = mutableListOf<String>()

        // 1) Reprocesar cola pendiente (uploads/deletes encolados por el watcher o la UI)
        try {
            if (!processQueue(api, session)) anyFailure = true
        } catch (e: Exception) {
            anyFailure = true
            errors += e.message ?: "cola"
            Log.w(tag, "Fallo procesando cola: ${e.message}")
        }

        // 2) Pull remoto: diff + download de cambios de otros dispositivos
        try {
            pullRemote(api, session)
        } catch (e: Exception) {
            anyFailure = true
            errors += e.message ?: "pull"
            Log.w(tag, "Fallo en pull remoto: ${e.message}")
        }

        statusStore.set(
            if (anyFailure) SyncStatus.STATE_ERROR else SyncStatus.STATE_UP_TO_DATE,
            if (anyFailure) errors.joinToString("; ").take(200) else null
        )

        return if (anyFailure) Result.retry() else Result.success()
    }

    private suspend fun processQueue(api: SyncFilesApi, session: StoredSession): Boolean {
        val pending = queue.listByStatus("queued") + queue.listByStatus("retry")
        if (pending.isEmpty()) return true

        var allOk = true
        for (entry in pending) {
            try {
                when (entry.operation) {
                    "upload" -> uploadFile(api, session, entry)
                    "delete" -> api.delete(
                        DeleteRequest(
                            session_id = session.sessionId,
                            device_id = session.deviceId,
                            file_id = entry.fileId,
                            path_hash = Hashing.sha256Hex(entry.relativePath),
                            idempotency_key = entry.idempotencyKey
                        )
                    )
                    else -> Unit
                }
                queue.markDone(entry.queueId)
            } catch (e: Exception) {
                allOk = false
                queue.markRetry(entry.queueId, e.message ?: "error")
                Log.w(tag, "Fallo al procesar ${entry.queueId}: ${e.message}")
            }
        }
        return allOk
    }

    private suspend fun uploadFile(
        api: SyncFilesApi,
        session: StoredSession,
        entry: com.syncfiles.client.android.data.local.QueueEntry
    ) {
        val root = localRoot()
        val file = File(root, entry.relativePath)
        if (!file.exists()) {
            queue.markDone(entry.queueId)
            return
        }
        val content = file.readBytes()
        val text = Base64.encodeToString(content, Base64.NO_WRAP)
        val response: ApiResponse<Unit> = api.upload(
            UploadRequest(
                session_id = session.sessionId,
                device_id = session.deviceId,
                file_id = entry.fileId,
                relative_path = entry.relativePath,
                path_hash = Hashing.sha256Hex(entry.relativePath),
                checksum = Hashing.sha256Hex(content),
                size_bytes = content.size.toLong(),
                modified_at = System.currentTimeMillis(),
                idempotency_key = entry.idempotencyKey,
                content = text
            )
        )
        if (!response.accepted) {
            throw IllegalStateException(response.error?.message ?: "upload rechazado")
        }
        localStore.upsert(
            LocalFile(
                fileId = entry.fileId,
                relativePath = entry.relativePath,
                pathHash = Hashing.sha256Hex(entry.relativePath),
                checksum = Hashing.sha256Hex(content),
                sizeBytes = content.size.toLong(),
                modifiedAt = System.currentTimeMillis(),
                status = "synced",
                lastSyncVersion = 0
            )
        )
    }

    private suspend fun pullRemote(api: SyncFilesApi, session: StoredSession) {
        val diff = api.diff(
            com.syncfiles.client.android.data.api.DiffRequest(
                since = sessionStore.lastServerSeq,
                device_id = session.deviceId,
                session_id = session.sessionId,
                request_id = ApiClientFactory.newRequestId()
            )
        )

        for (change in diff.changes) {
            if (change.device_id == session.deviceId) continue
            when (change.operation) {
                "upload" -> {
                    val relative = change.relative_path ?: continue
                    val root = localRoot()
                    val target = File(root, relative)
                    val response = api.download(
                        com.syncfiles.client.android.data.api.DownloadRequest(
                            session_id = session.sessionId,
                            device_id = session.deviceId,
                            file_id = change.file_id,
                            path_hash = change.path_hash,
                            idempotency_key = ApiClientFactory.newIdempotencyKey()
                        )
                    )
                    target.parentFile?.mkdirs()
                    val bytes = Base64.decode(response.content, Base64.NO_WRAP)
                    if (Hashing.sha256Hex(bytes) != response.checksum) {
                        Log.w(tag, "Checksum no coincide al descargar $relative; se guarda el contenido del servidor")
                    }
                    target.writeBytes(bytes)
                    localStore.upsert(
                        LocalFile(
                            fileId = change.file_id,
                            relativePath = relative,
                            pathHash = change.path_hash,
                            checksum = response.checksum,
                            sizeBytes = bytes.size.toLong(),
                            modifiedAt = System.currentTimeMillis(),
                            status = "synced",
                            lastSyncVersion = 0
                        )
                    )
                }
                "delete" -> {
                    val relative = change.relative_path ?: continue
                    val target = File(localRoot(), relative)
                    if (target.exists() && target.delete()) {
                        localStore.deleteByPathHash(change.path_hash)
                        Log.i(tag, "Borrado local: $relative")
                    }
                }
                else -> Unit
            }
        }

        sessionStore.lastServerSeq = diff.server_seq
    }

    fun localRoot(): File {
        return SyncRootResolver.resolveRoot(applicationContext, sessionStore.getSyncRootUri())
    }

    companion object {
        const val WORK_NAME = "syncfiles_sync_worker"
        const val DEFAULT_FILE_ID = ""
    }
}
