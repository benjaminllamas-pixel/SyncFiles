# Arquitectura del Servidor

## 1. Objetivo y alcance del servidor

El servidor de SyncFiles es la capa central de estado, autenticación, autorización y coordinación de sincronización para V1. Su responsabilidad es garantizar coherencia y seguridad sin introducir complejidad innecesaria.

### 1.1 Objetivo
- validar identidad y sesión,
- mantener la fuente de verdad de los metadatos,
- servir archivos y deltas al cliente,
- aceptar cambios locales,
- aplicar políticas de consistencia,
- detectar conflictos de sincronización,
- conservar alternativas del conflicto de forma segura,
- mantener observabilidad operativa básica.

### 1.2 Alcance de V1
- sesiones server-side para cuentas precreadas,
- REST/JSON sobre TLS 1.3,
- SQLite como base operativa,
- almacenamiento local mediante `LocalDiskStorageProvider`,
- sincronización bidireccional con polling,
- operaciones de upload/download/delete/rename/move/copy,
- conflicto con regla de “archivo más reciente gana por defecto” y preservación alternativa bajo confirmación del usuario,
- auditoría mínima y backoff para reintentos.

### 1.3 Fuera de alcance de V1
- E2E obligatorio,
- NAS obligatorio,
- PostgreSQL obligatorio,
- Kafka obligatorio,
- WebSocket/gRPC obligatorios,
- versionado formal,
- sharing/collaboration multiusuario,
- trabajo offline obligatorio.

## 2. Topología del servidor

### 2.1 Topología operativa de V1
- single-node,
- monolito modular,
- SQLite como base de metadatos,
- almacenamiento local mediante `LocalDiskStorageProvider`,
- aplicación ejecutándose en una sola instancia operativa,
- backups automáticos externos para recuperación.

### 2.2 Supuestos de despliegue
- SPOF aceptado,
- downtime planificado permitido,
- no se usa cluster ni HA activo-activo,
- la recuperación se basa en backup, restauración y reintentos.

## 3. Módulos del servidor

### 3.1 Módulos activos

| Módulo | Responsabilidad | Estado |
|---|---|---|
| `Identity` | login, validación, sesion, revocación | Activo |
| `Sync Orchestrator` | coordinación de sincronización | Activo |
| `Metadata Catalog` | fuente de verdad de metadatos | Activo |
| `Conflict Resolution` | política de conflicto y resolución | Activo |
| `Storage Gateway` | acceso a archivos y layout local | Activo |
| `Background Jobs` | retry, cola, limpieza | Activo (mínimo) |
| `Audit` | eventos operativos y de seguridad | Activo |
| `Versioning` | historial formal de versiones | Stub |

### 3.2 Reglas de acoplamiento
- todos los módulos usan contratos internos,
- ninguno depende directamente de un storage concreto,
- `Metadata Catalog` es la fuente canónica del estado,
- `Storage Gateway` es el único punto de acceso a archivos,
- `Identity` y `Sync Orchestrator` no deben aceptar cambios de estado sin validar al servidor.

## 4. Principios del backend V1

- El servidor es la fuente de verdad de todos los metadatos.
- El cliente solo propone cambios.
- La sincronización debe ser idempotente y serializable por archivo.
- Las operaciones tienen atomicidad por archivo, no por lote completo.
- No hay sobrescritura silenciosa.
- El conflicto se resuelve con la rázon “archivo más reciente ganará por defecto”, pero el usuario puede preservar la alternativa.
- El resultado final debe ser reproducible a partir del estado y del `server_seq`.

## 5. Modelo de persistencia

### 5.1 Base de datos V1

La base de datos del servidor usar SQLite con un esquema mínimo, compatible con una migración posterior a PostgreSQL.

