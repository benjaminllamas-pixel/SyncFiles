package com.syncfiles.client.android.data.api

import retrofit2.http.Body
import retrofit2.http.GET
import retrofit2.http.POST

data class LoginRequest(
    val email: String,
    val password: String,
    val device_id: String
)

data class LoginResponse(
    val session_id: String,
    val expires_at: Long,
    val device_id: String,
    val user_id: String
)

data class SessionStatus(
    val session_id: String,
    val user_id: String,
    val device_id: String,
    val token_hash: String,
    val issued_at: Long,
    val expires_at: Long,
    val revoked_at: Long?,
    val status: String
)

data class ApiResponse<T>(
    val accepted: Boolean,
    val status: String,
    val server_seq: Long?,
    val data: T?,
    val error: ApiError?
)

data class ApiError(
    val code: String,
    val message: String,
    val retryable: Boolean,
    val request_id: String
)

data class DiffRequest(
    val since: Long,
    val device_id: String,
    val session_id: String,
    val request_id: String
)

data class DiffResponse(
    val changes: List<ChangeEntry>,
    val server_seq: Long
)

data class ChangeEntry(
    val file_id: String,
    val operation: String,
    val path_hash: String,
    val checksum: String,
    val modified_at: Long,
    val device_id: String,
    val relative_path: String?,
    val content: String?,
    val size_bytes: Long?
)

data class UploadRequest(
    val session_id: String,
    val device_id: String,
    val file_id: String,
    val relative_path: String,
    val path_hash: String,
    val checksum: String,
    val size_bytes: Long,
    val modified_at: Long,
    val idempotency_key: String,
    val content: String
)

data class DeleteRequest(
    val session_id: String,
    val device_id: String,
    val file_id: String,
    val path_hash: String,
    val idempotency_key: String
)

data class ResolveConflictRequest(
    val session_id: String,
    val device_id: String,
    val conflict_id: String,
    val decision: String,
    val preserve_alternative: Boolean,
    val new_name: String?
)

data class DownloadRequest(
    val session_id: String,
    val device_id: String,
    val file_id: String,
    val path_hash: String,
    val idempotency_key: String
)

data class DownloadResponse(
    val accepted: Boolean,
    val status: String,
    val checksum: String,
    val content: String,
    val file_id: String
)

interface SyncFilesApi {

    @POST("auth/login")
    suspend fun login(@Body request: LoginRequest): LoginResponse

    @POST("auth/logout")
    suspend fun logout(@Body request: Map<String, String>): ApiResponse<Unit>

    @GET("session/status")
    suspend fun sessionStatus(): SessionStatus

    @POST("sync/diff")
    suspend fun diff(@Body request: DiffRequest): DiffResponse

    @POST("sync/upload")
    suspend fun upload(@Body request: UploadRequest): ApiResponse<Unit>

    @POST("sync/download")
    suspend fun download(@Body request: DownloadRequest): DownloadResponse

    @POST("sync/delete")
    suspend fun delete(@Body request: DeleteRequest): ApiResponse<Unit>

    @POST("conflicts/resolve")
    suspend fun resolveConflict(@Body request: ResolveConflictRequest): ApiResponse<Unit>
}
