use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub user_id: String,
    pub email: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Device {
    pub device_id: String,
    pub user_id: String,
    pub platform: String,
    pub device_name: Option<String>,
    pub last_seen_at: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub session_id: String,
    pub user_id: String,
    pub device_id: String,
    pub token_hash: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub revoked_at: Option<i64>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FileEntry {
    pub file_id: String,
    pub user_id: String,
    pub device_id: String,
    pub relative_path: String,
    pub path_hash: String,
    pub checksum: String,
    pub size_bytes: i64,
    pub modified_at: i64,
    pub synced_at: Option<i64>,
    pub status: String,
    pub last_sync_version: i64,
    pub deleted_at: Option<i64>,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SyncQueueEntry {
    pub queue_id: String,
    pub file_id: String,
    pub user_id: String,
    pub device_id: String,
    pub operation: String,
    pub status: String,
    pub attempts: i32,
    pub idempotency_key: String,
    pub payload_json: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Conflict {
    pub conflict_id: String,
    pub file_id: String,
    pub user_id: String,
    pub device_local: Option<String>,
    pub device_remote: Option<String>,
    pub local_checksum: Option<String>,
    pub remote_checksum: Option<String>,
    pub conflict_type: String,
    pub strategy: String,
    pub created_at: i64,
    pub resolved_at: Option<i64>,
    pub resolved_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditEntry {
    pub audit_id: String,
    pub user_id: Option<String>,
    pub device_id: Option<String>,
    pub event_name: String,
    pub event_type: Option<String>,
    pub payload_json: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub session_id: String,
    pub expires_at: i64,
    pub device_id: String,
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffRequest {
    pub since: i64,
    pub device_id: String,
    pub session_id: String,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResponse {
    pub changes: Vec<ChangeEntry>,
    pub server_seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEntry {
    pub file_id: String,
    pub operation: String,
    pub path_hash: String,
    pub checksum: String,
    pub modified_at: i64,
    pub device_id: String,
    pub relative_path: Option<String>,
    pub content: Option<String>,
    pub size_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadRequest {
    pub session_id: String,
    pub device_id: String,
    pub file_id: String,
    pub relative_path: String,
    pub path_hash: String,
    pub checksum: String,
    pub size_bytes: i64,
    pub modified_at: i64,
    pub idempotency_key: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub accepted: bool,
    pub status: String,
    pub server_seq: Option<i64>,
    pub data: Option<T>,
    pub error: Option<ApiError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRequest {
    pub session_id: String,
    pub device_id: String,
    pub file_id: String,
    pub path_hash: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveConflictRequest {
    pub session_id: String,
    pub device_id: String,
    pub conflict_id: String,
    pub decision: String,
    pub preserve_alternative: bool,
    pub new_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResponse {
    pub accepted: bool,
    pub status: String,
    pub checksum: String,
    pub content: String,
    pub file_id: String,
}

pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn uuid_str() -> String {
    Uuid::new_v4().to_string()
}