```sql
CREATE TABLE users (
  user_id TEXT PRIMARY KEY,
  email TEXT NOT NULL,
  password_hash TEXT NOT NULL,
  display_name TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE devices (
  device_id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  platform TEXT NOT NULL,
  device_name TEXT,
  last_seen_at INTEGER,
  created_at INTEGER NOT NULL,
  FOREIGN KEY(user_id) REFERENCES users(user_id)
);

CREATE TABLE sessions (
  session_id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  device_id TEXT NOT NULL,
  token_hash TEXT NOT NULL,
  issued_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  revoked_at INTEGER,
  status TEXT NOT NULL CHECK(status IN ('active','expired','revoked')),
  FOREIGN KEY(user_id) REFERENCES users(user_id),
  FOREIGN KEY(device_id) REFERENCES devices(device_id)
);

CREATE TABLE files (
  file_id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  device_id TEXT NOT NULL,
  relative_path TEXT NOT NULL,
  path_hash TEXT NOT NULL,
  checksum TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  modified_at INTEGER NOT NULL,
  synced_at INTEGER,
  status TEXT NOT NULL CHECK(status IN ('pending','synced','conflict','quarantined','deleted')),
  last_sync_version INTEGER DEFAULT 0,
  deleted_at INTEGER,
  UNIQUE(user_id, path_hash),
  FOREIGN KEY(user_id) REFERENCES users(user_id),
  FOREIGN KEY(device_id) REFERENCES devices(device_id)
);

CREATE TABLE sync_queue (
  queue_id TEXT PRIMARY KEY,
  file_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  device_id TEXT NOT NULL,
  operation TEXT NOT NULL CHECK(operation IN ('upload','download','delete','rename','move','copy')),
  status TEXT NOT NULL CHECK(status IN ('queued','in_flight','retry','done','failed')),
  attempts INTEGER NOT NULL DEFAULT 0,
  idempotency_key TEXT NOT NULL,
  payload_json TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  last_error TEXT,
  FOREIGN KEY(file_id) REFERENCES files(file_id),
  FOREIGN KEY(user_id) REFERENCES users(user_id),
  FOREIGN KEY(device_id) REFERENCES devices(device_id)
);

CREATE TABLE conflicts (
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
  resolved_by TEXT,
  FOREIGN KEY(file_id) REFERENCES files(file_id),
  FOREIGN KEY(user_id) REFERENCES users(user_id)
);

CREATE TABLE audit_log (
  audit_id TEXT PRIMARY KEY,
  user_id TEXT,
  device_id TEXT,
  event_name TEXT NOT NULL,
  event_type TEXT NOT NULL,
  payload_json TEXT,
  created_at INTEGER NOT NULL
);

CREATE INDEX idx_files_user_path ON files(user_id, path_hash);
CREATE INDEX idx_sync_queue_user_status ON sync_queue(user_id, status, created_at);
CREATE INDEX idx_conflicts_user_file ON conflicts(user_id, file_id);
CREATE INDEX idx_audit_event_time ON audit_log(created_at, event_name);
```

### 5.2 Reglas de estado
- `files.status` refleja el estado final conocido por el servidor.
- `sync_queue.status` refleja el estado del trabajo en cola.
- `conflicts` conserva la alternativa de conflicto y quién la resolvió.
- `sessions` tienen TTL de 24h base y expiración absoluta de 7 días.

## 6. Almacenamiento de archivos

### 6.1 `StorageProvider`
El backend accede a archivos solo a través de `StorageProvider`.

```text
StorageProvider
  └── LocalDiskStorageProvider
```

### 6.2 Layout local V1

```text
/data/
  syncfiles/
    users/
      {user_id}/
        files/
          {relative_path_normalized}
        conflicts/
          {file_id}/
            {device_id}_{timestamp}_{original_name}
        staging/
          {queue_id}_{file_name}
        backups/
          {date}/
            metadata.sqlite
            snapshots/
```

Reglas:
- la ruta canónica vive en `files/`;
- la alternativa de conflicto vive en `conflicts/`;
- los archivos temporales deben vivir en `staging/` y no pueden sustituir la versión canónica antes de confirmar;
- el `relative_path_normalized` se usa para establecer `path_hash` y resolver identidad de archivo.

## 7. Flujo de sincronización server-side

### 7.1 Orden de coordinación
- el servidor revisa el `session_id` y `device_id`;
- valida permisos y ownership;
- valida idempotency;
- aplica operación por archivo;
- actualiza `Metadata Catalog`;
- escribe en `audit_log`;
- devuelve `server_seq` con la respuesta.

### 7.2 Reglas de consistencia
- atomicidad por archivo,
- idempotencia por operación,
- orden de aplicación por `server_seq`,
- commit solo cuando el archivo y el metadata están validados,
- ninguna operación parcialmente aplicada queda sin validación.

### 7.3 Conflicto V1
Regla del backend:
- si ambos lados cambiaron desde `last_sync` y el `checksum` difiere, se marca conflicto.
- el archivo más reciente gana por defecto.
- si el usuario elige conservar la alternativa, se guarda una copia/renombrado en `conflicts/` y se deja la versión canónica como la aceptada por la regla más reciente.
- no hay sobrescritura silenciosa.

### 7.4 Delete vs modify
- si la operación es `delete` en un lado y `modify` en el otro, el borrado puede ser canónico por defecto;
- si el usuario quiere conservar la versión alternativa, se preserva la variante modificada en conflicto y se registra la decisión.

## 8. Contrato API V1

### 8.1 Base
- protocolo: REST/JSON sobre TLS 1.3,
- versionado: `/v1`,
- polling: cada 30s;
- idempotencia: obligatoria,
- timeouts y rate limiting: 429 con `Retry-After` cuando aplique.

### 8.2 Endpoints mínimos

#### 8.2.1 Auth
- `POST /v1/auth/login`
- `POST /v1/auth/logout`

#### 8.2.2 Sync
- `GET /v1/sync/diff?since=<server_seq>`
- `POST /v1/sync/upload`
- `POST /v1/sync/download`
- `POST /v1/sync/delete`
- `POST /v1/sync/rename`
- `POST /v1/sync/move`
- `POST /v1/sync/copy`

#### 8.2.3 Conflict
- `GET /v1/conflicts`
- `POST /v1/conflicts/resolve`

### 8.3 Payloads mínimos

