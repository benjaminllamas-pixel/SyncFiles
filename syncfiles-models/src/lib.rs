use serde::{Deserialize, Serialize};
use chrono::Utc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub user_id: String,
    pub email: String,
    pub password_hash: String,
    pub display_name: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub device_id: String,
    pub user_id: String,
    pub platform: String,
    pub device_name: Option<String>,
    pub last_seen_at: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub struct DownloadRequest {
    pub session_id: String,
    pub device_id: String,
    pub file_id: String,
    pub path_hash: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameRequest {
    pub session_id: String,
    pub device_id: String,
    pub file_id: String,
    pub old_path: String,
    pub new_path: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveRequest {
    pub session_id: String,
    pub device_id: String,
    pub file_id: String,
    pub old_path: String,
    pub new_path: String,
    pub idempotency_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyRequest {
    pub session_id: String,
    pub device_id: String,
    pub file_id: String,
    pub source_path: String,
    pub destination_path: String,
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
pub struct RevokeDeviceRequest {
    pub session_id: String,
    pub device_id: String,
    pub target_device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResponse {
    pub accepted: bool,
    pub status: String,
    pub checksum: String,
    pub content: String,
    pub file_id: String,
    pub server_seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListItem {
    pub file_id: String,
    pub relative_path: String,
    pub path_hash: String,
    pub checksum: String,
    pub size_bytes: i64,
    pub modified_at: i64,
    pub synced_at: Option<i64>,
    pub status: String,
    pub deleted_at: Option<i64>,
    pub device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesListResponse {
    pub files: Vec<FileListItem>,
    pub total: i64,
    pub server_seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueResponse {
    pub entries: Vec<SyncQueueEntry>,
    pub total: i64,
    pub server_seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityResponse {
    pub events: Vec<AuditEntry>,
    pub total: i64,
    pub server_seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictsResponse {
    pub conflicts: Vec<Conflict>,
    pub total: i64,
    pub server_seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceWithSessions {
    pub device_id: String,
    pub platform: String,
    pub device_name: Option<String>,
    pub last_seen_at: i64,
    pub created_at: i64,
    pub active_sessions: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicesResponse {
    pub devices: Vec<DeviceWithSessions>,
    pub total: i64,
    pub server_seq: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub used_bytes: i64,
    pub file_count: i64,
    pub last_modified_at: Option<i64>,
    pub server_seq: i64,
}

pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn uuid_str() -> String {
    Uuid::new_v4().to_string()
}

pub fn compute_path_hash(path: &str) -> String {
    use sha2::{Sha256, Digest};
    let normalized = path.trim_start_matches('/');
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn compute_checksum(content: &[u8]) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

pub mod schema {
    pub const SERVER_INIT_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    user_id TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    display_name TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS devices (
    device_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    platform TEXT NOT NULL,
    device_name TEXT,
    last_seen_at INTEGER,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(user_id)
);

CREATE TABLE IF NOT EXISTS sessions (
    session_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    issued_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    revoked_at INTEGER,
    status TEXT NOT NULL DEFAULT 'active',
    FOREIGN KEY(user_id) REFERENCES users(user_id),
    FOREIGN KEY(device_id) REFERENCES devices(device_id)
);

CREATE TABLE IF NOT EXISTS files (
    file_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    path_hash TEXT NOT NULL,
    checksum TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    modified_at INTEGER NOT NULL,
    synced_at INTEGER,
    status TEXT NOT NULL DEFAULT 'pending',
    last_sync_version INTEGER DEFAULT 0,
    deleted_at INTEGER,
    content TEXT,
    UNIQUE(user_id, path_hash),
    FOREIGN KEY(user_id) REFERENCES users(user_id),
    FOREIGN KEY(device_id) REFERENCES devices(device_id)
);

CREATE TABLE IF NOT EXISTS sync_queue (
    queue_id TEXT PRIMARY KEY,
    file_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    attempts INTEGER NOT NULL DEFAULT 0,
    idempotency_key TEXT NOT NULL,
    payload_json TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_error TEXT
);

CREATE TABLE IF NOT EXISTS idempotency_keys (
    idempotency_key TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS conflicts (
    conflict_id TEXT PRIMARY KEY,
    file_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    device_local TEXT,
    device_remote TEXT,
    local_checksum TEXT,
    remote_checksum TEXT,
    conflict_type TEXT NOT NULL,
    strategy TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    resolved_at INTEGER,
    resolved_by TEXT
);

CREATE TABLE IF NOT EXISTS audit_log (
    audit_id TEXT PRIMARY KEY,
    user_id TEXT,
    device_id TEXT,
    event_name TEXT NOT NULL,
    event_type TEXT,
    payload_json TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_files_user_path ON files(user_id, path_hash);
CREATE INDEX IF NOT EXISTS idx_files_user_deleted ON files(user_id, deleted_at);
CREATE INDEX IF NOT EXISTS idx_sync_queue_user_status ON sync_queue(user_id, status, created_at);
CREATE INDEX IF NOT EXISTS idx_conflicts_user_file ON conflicts(file_id);
CREATE INDEX IF NOT EXISTS idx_audit_event_time ON audit_log(created_at, event_name);
CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_sessions_token_hash ON sessions(token_hash);
CREATE INDEX IF NOT EXISTS idx_idempotency_session ON idempotency_keys(session_id);
"#;

    pub const CLIENT_INIT_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS files (
    file_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    path_hash TEXT NOT NULL,
    checksum TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    modified_at INTEGER NOT NULL,
    synced_at INTEGER,
    status TEXT NOT NULL DEFAULT 'pending',
    last_sync_version INTEGER DEFAULT 0,
    deleted_at INTEGER,
    UNIQUE(user_id, path_hash)
);

CREATE TABLE IF NOT EXISTS sync_queue (
    queue_id TEXT PRIMARY KEY,
    file_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    attempts INTEGER NOT NULL DEFAULT 0,
    idempotency_key TEXT NOT NULL,
    payload_json TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_error TEXT,
    FOREIGN KEY(file_id) REFERENCES files(file_id)
);

CREATE TABLE IF NOT EXISTS sessions (
    session_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    token_hash TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    revoked_at INTEGER,
    status TEXT NOT NULL DEFAULT 'active'
);

CREATE TABLE IF NOT EXISTS conflicts (
    conflict_id TEXT PRIMARY KEY,
    file_id TEXT NOT NULL,
    device_local TEXT,
    device_remote TEXT,
    local_checksum TEXT,
    remote_checksum TEXT,
    strategy TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    resolved_at INTEGER,
    resolved_by TEXT
);

CREATE TABLE IF NOT EXISTS audit_log (
    audit_id TEXT PRIMARY KEY,
    user_id TEXT,
    device_id TEXT,
    event_name TEXT NOT NULL,
    payload_json TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS devices (
    device_id TEXT PRIMARY KEY,
    platform TEXT NOT NULL,
    device_name TEXT,
    last_seen_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_files_user_path ON files(user_id, path_hash);
CREATE INDEX IF NOT EXISTS idx_sync_queue_status ON sync_queue(status, created_at);
CREATE INDEX IF NOT EXISTS idx_conflicts_file ON conflicts(file_id);

CREATE TABLE IF NOT EXISTS metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_hash_ignores_leading_slash() {
        // Contrato client/server: "/docs/a.txt" y "docs/a.txt" son la misma ruta.
        assert_eq!(compute_path_hash("/docs/a.txt"), compute_path_hash("docs/a.txt"));
        assert_ne!(compute_path_hash("docs/a.txt"), compute_path_hash("docs/b.txt"));
    }

    #[test]
    fn path_hash_is_sha256_hex() {
        let h = compute_path_hash("docs/a.txt");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn checksum_matches_sha256_reference() {
        // Vector conocido de SHA-256 ("abc").
        assert_eq!(
            compute_checksum(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // Estable y sensible al contenido.
        assert_eq!(compute_checksum(b""), compute_checksum(b""));
        assert_ne!(compute_checksum(b"a"), compute_checksum(b"b"));
    }

    #[test]
    fn upload_request_roundtrip_preserves_fields() {
        let req = UploadRequest {
            session_id: "s1".into(),
            device_id: "d1".into(),
            file_id: "f1".into(),
            relative_path: "docs/a.txt".into(),
            path_hash: compute_path_hash("docs/a.txt"),
            checksum: compute_checksum(b"hola"),
            size_bytes: 4,
            modified_at: 42,
            idempotency_key: "idem-1".into(),
            content: "aG9sYQ==".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: UploadRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.relative_path, "docs/a.txt");
        assert_eq!(back.idempotency_key, "idem-1");
        assert_eq!(back.content, "aG9sYQ==");
        // El JSON usa exactamente los nombres del contrato REST.
        assert!(json.contains("\"relative_path\""));
        assert!(json.contains("\"idempotency_key\""));
    }

    #[test]
    fn diff_response_parses_server_payload() {
        // Payload real emitido por el servidor (snake_case, campos opcionales).
        let json = r#"{
            "changes": [
                {
                    "file_id": "f1",
                    "operation": "upload",
                    "path_hash": "abc",
                    "checksum": "def",
                    "modified_at": 7,
                    "device_id": "otro-device",
                    "relative_path": "docs/a.txt",
                    "content": null,
                    "size_bytes": 10
                }
            ],
            "server_seq": 5
        }"#;
        let diff: DiffResponse = serde_json::from_str(json).unwrap();
        assert_eq!(diff.changes.len(), 1);
        assert_eq!(diff.server_seq, 5);
        assert_eq!(diff.changes[0].operation, "upload");
        assert_eq!(diff.changes[0].relative_path.as_deref(), Some("docs/a.txt"));
        assert!(diff.changes[0].content.is_none());
    }

    #[test]
    fn api_error_parses_conflict_shape() {
        // El 409 de conflicto llega como ApiError embebido en ApiResponse.
        let json = r#"{
            "accepted": false,
            "status": "conflict",
            "server_seq": 12,
            "data": null,
            "error": {
                "code": "CONFLICT",
                "message": "Conflicto de checksum detectado",
                "retryable": false,
                "request_id": "req-1"
            }
        }"#;
        let resp: ApiResponse<()> = serde_json::from_str(json).unwrap();
        let err = resp.error.expect("error presente");
        assert_eq!(err.code, "CONFLICT");
        assert!(!err.retryable);
        assert!(!resp.accepted);
    }

    #[test]
    fn resolve_conflict_request_serializes_decision() {
        let req = ResolveConflictRequest {
            session_id: "s1".into(),
            device_id: "d1".into(),
            conflict_id: "c1".into(),
            decision: "keep_local".into(),
            preserve_alternative: true,
            new_name: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"decision\":\"keep_local\""));
        assert!(json.contains("\"preserve_alternative\":true"));
        let back: ResolveConflictRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.decision, "keep_local");
    }
}