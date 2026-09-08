use anyhow::Result;
use sqlx::SqlitePool;
use crate::models::*;
use syncfiles_models::{User, Session, FileEntry, SyncQueueEntry, Conflict, AuditEntry};

fn now_ms_local() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub async fn get_file_by_id_for_user(pool: &SqlitePool, user_id: &str, file_id: &str) -> Result<Option<FileEntry>> {
    let row = sqlx::query_as::<_, FileEntryRow>(
        "SELECT file_id, user_id, device_id, relative_path, path_hash, checksum, size_bytes, modified_at, synced_at, status, last_sync_version, deleted_at, content FROM files WHERE user_id = ? AND file_id = ?"
    )
    .bind(user_id)
    .bind(file_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| FileEntry {
        file_id: r.file_id,
        user_id: r.user_id,
        device_id: r.device_id,
        relative_path: r.relative_path,
        path_hash: r.path_hash,
        checksum: r.checksum,
        size_bytes: r.size_bytes,
        modified_at: r.modified_at,
        synced_at: r.synced_at,
        status: r.status,
        last_sync_version: r.last_sync_version,
        deleted_at: r.deleted_at,
        content: r.content,
    }))
}

pub async fn init_db(pool: &SqlitePool) -> Result<()> {
    let sql = crate::models::schema::SERVER_INIT_SQL;
    for statement in sql.split(';') {
        let stmt = statement.trim();
        if !stmt.is_empty() {
            let _ = sqlx::query(stmt).execute(pool).await;
        }
    }
    Ok(())
}

pub async fn get_user_by_email(pool: &SqlitePool, email: &str) -> Result<Option<User>> {
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT user_id, email, password_hash, display_name, created_at, updated_at FROM users WHERE email = ?"
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| User {
        user_id: r.user_id,
        email: r.email,
        password_hash: r.password_hash,
        display_name: r.display_name,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }))
}

pub async fn get_user_by_id(pool: &SqlitePool, user_id: &str) -> Result<Option<User>> {
    let row = sqlx::query_as::<_, UserRow>(
        "SELECT user_id, email, password_hash, display_name, created_at, updated_at FROM users WHERE user_id = ?"
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| User {
        user_id: r.user_id,
        email: r.email,
        password_hash: r.password_hash,
        display_name: r.display_name,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }))
}

pub async fn create_user(pool: &SqlitePool, email: &str, password_hash: &str, user_id: &str) -> Result<User> {
    let now = now_ms_local();
    let user = User {
        user_id: user_id.to_string(),
        email: email.to_string(),
        password_hash: password_hash.to_string(),
        display_name: None,
        created_at: now,
        updated_at: now,
    };
    sqlx::query(
        "INSERT INTO users (user_id, email, password_hash, display_name, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(&user.user_id)
    .bind(&user.email)
    .bind(&user.password_hash)
    .bind(&user.display_name)
    .bind(user.created_at)
    .bind(user.updated_at)
    .execute(pool)
    .await?;
    Ok(user)
}

pub async fn create_session(pool: &SqlitePool, session: &Session) -> Result<()> {
    sqlx::query(
        "INSERT INTO sessions (session_id, user_id, device_id, token_hash, issued_at, expires_at, revoked_at, status) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&session.session_id)
    .bind(&session.user_id)
    .bind(&session.device_id)
    .bind(&session.token_hash)
    .bind(session.issued_at)
    .bind(session.expires_at)
    .bind(session.revoked_at)
    .bind(&session.status)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_active_session(pool: &SqlitePool, token_hash: &str) -> Result<Option<Session>> {
    let now = now_ms_local();
    let row = sqlx::query_as::<_, SessionRow>(
        "SELECT session_id, user_id, device_id, token_hash, issued_at, expires_at, revoked_at, status FROM sessions WHERE token_hash = ? AND status = 'active' AND revoked_at IS NULL AND expires_at > ?"
    )
    .bind(token_hash)
    .bind(now)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| Session {
        session_id: r.session_id,
        user_id: r.user_id,
        device_id: r.device_id,
        token_hash: r.token_hash,
        issued_at: r.issued_at,
        expires_at: r.expires_at,
        revoked_at: r.revoked_at,
        status: r.status,
    }))
}

pub async fn revoke_session(pool: &SqlitePool, session_id: &str) -> Result<()> {
    sqlx::query("UPDATE sessions SET status = 'revoked', revoked_at = ? WHERE session_id = ?")
        .bind(now_ms_local())
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn upsert_file(pool: &SqlitePool, file: &FileEntry) -> Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO files (file_id, user_id, device_id, relative_path, path_hash, checksum, size_bytes, modified_at, synced_at, status, last_sync_version, deleted_at, content) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&file.file_id)
    .bind(&file.user_id)
    .bind(&file.device_id)
    .bind(&file.relative_path)
    .bind(&file.path_hash)
    .bind(&file.checksum)
    .bind(file.size_bytes)
    .bind(file.modified_at)
    .bind(file.synced_at)
    .bind(&file.status)
    .bind(file.last_sync_version)
    .bind(file.deleted_at)
    .bind(&file.content)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_files_modified_since(pool: &SqlitePool, user_id: &str, since: i64) -> Result<Vec<FileEntry>> {
    let rows = sqlx::query_as::<_, FileEntryRow>(
        "SELECT file_id, user_id, device_id, relative_path, path_hash, checksum, size_bytes, modified_at, synced_at, status, last_sync_version, deleted_at, content FROM files WHERE user_id = ? AND (modified_at > ? OR deleted_at > ?)"
    )
    .bind(user_id)
    .bind(since)
    .bind(since)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| FileEntry {
        file_id: r.file_id,
        user_id: r.user_id,
        device_id: r.device_id,
        relative_path: r.relative_path,
        path_hash: r.path_hash,
        checksum: r.checksum,
        size_bytes: r.size_bytes,
        modified_at: r.modified_at,
        synced_at: r.synced_at,
        status: r.status,
        last_sync_version: r.last_sync_version,
        deleted_at: r.deleted_at,
        content: r.content,
    }).collect())
}

