# Plan: Robustez y Respaldo de Datos — SyncFiles V1

## Estado Actual

### Lo que YA existe (probado en código)
- Persistencia de metadatos doble capa SQLite (servidor + cliente) con esquema idéntico en syncfiles-models/src/lib.rs
- Almacenamiento de archivos vía StorageProvider trait + LocalDiskStorageProvider con layout users/<user>/files/
- Integridad por checksum SHA-256 en cliente y servidor, con validación de path_hash
- Cola persistente sync_queue con idempotency_key — sobrevive a reinicios (test validado)
- Auditoría append-only audit_log + change_log con cursor de seq (orden reproducible)
- Migraciones versionadas migrations.rs con tabla _migrations y scripts idempotentes
- Health checks liveness + readiness (DB + storage probe)

### Lo que FALTA o es Débil
1. Backups automatizados — documentados pero sin código
2. WAL mode + foreign_keys — SQLite en modo default
3. Versionado de archivos — Versioning es stub
4. Preservación de conflictos incompleta
5. Retención/GDPR — sin purga automática
6. Ruta de migraciones frágil — relativo al CWD
7. Health check readiness — falla en CI
8. RateLimiter in-memory — sin preparación para Redis

---

## Fase 0: Correcciones Inmediatas de Bajo Riesgo

### Tarea 0.1 — Activar WAL mode + foreign_keys en SQLite
**Archivo:** `syncfiles-server/src/state.rs` (método `AppState::new`)

Antes de crear el pool, configurar pragmas:
```rust
let mut connect_options = SqliteConnectOptions::from_str(&config.database_url)?;
connect_options = connect_options
    .pragma("journal_mode=WAL")
    .pragma("foreign_keys=ON")
    .pragma("synchronous=NORMAL")
    .pragma("busy_timeout=5000");
```

**Impacto:** WAL protege contra corrupción parcial al cerrar bruscamente. foreign_keys=ON activa las FKs que el esquema declara pero SQLite no validaba por default. synchronous=NORMAL con WAL es seguro.

**Test:** Agregar test en `db.rs` que abra pool, inserte datos, simule crash (sin close), reapertura y verifique `PRAGMA integrity_check`.

### Tarea 0.2 — Corregir ruta de migraciones a path absoluto
**Archivo:** `syncfiles-server/src/main.rs` (línea 35)

Reemplazar:
```rust
let migrations_dir = std::path::Path::new("./syncfiles-server/migrations");
```
por:
```rust
let migrations_dir = std::env::current_exe()?
    .parent()
    .unwrap_or_else(|| std::path::Path::new("."))
    .join("migrations");
let migrations_dir = if migrations_dir.is_dir() {
    migrations_dir
} else {
    std::path::PathBuf::from("syncfiles-server/migrations")
};
```

**Impacto:** Las migraciones ya no dependen del CWD. Fallback al comportamiento actual si el path absoluto no existe.

### Tarea 0.3 — Agregar PRAGMA integrity_check al arranque
**Archivo:** `syncfiles-server/src/db.rs` (función `init_db`)

Después de inicializar el esquema:
```rust
let check: String = sqlx::query_scalar("PRAGMA integrity_check")
    .fetch_one(pool).await?;
if check != "ok" {
    tracing::error!("SQLite integrity_check falló: {}", check);
}
```

**Impacto:** Detecta corrupción de DB al arranque. No panic — loguea pero el server arranca.

### Tarea 0.4 — Health check readiness: no escribir archivo en CI
**Archivo:** `syncfiles-server/src/health.rs`

Hacer que la escritura del probe file sea opcional mediante variable de entorno:
```rust
if std::env::var("SF_HEALTH_PROBE_WRITE").map(|v| v == "1").unwrap_or(true) {
    // escribir probe file
}
```

**Impacto:** El readiness check falla si storage_root no es escribible (ej: CI). Con esta variable, se puede desactivar en CI.

---