#### Login
```json
{
  "email": "user@example.com",
  "password": "********",
  "device_id": "android-1"
}
```

#### Upload
```json
{
  "session_id": "sess_123",
  "device_id": "android-1",
  "file_id": "f_1",
  "relative_path": "docs/a.txt",
  "path_hash": "abc123",
  "checksum": "sha256:...",
  "size_bytes": 1024,
  "modified_at": 1700000000000,
  "idempotency_key": "op_123",
  "content": "base64(...)"
}
```

#### Resolve conflict
```json
{
  "session_id": "sess_123",
  "device_id": "android-1",
  "conflict_id": "c_10",
  "decision": "accept_latest",
  "preserve_alternative": true,
  "new_name": "a_conflict_20260101_155530.txt"
}
```

### 8.4 Respuestas mínimas

Success:
```json
{
  "accepted": true,
  "status": "synced",
  "server_seq": 43,
  "file_id": "f_1"
}
```

Error:
```json
{
  "code": "conflict_detected",
  "message": "The file has diverged and requires resolution.",
  "retryable": false,
  "request_id": "req_4421"
}
```

Reglas:
- todo éxito debe devolver `accepted` y `server_seq`;
- todo error debe devolver `code`, `message`, `retryable` y `request_id`;
- la misma `idempotency_key` no puede producir efectos distintos.

## 9. Seguridad del servidor

### 9.1 Autenticación y sesión
- token opaco,
- estado server-side,
- `device_id` obligatorio por sesión,
- validar sesión/usuario/dispositivo en cada request,
- 24h TTL base y 7 días absolutos,
- máximo 3 sesiones concurrentes por usuario.

### 9.2 Autorización
- cada acción requiere ownership por `user_id`,
- cada file y path se valida contra el usuario autenticado,
- el cliente no puede forzar permisos.

### 9.3 Seguridad del canal
- TLS 1.3 obligatorio,
- validación del chain del sistema operativo,
- fail-closed ante error TLS,
- no pinning en v1.

### 9.4 Seguridad de datos y metadatos
- v1 puede operar con metadatos visibles en servidor,
- E2E no es obligatorio,
- los nombres visibles y el contenido quedan fuera del alcance de v1,
- cualquier campo sensible debe ser separable para v2 sin reescritura masiva.

## 10. Operación y observabilidad

### 10.1 Eventos mínimos
- `auth.login_success`
- `auth.login_failed`
- `sync.file_uploaded`
- `sync.file_downloaded`
- `sync.file_deleted`
- `sync.file_renamed`
- `sync.file_moved`
- `sync.file_copied`
- `sync.conflict_detected`
- `sync.retry_scheduled`
- `sync.quarantined`

Cada evento debe incluir:
- `ts`
- `level`
- `event`
- `user_id`
- `device_id`
- `request_id`
- `result`
- `message`

### 10.2 Métricas mínimas
- sync latency p50/p95,
- failed operations total,
- active sessions,
- queue depth,
- retry rate,
- conflict rate,
- storage usage per user.

### 10.3 Resiliencia operativa
- backoff exponencial con jitter,
- 429 con `Retry-After`,
- máximo 5 reintentos por operación,
- reintentos por red, timeout, 408, 429, 5xx,
- error terminal para 400, 401, 403, 404, 422.

### 10.4 Backup y recuperación
- backup diario de SQLite y de archivos,
- almacenamiento fuera del nodo principal,
- restauración verificada con checksum,
- RPO ≤ 24 horas,
- RTO < 4 horas.

## 11. Criterios de aceptación del servidor V1

La implementación del backend debe cumplir lo siguiente:
- autentica usuarios precreados,
- vincula sesión a dispositivo,
- valida permisos por usuario,
- sirve API REST versión /v1,
- mantiene SQLite como base operativa,
- acepta operaciones de archivo,
- aplica idempotencia y serialización por archivo,
- resuelve conflictos evitando sobrescritura silenciosa,
- guarda copias alternativas en `conflicts/`,
- mantiene auditoría mínima,
- soporta restauración ante caída con backup local o remoto.

## 12. Evolución post-V1

Tras la entrega de V1, el servidor se prepara para:
- NAS,
- PostgreSQL,
- Kafka,
- E2E,
- resolución avanzada de conflictos,
- WebSocket/gRPC como evolución de transporte,
- versionado formal,
- capacidad multiusuario y compartición más rica.

Este diseño del servidor deja la base consistente con el cliente, la API y la arquitectura general, y mantiene la evolución de V2 como un paso natural tras la estabilización de la versión V1.
co) sin ampliar funcionalidades no aprobadas.
- [x] Respeta contrato REST `/v1` + TLS 1.3 + polling 30s en v1.
- [x] Mantiene regla de conflicto por copia en conflicto universal.
- [x] Conserva SQLite + `LocalDiskStorageProvider` como estado operativo v1.
- [x] No convierte Kafka/Redis/NAS/E2E en requisitos obligatorios de v1.
- [x] Conserva separación explícita entre estado v1 y evolución post-MVP con triggers claros.