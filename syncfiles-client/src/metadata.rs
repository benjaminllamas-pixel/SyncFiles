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

    pub fn get_file_by_id(&self, file_id: &str) -> Result<Option<FileEntry>> {
        let mut stmt = self.conn.prepare("SELECT file_id, user_id, device_id, relative_path, path_hash, checksum, size_bytes, modified_at, synced_at, status, last_sync_version, deleted_at FROM files WHERE file_id = ?1")?;
        let result = stmt.query_row(params![file_id], |row| {
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
        }).ok();
        Ok(result)
    }

    pub fn get_file_by_relative_path(&self, relative_path: &str) -> Result<Option<FileEntry>> {
        let mut stmt = self.conn.prepare("SELECT file_id, user_id, device_id, relative_path, path_hash, checksum, size_bytes, modified_at, synced_at, status, last_sync_version, deleted_at FROM files WHERE relative_path = ?1")?;
        let result = stmt.query_row(params![relative_path], |row| {
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
        }).ok();
        Ok(result)
    }

    pub fn update_file_path(&self, file_id: &str, relative_path: &str, path_hash: &str) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        self.conn.execute(
            "UPDATE files SET relative_path = ?1, path_hash = ?2, modified_at = ?3 WHERE file_id = ?4",
            params![relative_path, path_hash, now, file_id],
        )?;
        Ok(())
    }

    /// Marca por ruta relativa un archivo de vuelta a 'pending' (p.ej. cambio detectado en escaneo).
    pub fn update_file_status(&self, relative_path: &str, status: &str) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        self.conn.execute(
            "UPDATE files SET status = ?1, synced_at = NULL, modified_at = ?2 WHERE relative_path = ?3",
            params![status, now, relative_path],
        )?;
        Ok(())
    }

    pub fn rename_local_path(&self, old_path: &str, new_path: &str) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        let new_hash = compute_path_hash(new_path);
        self.conn.execute(
            "UPDATE files SET relative_path = ?1, path_hash = ?2, modified_at = ?3 WHERE relative_path = ?4",
            params![new_path, new_hash, now, old_path],
        )?;
        Ok(())
    }

    pub fn queue_op(&self, queue_id: &str, file_id: &str, operation: &str, idempotency_key: &str, payload: Option<String>) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        self.conn.execute(
            "INSERT OR REPLACE INTO sync_queue (queue_id, file_id, operation, status, attempts, idempotency_key, payload_json, created_at, updated_at, last_error) VALUES (?1, ?2, ?3, 'queued', 0, ?4, ?5, ?6, ?6, NULL)",
            params![queue_id, file_id, operation, idempotency_key, payload, now],
        )?;
        Ok(())
    }

    pub fn enqueue(&self, path: &str, operation: &str, payload: Option<String>) -> Result<()> {
        let path_hash = compute_path_hash(path);
        let file_id = uuid::Uuid::new_v4().to_string();
        let queue_id = uuid::Uuid::new_v4().to_string();
        let idempotency_key = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().timestamp_millis();
        self.conn.execute(
            "INSERT INTO files (file_id, user_id, device_id, relative_path, path_hash, checksum, size_bytes, modified_at, synced_at, status, last_sync_version, deleted_at) VALUES (?1, ?2, ?3, ?4, ?5, '', 0, ?6, NULL, 'pending', 0, NULL)",
            params![file_id, "local", "watcher", path, path_hash, now],
        )?;
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

    pub fn get_queue_entry(&self, queue_id: &str) -> Result<Option<SyncQueueEntry>> {
        let mut stmt = self.conn.prepare("SELECT queue_id, file_id, operation, status, attempts, idempotency_key, payload_json, created_at, updated_at, last_error FROM sync_queue WHERE queue_id = ?1")?;
        let result = stmt.query_row(params![queue_id], |row| {
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
        }).ok();
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
            "INSERT INTO conflicts (conflict_id, file_id, device_local, device_remote, local_checksum, remote_checksum, strategy, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'last_write_wins', ?7)",
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
            "INSERT INTO audit_log (audit_id, user_id, device_id, event_name, payload_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![audit_id, "local", &self.config.device_id, event_name, payload, now],
        )?;
        Ok(())
    }

    pub fn hash_file(&self, path: &Path) -> Result<String> {
        let content = std::fs::read(path)?;
        Ok(compute_checksum(&content))
    }

    pub fn hash_file_at(path: &Path) -> Result<String> {
        let content = std::fs::read(path)?;
        Ok(compute_checksum(&content))
    }

    pub fn path_hash(&self, path: &str) -> String {
        compute_path_hash(path)
    }

    pub fn get_last_server_seq(&self) -> Result<i64> {
        let mut stmt = self.conn.prepare("SELECT CAST(value AS INTEGER) FROM metadata WHERE key = 'last_server_seq'")?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(root: &Path) -> Config {
        Config {
            server_url: "http://127.0.0.1:8080".into(),
            email: "test@syncfiles.local".into(),
            password: "secret".into(),
            device_id: "device-test".into(),
            sync_root: root.to_path_buf(),
            polling_interval_secs: 30,
        }
    }

    fn store_in_dir(dir_name: &str) -> (MetadataStore, std::path::PathBuf) {
        let tmp = std::env::temp_dir().join(format!(
            "sf-client-test-{}-{}",
            dir_name,
            std::process::id()
        ));
        std::fs::remove_dir_all(&tmp).ok();
        std::fs::create_dir_all(&tmp).unwrap();
        // Config::data_dir() lee SF_DATA_DIR; se setea por test (tests corren
        // en el mismo proceso, así que se restaura el valor al final del test).
        std::env::set_var("SF_DATA_DIR", &tmp);
        let store = MetadataStore::new(test_config(&tmp.join("sync"))).unwrap();
        (store, tmp)
    }

    fn cleanup(tmp: &std::path::PathBuf) {
        std::env::remove_var("SF_DATA_DIR");
        std::fs::remove_dir_all(tmp).ok();
    }

    fn sample_file(path: &str, status: &str) -> FileEntry {
        FileEntry {
            file_id: uuid::Uuid::new_v4().to_string(),
            user_id: "user-1".into(),
            device_id: "device-test".into(),
            relative_path: path.into(),
            path_hash: compute_path_hash(path),
            checksum: compute_checksum(b"contenido"),
            size_bytes: 9,
            modified_at: now_ms(),
            synced_at: Some(now_ms()),
            status: status.into(),
            last_sync_version: 0,
            deleted_at: None,
            content: None,
        }
    }

    #[test]
    fn session_lifecycle_save_get_revoke() {
        let (store, tmp) = store_in_dir("session");
        let session = Session {
            session_id: "sess-1".into(),
            user_id: "user-1".into(),
            device_id: "device-test".into(),
            expires_at: now_ms() + 3_600_000,
            token_hash: "hash".into(),
        };
        store.save_session(&session).unwrap();

        let active = store.get_active_session().unwrap().expect("sesión activa");
        assert_eq!(active.session_id, "sess-1");
        assert!(!active.is_expired());

        store.revoke_session().unwrap();
        assert!(store.get_active_session().unwrap().is_none());
        cleanup(&tmp);
    }

    #[test]
    fn expired_session_is_not_active() {
        let (store, tmp) = store_in_dir("session-exp");
        let session = Session {
            session_id: "sess-old".into(),
            user_id: "user-1".into(),
            device_id: "device-test".into(),
            // Expiró hace 1 ms: no debe considerarse activa.
            expires_at: now_ms() - 1,
            token_hash: "hash".into(),
        };
        store.save_session(&session).unwrap();
        assert!(store.get_active_session().unwrap().is_none());
        cleanup(&tmp);
    }

    #[test]
    fn file_upsert_and_lookup_by_path() {
        let (store, tmp) = store_in_dir("files");
        let entry = sample_file("docs/a.txt", "synced");
        store.upsert_file(&entry).unwrap();

        let found = store
            .get_file_by_relative_path("docs/a.txt")
            .unwrap()
            .expect("encontrado");
        assert_eq!(found.file_id, entry.file_id);
        assert_eq!(found.status, "synced");

        let by_id = store.get_file_by_id(&entry.file_id).unwrap().expect("por id");
        assert_eq!(by_id.relative_path, "docs/a.txt");

        assert!(store.get_file_by_relative_path("no-existe.txt").unwrap().is_none());
        cleanup(&tmp);
    }

    #[test]
    fn files_by_status_filter() {
        let (store, tmp) = store_in_dir("status");
        store.upsert_file(&sample_file("a.txt", "pending")).unwrap();
        store.upsert_file(&sample_file("b.txt", "synced")).unwrap();
        store.upsert_file(&sample_file("c.txt", "pending")).unwrap();

        let pending = store.get_files_by_status("pending").unwrap();
        assert_eq!(pending.len(), 2);
        assert!(pending.iter().all(|f| f.status == "pending"));
        cleanup(&tmp);
    }

    #[test]
    fn rename_updates_path_and_hash() {
        let (store, tmp) = store_in_dir("rename");
        store.upsert_file(&sample_file("docs/viejo.txt", "synced")).unwrap();

        store.rename_local_path("docs/viejo.txt", "docs/nuevo.txt").unwrap();
        let renamed = store
            .get_file_by_relative_path("docs/nuevo.txt")
            .unwrap()
            .expect("renombrado");
        assert_eq!(renamed.path_hash, compute_path_hash("docs/nuevo.txt"));
        assert!(store.get_file_by_relative_path("docs/viejo.txt").unwrap().is_none());
        cleanup(&tmp);
    }

    #[test]
    fn update_file_status_resets_synced_at() {
        let (store, tmp) = store_in_dir("status-reset");
        let mut entry = sample_file("a.txt", "synced");
        entry.synced_at = Some(1234);
        store.upsert_file(&entry).unwrap();

        store.update_file_status("a.txt", "pending").unwrap();
        let updated = store.get_file_by_relative_path("a.txt").unwrap().unwrap();
        assert_eq!(updated.status, "pending");
        // synced_at se limpia: el archivo vuelve a la cola de pendientes.
        assert!(updated.synced_at.is_none());
        cleanup(&tmp);
    }

    #[test]
    fn queue_op_persists_with_payload() {
        let (store, tmp) = store_in_dir("queue");
        let file = sample_file("q.txt", "pending");
        store.upsert_file(&file).unwrap();

        let queue_id = uuid::Uuid::new_v4().to_string();
        store
            .queue_op(&queue_id, &file.file_id, "upload", "idem-1", Some("{\"k\":1}".into()))
            .unwrap();

        let entry = store.get_queue_entry(&queue_id).unwrap().unwrap();
        assert_eq!(entry.status, "queued");
        assert_eq!(entry.attempts, 0);
        assert_eq!(entry.payload_json.as_deref(), Some("{\"k\":1}"));
        assert_eq!(entry.last_error, None);
        cleanup(&tmp);
    }

    #[test]
    fn queue_retry_cycle_survives_restart() {
        // Escenario 5.4: operación 'retry' persistida que debe reanudarse tras
        // reiniciar el cliente (la DB local se reabre en una instancia nueva).
        let (store, tmp) = store_in_dir("queue-retry");

        let file = sample_file("persist.txt", "pending");
        store.upsert_file(&file).unwrap();
        let queue_id = uuid::Uuid::new_v4().to_string();
        store
            .queue_op(&queue_id, &file.file_id, "upload", "idem-2", Some("{\"k\":2}".into()))
            .unwrap();
        // El intento falla: pasa a 'retry' con error, intentos incrementados.
        store.update_queue_status(&queue_id, "retry", Some("HTTP 400")).unwrap();
        store.increment_attempts(&queue_id).unwrap();

        // "Reinicio": nueva MetadataStore sobre el mismo SF_DATA_DIR.
        let reopened = MetadataStore::new(test_config(&tmp.join("sync"))).unwrap();
        let queued = reopened.get_queued_ops().unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].status, "retry");
        assert_eq!(queued[0].attempts, 1);
        assert_eq!(queued[0].last_error.as_deref(), Some("HTTP 400"));

        // done ya no debe volver a listarse por get_queued_ops.
        reopened.update_queue_status(&queue_id, "done", None).unwrap();
        assert!(reopened.get_queued_ops().unwrap().is_empty());
        cleanup(&tmp);
    }

    #[test]
    fn enqueue_creates_pending_file_and_queue_row() {
        let (store, tmp) = store_in_dir("enqueue");
        store.enqueue("docs/nuevo.txt", "upload", None).unwrap();

        let file = store
            .get_file_by_relative_path("docs/nuevo.txt")
            .unwrap()
            .expect("fila files creada");
        assert_eq!(file.status, "pending");
        assert_eq!(file.checksum, ""); // el checksum lo calcula push_local

        let queued = store.get_queued_ops().unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].operation, "upload");
        assert_eq!(queued[0].file_id, file.file_id);
        cleanup(&tmp);
    }

    #[test]
    fn conflict_mark_and_resolve() {
        let (store, tmp) = store_in_dir("conflict");
        let file = sample_file("conf.txt", "synced");
        store.upsert_file(&file).unwrap();

        store
            .mark_conflict("c-1", &file.file_id, "local-abc", "remote-def")
            .unwrap();
        let conflicts = store.get_conflicts().unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].conflict_id, "c-1");
        assert_eq!(conflicts[0].local_checksum.as_deref(), Some("local-abc"));

        store.resolve_conflict("c-1", "user-1").unwrap();
        // Solo listamos conflictos sin resolver: desaparece de la vista.
        assert!(store.get_conflicts().unwrap().is_empty());
        cleanup(&tmp);
    }

    #[test]
    fn last_server_seq_persists() {
        let (store, tmp) = store_in_dir("seq");
        assert_eq!(store.get_last_server_seq().unwrap(), 0);
        store.set_last_server_seq(41).unwrap();
        assert_eq!(store.get_last_server_seq().unwrap(), 41);

        let reopened = MetadataStore::new(test_config(&tmp.join("sync"))).unwrap();
        assert_eq!(reopened.get_last_server_seq().unwrap(), 41);
        cleanup(&tmp);
    }

    #[test]
    fn hash_file_at_matches_compute_checksum() {
        let (store, tmp) = store_in_dir("hash");
        let file_path = tmp.join("sample.bin");
        std::fs::write(&file_path, b"hola").unwrap();
        assert_eq!(
            MetadataStore::hash_file_at(&file_path).unwrap(),
            compute_checksum(b"hola")
        );
        assert_eq!(store.hash_file(&file_path).unwrap(), compute_checksum(b"hola"));
        cleanup(&tmp);
    }

    #[test]
    fn audit_event_persists() {
        let (store, tmp) = store_in_dir("audit");
        store.audit_event("test.event", "{\"x\":1}").unwrap();
        let log = store.get_audit_log().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].event_name, "test.event");
        assert_eq!(log[0].payload_json.as_deref(), Some("{\"x\":1}"));
        cleanup(&tmp);
    }
}