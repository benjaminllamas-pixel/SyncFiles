pub use syncfiles_models::*;
use sqlx::FromRow;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserRow {
    pub user_id: String,
    pub email: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeviceRow {
    pub device_id: String,
    pub user_id: String,
    pub platform: String,
    pub device_name: Option<String>,
    pub last_seen_at: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SessionRow {
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
pub struct FileEntryRow {
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
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SyncQueueEntryRow {
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
pub struct ConflictRow {
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
pub struct AuditEntryRow {
    pub audit_id: String,
    pub user_id: Option<String>,
    pub device_id: Option<String>,
    pub event_name: String,
    pub event_type: Option<String>,
    pub payload_json: Option<String>,
    pub created_at: i64,
}