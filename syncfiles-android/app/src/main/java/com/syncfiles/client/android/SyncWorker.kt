package com.syncfiles.client.android

import android.content.Context
import android.util.Log
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.syncfiles.client.android.data.api.ApiClientFactory
import com.syncfiles.client.android.data.api.ApiResponse
import com.syncfiles.client.android.data.api.DeleteRequest
import com.syncfiles.client.android.data.api.SyncFilesApi
import com.syncfiles.client.android.data.api.UploadRequest
import com.syncfiles.client.android.data.local.SyncQueueStore
import com.syncfiles.client.android.data.storage.SessionStore
import com.syncfiles.client.android.data.util.Hashing
import java.io.File
import java.util.UUID

class SyncWorker(
    context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    private val sessionStore = SessionStore(context)
    private val queue = SyncQueueStore(context)
    private val tag = "SyncWorker"

    override suspend fun doWork(): Result {
        val session = sessionStore.getActiveSession() ?: return Result.success()
        val serverConfig = sessionStore.getServerConfig() ?: return Result.success()

        val pending = queue.listByStatus("queued")
        if (pending.isEmpty()) return Result.success()

        val api: SyncFilesApi = ApiClientFactory.create(serverConfig.baseUrl) { session.sessionId }

        var anyFailure = false
        for (entry in pending) {
            try {
                when (entry.operation) {
                    "upload" -> uploadFile(api, session, entry, serverConfig.baseUrl)
                    "delete" -> api.delete(
                        DeleteRequest(
                            session_id = session.sessionId,
                            device_id = session.deviceId,
                            file_id = entry.fileId,
                            path_hash = entry.idempotencyKey,
                            idempotency_key = entry.idempotencyKey
                        )
                    )
                    else -> Unit
                }
                queue.markDone(entry.queueId)
            } catch (e: Exception) {
                anyFailure = true
                queue.markRetry(entry.queueId, e.message ?: "error")
                Log.w(tag, "Fallo al procesar ${entry.queueId}: ${e.message}")
            }
        }

        return if (anyFailure) Result.retry() else Result.success()
    }

    private suspend fun uploadFile(
        api: SyncFilesApi,
        session: com.syncfiles.client.android.data.storage.StoredSession,
        entry: com.syncfiles.client.android.data.local.QueueEntry,
        baseUrl: String
    ) {
        val root = File(applicationContext.filesDir, "sync_root").apply { mkdirs() }
        val file = File(root, entry.relativePath)
        if (!file.exists()) {
            queue.markDone(entry.queueId)
            return
        }
        val content = file.readBytes()
        val text = String(content, Charsets.UTF_8)
        val response: ApiResponse<Unit> = api.upload(
            UploadRequest(
                session_id = session.sessionId,
                device_id = session.deviceId,
                file_id = entry.fileId,
                relative_path = entry.relativePath,
                path_hash = entry.idempotencyKey,
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
    }

    companion object {
        const val WORK_NAME = "syncfiles_sync_worker"
    }
}