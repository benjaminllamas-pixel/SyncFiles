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
CREATE INDEX IF NOT EXISTS idx_sync_queue_user_status ON sync_queue(user_id, status, created_at);
CREATE INDEX IF NOT EXISTS idx_conflicts_user_file ON conflicts(file_id);
CREATE INDEX IF NOT EXISTS idx_audit_event_time ON audit_log(created_at, event_name);