## Fase 1: Backups Automatizados y Endurecimiento SQLite

### Tarea 1.1 — Script de backup bash
**Archivo nuevo:** `scripts/backup.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail
# Variables: SF_STORAGE_ROOT, SF_DATABASE_URL, BACKUP_DIR, BACKUP_RETENTION_DAYS
# 1. Backup de metadatos: VACUUM INTO 'backup.db' (atomic, online)
# 2. Backup de archivos: tar -czf storage-backup.tar.gz -C "$SF_STORAGE_ROOT" users/
# 3. Combinar en timestamped archive: backup_YYYYMMDD_HHMM.tar.gz
# 4. Rotación: eliminar backups más antiguos que BACKUP_RETENTION_DAYS
# 5. Loggear resultado a un archivo de log
```

**Dependencias:** Cero. Solo bash + sqlite3 CLI (o usar `.backup` de sqlite3).

**Frecuencia:** Diario via cron:
```
0 2 * * * /path/to/scripts/backup.sh >> /var/log/syncfiles-backup.log 2>&1
```

### Tarea 1.2 — Endpoint de backup manual en el server
**Archivo nuevo:** `syncfiles-server/src/backup.rs` + endpoint en `main.rs`

Endpoint `POST /admin/backup` (protegido por token de admin):
1. Ejecuta `VACUUM INTO '<backup_dir>/metadata_<timestamp>.db'` via pool.
2. Crea tar de `storage_root/users/` a `<backup_dir>/storage_<timestamp>.tar.gz`.
3. Devuelve path + checksum del backup.

### Tarea 1.3 — Test E2E de backup y restore
**Archivo nuevo:** `scripts/e2e-backup-restore.sh`

Flujo:
1. Iniciar server, subir archivos, verificar que existen.
2. Ejecutar backup.
3. Parar server, borrar DB + storage.
4. Restaurar backup.
5. Reiniciar server, verificar que los archivos están de vuelta y los checksums coinciden.

### Tarea 1.4 — Job background de limpieza y salud
**Archivo nuevo:** `syncfiles-server/src/jobs.rs`

Un `tokio::task::spawn` en `main.rs` que ejecute periódicamente:
- `prune_audit_log()` — diario, eliminar auditoría con más de 90 días
- `check_foreign_keys()` — diario, `PRAGMA foreign_key_check`
- `VACUUM` (o `PRAGMA optimize`) — semanal, reclaim space de deletes
- `prune_sessions_expired()` — eliminar sesiones con expires_at mayor a 7 días
- `prune_idempotency_keys()` — eliminar claves con más de 30 días

### Tarea 1.5 — Preparar RateLimiter para Redis
**Archivo:** `syncfiles-server/src/middleware/mod.rs`

Refactorar `RateLimiter` a un trait:
```rust
pub trait RateLimiter: Send + Sync {
    fn allow(&self, key: &str) -> bool;
}
pub struct InMemoryRateLimiter { ... }
pub struct RedisRateLimiter { ... }  // para V3
```
El `build_rate_limiter()` elige según env var `SF_RATELIMITER_BACKEND=redis|memory`.

### Tarea 1.6 — Content-Type middleware: excluir /sync/upload
**Archivo:** `syncfiles-server/src/middleware/mod.rs`

Excluir `/api/v1/sync/upload` del middleware Content-Type para permitir multipart/form-data en V2.

---

## Fase 2: Versionado de Archivos y Preservación de Conflictos

### Tarea 2.1 — Migración 002: tabla file_versions
**Archivo nuevo:** `syncfiles-server/migrations/002__add_versions.sql`

```sql
CREATE TABLE IF NOT EXISTS file_versions (
    version_id TEXT PRIMARY KEY,
    file_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    version_seq INTEGER NOT NULL,
    relative_path TEXT NOT NULL,
    path_hash TEXT NOT NULL,
    checksum TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    content_ref TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    created_by_device TEXT NOT NULL,
    FOREIGN KEY(file_id) REFERENCES files(file_id)
);
CREATE INDEX IF NOT EXISTS idx_versions_file_seq ON file_versions(file_id, version_seq);
```