pub async fn get_file_by_id(pool: &SqlitePool, file_id: &str) -> Result<Option<FileEntry>> {
    let row = sqlx::query_as::<_, FileEntryRow>(
        "SELECT file_id, user_id, device_id, relative_path, path_hash, checksum, size_bytes, modified_at, synced_at, status, last_sync_version, deleted_at, content FROM files WHERE file_id = ?"
    )
    .bind(file_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| FileEntry {
        file_id: r.file_id,
        user_id: r.user_id,
        device_id: r.device_id,
        relative_path: r.relative_path,
        path_hash: r.path_hash,
        checksum: r.checksum,
        size_bytes: r.size_bytes,
        modified_at: r.modified_at,
        synced_at: r.synced_at,
        status: r.status,
        last_sync_version: r.last_sync_version,
        deleted_at: r.deleted_at,
        content: r.content,
    }))
}

pub async fn get_files_by_path_hash(pool: &SqlitePool, user_id: &str, path_hash: &str) -> Result<Option<FileEntry>> {
    let row = sqlx::query_as::<_, FileEntryRow>(
        "SELECT file_id, user_id, device_id, relative_path, path_hash, checksum, size_bytes, modified_at, synced_at, status, last_sync_version, deleted_at, content FROM files WHERE user_id = ? AND path_hash = ?"
    )
    .bind(user_id)
    .bind(path_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| FileEntry {
        file_id: r.file_id,
        user_id: r.user_id,
        device_id: r.device_id,
        relative_path: r.relative_path,
        path_hash: r.path_hash,
        checksum: r.checksum,
        size_bytes: r.size_bytes,
        modified_at: r.modified_at,
        synced_at: r.synced_at,
        status: r.status,
        last_sync_version: r.last_sync_version,
        deleted_at: r.deleted_at,
        content: r.content,
    }))
}

pub async fn queue_op(pool: &SqlitePool, op: &SyncQueueEntry) -> Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO sync_queue (queue_id, file_id, user_id, device_id, operation, status, attempts, idempotency_key, payload_json, created_at, updated_at, last_error) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&op.queue_id)
    .bind(&op.file_id)
    .bind(&op.user_id)
    .bind(&op.device_id)
    .bind(&op.operation)
    .bind(&op.status)
    .bind(op.attempts)
    .bind(&op.idempotency_key)
    .bind(&op.payload_json)
    .bind(op.created_at)
    .bind(op.updated_at)
    .bind(&op.last_error)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_queued_ops(pool: &SqlitePool, user_id: &str) -> Result<Vec<SyncQueueEntry>> {
    let rows = sqlx::query_as::<_, SyncQueueEntryRow>(
        "SELECT queue_id, file_id, user_id, device_id, operation, status, attempts, idempotency_key, payload_json, created_at, updated_at, last_error FROM sync_queue WHERE user_id = ? AND status IN ('queued', 'retry') ORDER BY created_at LIMIT 50"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| SyncQueueEntry {
        queue_id: r.queue_id,
        file_id: r.file_id,
        user_id: r.user_id,
        device_id: r.device_id,
        operation: r.operation,
        status: r.status,
        attempts: r.attempts,
        idempotency_key: r.idempotency_key,
        payload_json: r.payload_json,
        created_at: r.created_at,
        updated_at: r.updated_at,
        last_error: r.last_error,
    }).collect())
}

