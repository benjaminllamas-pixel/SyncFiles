# Base de Datos de Metadatos

## BD Local (SQLite)

```sql
-- Archivo de BD: ~/.syncfiles/cache/metadata.db

PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;

CREATE TABLE files (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    size INTEGER,
    hash TEXT,
    modified_at INTEGER,
    synced_at INTEGER,
    status TEXT DEFAULT 'pending'
);

CREATE TABLE sync_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id TEXT,
    operation TEXT,
    timestamp INTEGER,
    retry_count INTEGER DEFAULT 0
);

CREATE INDEX idx_sync_queue_timestamp ON sync_queue(timestamp);
```

## BD Servidor (PostgreSQL)

```sql
-- Replicada, con backups
-- Encriptación a nivel de columna donde necesario

CREATE SCHEMA syncfiles;

CREATE TABLE syncfiles.users (...)
CREATE TABLE syncfiles.files (...)
CREATE TABLE syncfiles.sync_status (...)
CREATE TABLE syncfiles.audit_log (...)
```

## Optimizaciones

- Connection pooling (20-50 conexiones)
- Prepared statements (prevenir SQL injection)
- Índices multi-columna en queries frecuentes
- VACUUM automático cada 24h
- Particionamiento por user_id si escala