### Tarea 2.2 — Flujo de versionado en upload_handler
**Archivo:** `syncfiles-server/src/handlers.rs` (upload_handler)

Cuando se acepta un archivo cuyo file_id ya existe con checksum distinto (y no es conflicto), antes de sobrescribir:
1. Copiar contenido actual a `<storage_root>/users/<user>/versions/<file_id>/<version_seq>_<timestamp>`
2. Insertar fila en `file_versions` con metadatos del estado anterior
3. Sobrescribir con el nuevo contenido

### Tarea 2.3 — Endpoints de versionado
**Archivos:** `syncfiles-server/src/handlers.rs` + `main.rs`

| Endpoint | Acción |
|----------|--------|
| `GET /files/:file_id/versions` | Lista versiones de un archivo |
| `POST /files/:file_id/versions/:version_id/restore` | Restaurar versión (crea nueva versión con contenido antiguo) |
| `DELETE /files/:file_id/versions/:version_id` | Eliminar una versión específica |

### Tarea 2.4 — Delete vs Modify conflict
**Archivo:** `syncfiles-server/src/handlers.rs` (delete_handler)

Antes de borrar, buscar si hay un conflicto reciente (< 5 min) para el mismo file_id. Si existe, copiar el contenido actual a `conflicts/` antes de borrar.

### Tarea 2.5 — Rename/Move conflict detection
**Archivos:** `syncfiles-server/src/handlers.rs` (rename_handler, move_handler)

Comparar el path_hash actual en DB con el path_hash que el cliente envió. Si difieren, preservar la alternativa antes de aplicar el cambio.

### Tarea 2.6 — Mejoras al endpoint de resolución de conflictos
**Archivo:** `syncfiles-server/src/handlers.rs` (resolve_conflict_handler)

Permitir al usuario elegir: `keep_local`, `keep_remote`, `keep_both`, `discard_alternative`. Al resolver, mover la alternativa desde `conflicts/` a su ubicación final o eliminarla.

---

## Fase 3: Retención, Exportación y Purga GDPR

### Tarea 3.1 — Exportación de datos del usuario
**Archivo nuevo endpoint:** `GET /api/v1/gdpr/export?user_id=X`

Recuperar todos los files, sync_queue, audit_log, conflicts del usuario. Serializar a JSON estructurado. Incluir metadatos: timestamps, device_ids, checksums (no contenido binario, solo referencias). Devolver como archivo JSON descargable.

### Tarea 3.2 — Purga (derecho al olvido)
**Archivo nuevo endpoint:** `DELETE /api/v1/gdpr/erase?user_id=X`

1. Eliminar todas las filas de files, sessions, sync_queue, conflicts, audit_log, change_log donde user_id coincida.
2. Eliminar directorio `<storage_root>/users/<user_id>/` recursivamente.
3. Registrar la operación en un log de auditoría separado (GDPR erase log).

### Tarea 3.3 — Retención automática (ya parcialmente en Fase 1)
- `prune_audit_log()` — 90 días (diario)
- `prune_sessions_expired()` — 7 días (diario)
- `prune_idempotency_keys()` — 30 días (diario)
- `change_log` NO purgar en V1 — es inmutable por diseño (los clientes usan cursor de seq)

---

## Fase 4: Opciones de Alta Disponibilidad

### Opción 1: Single-node + backups externos (VAI)
Estado actual + Fase 1. Costo cero. SPOF aceptado. RTO alto.

### Opción 2: Active-Passive con storage compartido (BAJO COSTO)
Dos instancias del server compartiendo:
- Misma base de datos SQLite en disco compartido (NFS/SMB/iSCSI)
- Mismo storage_root

Heartbeat cada 5s. Si el primario falla, el secundario asume.
Costo: bajo (solo disco compartido + script de failover de 30 líneas).
Desventaja: SQLite no diseñado para acceso concurrente desde dos nodos.