pub async fn update_queue_status(pool: &SqlitePool, queue_id: &str, status: &str, error: Option<&str>) -> Result<()> {
    sqlx::query("UPDATE sync_queue SET status = ?, updated_at = ?, last_error = ? WHERE queue_id = ?")
        .bind(status)
        .bind(now_ms_local())
        .bind(error)
        .bind(queue_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn check_idempotency(pool: &SqlitePool, idempotency_key: &str) -> Result<bool> {
    if idempotency_key.is_empty() {
        return Ok(false);
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM idempotency_keys WHERE idempotency_key = ?"
    )
    .bind(idempotency_key)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn record_idempotency(pool: &SqlitePool, idempotency_key: &str, session_id: &str) -> Result<()> {
    if idempotency_key.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "INSERT OR IGNORE INTO idempotency_keys (idempotency_key, session_id, created_at) VALUES (?, ?, ?)"
    )
    .bind(idempotency_key)
    .bind(session_id)
    .bind(now_ms_local())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_conflict(pool: &SqlitePool, conflict: &Conflict) -> Result<()> {
    sqlx::query(
        "INSERT INTO conflicts (conflict_id, file_id, user_id, device_local, device_remote, local_checksum, remote_checksum, conflict_type, strategy, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&conflict.conflict_id)
    .bind(&conflict.file_id)
    .bind(&conflict.user_id)
    .bind(&conflict.device_local)
    .bind(&conflict.device_remote)
    .bind(&conflict.local_checksum)
    .bind(&conflict.remote_checksum)
    .bind(&conflict.conflict_type)
    .bind(&conflict.strategy)
    .bind(conflict.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_conflicts_for_file(pool: &SqlitePool, file_id: &str) -> Result<Option<Conflict>> {
    let row = sqlx::query_as::<_, ConflictRow>(
        "SELECT conflict_id, file_id, user_id, device_local, device_remote, local_checksum, remote_checksum, conflict_type, strategy, created_at, resolved_at, resolved_by FROM conflicts WHERE file_id = ?"
    )
    .bind(file_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| Conflict {
        conflict_id: r.conflict_id,
        file_id: r.file_id,
        user_id: r.user_id,
        device_local: r.device_local,
        device_remote: r.device_remote,
        local_checksum: r.local_checksum,
        remote_checksum: r.remote_checksum,
        conflict_type: r.conflict_type,
        strategy: r.strategy,
        created_at: r.created_at,
        resolved_at: r.resolved_at,
        resolved_by: r.resolved_by,
    }))
}

pub async fn get_conflict_by_id(pool: &SqlitePool, conflict_id: &str) -> Result<Option<Conflict>> {
    let row = sqlx::query_as::<_, ConflictRow>(
        "SELECT conflict_id, file_id, user_id, device_local, device_remote, local_checksum, remote_checksum, conflict_type, strategy, created_at, resolved_at, resolved_by FROM conflicts WHERE conflict_id = ?"
    )
    .bind(conflict_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| Conflict {
        conflict_id: r.conflict_id,
        file_id: r.file_id,
        user_id: r.user_id,
        device_local: r.device_local,
        device_remote: r.device_remote,
        local_checksum: r.local_checksum,
        remote_checksum: r.remote_checksum,
        conflict_type: r.conflict_type,
        strategy: r.strategy,
        created_at: r.created_at,
        resolved_at: r.resolved_at,
        resolved_by: r.resolved_by,
    }))
}

pub async fn resolve_conflict(pool: &SqlitePool, conflict_id: &str, resolved_by: &str) -> Result<()> {
    sqlx::query("UPDATE conflicts SET resolved_at = ?, resolved_by = ? WHERE conflict_id = ?")
        .bind(now_ms_local())
        .bind(resolved_by)
        .bind(conflict_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn audit_event(pool: &SqlitePool, event: &AuditEntry) -> Result<()> {
    sqlx::query(
        "INSERT INTO audit_log (audit_id, user_id, device_id, event_name, event_type, payload_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&event.audit_id)
    .bind(&event.user_id)
    .bind(&event.device_id)
    .bind(&event.event_name)
    .bind(&event.event_type)
    .bind(&event.payload_json)
    .bind(event.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_last_server_seq(pool: &SqlitePool) -> Result<i64> {
    let result = sqlx::query_scalar::<_, String>(
        "SELECT COALESCE((SELECT value FROM metadata WHERE key = 'last_server_seq'), '0')"
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or("0".to_string());
    Ok(result.parse().unwrap_or(0))
}

pub async fn set_last_server_seq(pool: &SqlitePool, seq: i64) -> Result<()> {
    sqlx::query("INSERT OR REPLACE INTO metadata (key, value) VALUES ('last_server_seq', ?)")
        .bind(seq.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_file(pool: &SqlitePool, file_id: &str) -> Result<()> {
    sqlx::query("UPDATE files SET status = 'deleted', deleted_at = ? WHERE file_id = ?")
        .bind(now_ms_local())
        .bind(file_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn rename_file(pool: &SqlitePool, file_id: &str, new_relative_path: &str) -> Result<()> {
    let path_hash = syncfiles_models::compute_path_hash(new_relative_path);
    sqlx::query("UPDATE files SET relative_path = ?, path_hash = ? WHERE file_id = ?")
        .bind(new_relative_path)
        .bind(path_hash)
        .bind(file_id)
        .execute(pool)
        .await?;
    Ok(())
}