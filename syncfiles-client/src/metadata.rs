use anyhow::Result;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;
use chrono::Utc;

use crate::config::Config;
use syncfiles_models::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncQueueEntry {
    pub queue_id: String,
    pub file_id: String,
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
pub struct SessionRecord {
    pub session_id: String,
    pub user_id: String,
    pub device_id: String,
    pub token_hash: String,
    pub expires_at: i64,
    pub revoked_at: Option<i64>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub conflict_id: String,
    pub file_id: String,
    pub local_checksum: Option<String>,
    pub remote_checksum: Option<String>,
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
    pub payload_json: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRecord {
    pub device_id: String,
    pub platform: String,
    pub device_name: Option<String>,
    pub last_seen_at: i64,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub session_id: String,
    pub user_id: String,
    pub device_id: String,
    pub expires_at: i64,
    pub token_hash: String,
}

pub struct MetadataStore {
    conn: Connection,
    pub config: Config,
}

impl MetadataStore {
    pub fn new(cfg: Config) -> Result<Self> {
        let db_path = Config::data_dir().join("syncfiles.db");
        let conn = Connection::open(&db_path)?;
        conn.execute_batch(syncfiles_models::schema::CLIENT_INIT_SQL)?;
        info!("Base de datos local inicializada: {}", db_path.display());
        Ok(Self { conn, config: cfg })
    }

    pub fn init() -> Result<Self> {
        let config = Config::load()?;
        Self::new(config)
    }

    pub fn save_session(&self, session: &Session) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO sessions (session_id, user_id, device_id, token_hash, expires_at, revoked_at, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![session.session_id, session.user_id, session.device_id, session.token_hash, session.expires_at, None::<i64>, "active"],
        )?;
        info!("Sesión guardada: {}", session.session_id);
        Ok(())
    }

    pub fn get_active_session(&self) -> Result<Option<SessionRecord>> {
        let mut stmt = self.conn.prepare("SELECT session_id, user_id, device_id, token_hash, expires_at, revoked_at, status FROM sessions WHERE status = 'active' AND revoked_at IS NULL AND expires_at > ?1")?;
        let now = Utc::now().timestamp_millis();
        let result = stmt.query_row(params![now], |row| {
            Ok(SessionRecord {
                session_id: row.get(0)?,
                user_id: row.get(1)?,
                device_id: row.get(2)?,
                token_hash: row.get(3)?,
                expires_at: row.get(4)?,
                revoked_at: row.get(5)?,
                status: row.get(6)?,
            })
        }).ok();
        Ok(result)
    }

    pub fn revoke_session(&self) -> Result<()> {
        self.conn.execute("UPDATE sessions SET status = 'revoked', revoked_at = ?1 WHERE status = 'active'", params![Utc::now().timestamp_millis()])?;
        Ok(())
    }

    pub fn upsert_file(&self, entry: &FileEntry) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO files (file_id, user_id, device_id, relative_path, path_hash, checksum, size_bytes, modified_at, synced_at, status, last_sync_version, deleted_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![entry.file_id, entry.user_id, entry.device_id, entry.relative_path, entry.path_hash, entry.checksum, entry.size_bytes, entry.modified_at, entry.synced_at, entry.status, entry.last_sync_version, entry.deleted_at],
        )?;
        Ok(())
    }

    pub fn get_files_by_status(&self, status: &str) -> Result<Vec<FileEntry>> {
        let mut stmt = self.conn.prepare("SELECT file_id, user_id, device_id, relative_path, path_hash, checksum, size_bytes, modified_at, synced_at, status, last_sync_version, deleted_at FROM files WHERE status = ?1")?;
        let rows = stmt.query_map(params![status], |row| {
            Ok(FileEntry {
                file_id: row.get(0)?,
                user_id: row.get(1)?,
                device_id: row.get(2)?,
                relative_path: row.get(3)?,
                path_hash: row.get(4)?,
                checksum: row.get(5)?,
                size_bytes: row.get(6)?,
                modified_at: row.get(7)?,
                synced_at: row.get(8)?,
                status: row.get(9)?,
                last_sync_version: row.get(10)?,
                deleted_at: row.get(11)?,
                content: None,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn queue_op(&self, queue_id: &str, file_id: &str, operation: &str, idempotency_key: &str, payload: Option<String>) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        self.conn.execute(
            "INSERT OR REPLACE INTO sync_queue (queue_id, file_id, operation, status, attempts, idempotency_key, payload_json, created_at, updated_at, last_error) VALUES (?1, ?2, ?3, 'queued', 0, ?4, ?5, ?6, ?6, NULL)",
            params![queue_id, file_id, operation, idempotency_key, payload, now],
        )?;
        Ok(())
    }

    pub fn get_queued_ops(&self) -> Result<Vec<SyncQueueEntry>> {
        let mut stmt = self.conn.prepare("SELECT queue_id, file_id, operation, status, attempts, idempotency_key, payload_json, created_at, updated_at, last_error FROM sync_queue WHERE status IN ('queued', 'retry') ORDER BY created_at LIMIT 50")?;
        let rows = stmt.query_map([], |row| {
            Ok(SyncQueueEntry {
                queue_id: row.get(0)?,
                file_id: row.get(1)?,
                operation: row.get(2)?,
                status: row.get(3)?,
                attempts: row.get(4)?,
                idempotency_key: row.get(5)?,
                payload_json: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
                last_error: row.get(9)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn update_queue_status(&self, queue_id: &str, status: &str, error: Option<&str>) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        self.conn.execute(
            "UPDATE sync_queue SET status = ?1, updated_at = ?2, last_error = ?3 WHERE queue_id = ?4",
            params![status, now, error, queue_id],
        )?;
        Ok(())
    }

    pub fn increment_attempts(&self, queue_id: &str) -> Result<()> {
        self.conn.execute("UPDATE sync_queue SET attempts = attempts + 1, updated_at = ?1 WHERE queue_id = ?2", params![Utc::now().timestamp_millis(), queue_id])?;
        Ok(())
    }

    pub fn mark_conflict(&self, conflict_id: &str, file_id: &str, local_checksum: &str, remote_checksum: &str) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        self.conn.execute(
            "INSERT INTO conflicts (conflict_id, file_id, device_local, device_remote, local_checksum, remote_checksum, strategy, created_at) VALUES (?1, ?2, ?, ?, ?3, ?4, 'last_write_wins', ?5)",
            params![conflict_id, file_id, "unknown", "unknown", local_checksum, remote_checksum, now],
        )?;
        Ok(())
    }

    pub fn resolve_conflict(&self, conflict_id: &str, resolved_by: &str) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        self.conn.execute(
            "UPDATE conflicts SET resolved_at = ?1, resolved_by = ?2 WHERE conflict_id = ?3",
            params![now, resolved_by, conflict_id],
        )?;
        Ok(())
    }

    pub fn audit_event(&self, event_name: &str, payload: &str) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        let audit_id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO audit_log (audit_id, user_id, device_id, event_name, payload_json, created_at) VALUES (?1, ?, ?, ?2, ?3, ?4)",
            params![audit_id, "local", &self.config.device_id, event_name, payload, now],
        )?;
        Ok(())
    }

    pub fn hash_file(&self, path: &Path) -> Result<String> {
        let content = std::fs::read(path)?;
        Ok(compute_checksum(&content))
    }

    pub fn path_hash(&self, path: &str) -> String {
        compute_path_hash(path)
    }

    pub fn get_last_server_seq(&self) -> Result<i64> {
        let mut stmt = self.conn.prepare("SELECT value FROM metadata WHERE key = 'last_server_seq'")?;
        let result = stmt.query_row([], |row| row.get::<_, i64>(0)).unwrap_or(0);
        Ok(result)
    }

    pub fn set_last_server_seq(&self, seq: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO metadata (key, value) VALUES ('last_server_seq', ?1)",
            params![seq],
        )?;
        Ok(())
    }

    pub fn get_conflicts(&self) -> Result<Vec<Conflict>> {
        let mut stmt = self.conn.prepare("SELECT conflict_id, file_id, local_checksum, remote_checksum, strategy, created_at, resolved_at, resolved_by FROM conflicts WHERE resolved_at IS NULL ORDER BY created_at")?;
        let rows = stmt.query_map([], |row| {
            Ok(Conflict {
                conflict_id: row.get(0)?,
                file_id: row.get(1)?,
                local_checksum: row.get(2)?,
                remote_checksum: row.get(3)?,
                strategy: row.get(4)?,
                created_at: row.get(5)?,
                resolved_at: row.get(6)?,
                resolved_by: row.get(7)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn get_audit_log(&self) -> Result<Vec<AuditEntry>> {
        let mut stmt = self.conn.prepare("SELECT audit_id, user_id, device_id, event_name, payload_json, created_at FROM audit_log ORDER BY created_at DESC LIMIT 200")?;
        let rows = stmt.query_map([], |row| {
            Ok(AuditEntry {
                audit_id: row.get(0)?,
                user_id: row.get(1)?,
                device_id: row.get(2)?,
                event_name: row.get(3)?,
                payload_json: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn get_devices(&self) -> Result<Vec<DeviceRecord>> {
        let mut stmt = self.conn.prepare("SELECT device_id, platform, device_name, last_seen_at FROM devices")?;
        let rows = stmt.query_map([], |row| {
            Ok(DeviceRecord {
                device_id: row.get(0)?,
                platform: row.get(1)?,
                device_name: row.get(2)?,
                last_seen_at: row.get(3)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    pub fn upsert_device(&self, device: &DeviceRecord) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO devices (device_id, platform, device_name, last_seen_at) VALUES (?1, ?2, ?3, ?4)",
            params![device.device_id, device.platform, device.device_name, device.last_seen_at],
        )?;
        Ok(())
    }
}

impl SessionRecord {
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp_millis();
        now >= self.expires_at
    }
}