### Opción 3: Dual-write a dos SQLite + reconciliación (INGENIOSA, ESCALABLE)
Dos instancias con sus propias DB y storage. Nuevas escrituras se duplican a ambos (dual-write). Job de reconciliación cada hora usando change_log (ya existe).
Costo: bajo (solo servidores adicionales).
Ventajas: sin SPOF de hardware, failover transparente, change_log es el protocolo de replicación.
Migrabilidad: Excelente. Si se cambia a PostgreSQL, solo cambia la capa de DB.

### Opción 4: PostgreSQL + streaming replication + storage S3-compatible (PRO)
Reemplazar SQLite por PostgreSQL con streaming replication. Storage local por MinIO/S3.
Costo: moderado-alto.
Ventajas: HA total, durable 11 nines, escalable.

### Opción 5: Kubernetes + operator pattern (ENTERPRISE)
StatefulSet + PersistentVolumeClaim + operator customizado + KEDA para auto-scaling.
Costo: alto. Overkill para V1/V2.

### Recomendación
Para empezar: Opción 3 (dual-write). Es la más ingeniosa, barca, y es migratoria a opciones más avanzadas.

---

## Validación y Criterios de Aceptación

### Fase 0
- [ ] `cargo test --package syncfiles-server` pasa (incluyendo tests de db.rs)
- [ ] `PRAGMA journal_mode` devuelve `wal` al iniciar el server
- [ ] `PRAGMA foreign_key_check` no devuelve filas en base de datos de test
- [ ] Migraciones se ejecutan correctamente cuando el server se ejecuta desde un directorio diferente al del repo

### Fase 1
- [ ] `scripts/backup.sh` ejecutable, crea backups con timestamp, respeta retención
- [ ] `scripts/e2e-backup-restore.sh` pasa: backup → restore → verify checksums
- [ ] Endpoint `POST /admin/backup` devuelve 200 con path y checksum
- [ ] Jobs background se ejecutan sin error (verificar logs)
- [ ] RateLimiter trait compila y `build_rate_limiter()` funciona con `SF_RATELIMITER_BACKEND=memory`

### Fase 2
- [ ] Migración 002 se aplica correctamente (tabla `file_versions` existe)
- [ ] Test: upload de archivo modificado crea versión anterior automáticamente
- [ ] Test: restore de versión crea nueva versión con contenido antiguo
- [ ] Test: delete con conflicto reciente preserva alternativa
- [ ] Test: rename con path_hash conflict detection marca conflicto
- [ ] Tests de endpoints de versionado pasan

### Fase 3
- [ ] Endpoint `GET /gdpr/export` devuelve JSON con todos los datos del usuario
- [ ] Endpoint `DELETE /gdpr/erase` elimina todos los datos del usuario y el directorio de storage
- [ ] Jobs de retención eliminan datos antiguos según las políticas definidas

### Fase 4
- [ ] Opción elegida por el usuario es documentada y los componentes necesarios están implementados

---

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Mitigación |
|--------|--------------|------------|
| WAL mode rompe tests que asumen journal_mode=DELETE | Media | Actualizar tests a expectations de WAL |
| Migración 002 rompe bases existentes sin versionado | Baja | Script es `CREATE TABLE IF NOT EXISTS`, idempotente |
| Dual-write puede causar inconsistencias | Media | Tabla `pending_replicas` para retry automático |
| Purga GDPR puede eliminar datos equivocados | Baja | Test E2E con usuario de test antes de activar |
| Jobs background consumen recursos | Baja | Ejecutar con intervalos generosos (1h-24h) |

---

## Dependencias entre Fases

```
Fase 0 → Fase 1 → Fase 2 → Fase 3 → Fase 4
  (opcional)     (opcional)   (opcional)
```

Cada fase es independiente de las siguientes, pero se recomienda orden secuencial para minimizar riesgos.

---