# SyncFiles API — OpenAPI 3.0

Servidor: `syncfiles-server` · Puerto: 8080 · Base path: `/api/v1`

## Autenticación

Todos los endpoints (salvo `/auth/login`) exigen el header:
```
Authorization: Bearer <session_token>
```
El token se obtiene en `/auth/login` y expira tras `expires_at` (epoch millis).

## Seguridad global

- Rate limiting: 60 req/s por IP (configurable con `SF_RATE_LIMIT_RPS`).
- Content-Type: `application/json` en bodies.
- Validación de paths: sin `..`, sin path absoluto, sin caracteres extraños.
- Idempotency-Key: cada mutación recibe un `idempotency_key` único; repetir la misma clave devuelve la respuesta original.

## Endpoints

### POST /auth/login
```json
{ "email": "admin@syncfiles.local", "password": "syncfiles", "device_id": "desktop-abc" }
```
**200:** `LoginResponse` · **400:** `AUTH_FAILED`

### POST /auth/logout
**200:** `accepted: true`

### GET /session/status
**200:** `Session` · **401:** `UNAUTHORIZED`

### POST /sync/diff
```json
{ "since": 0, "device_id": "desktop-abc", "session_id": "...", "request_id": "..." }
```
Devuelve los cambios desde `since` (cursor de `seq` del `change_log`).
**200:** `DiffResponse` · **401:** `UNAUTHORIZED`

### POST /sync/upload
Cuerpo: `UploadRequest` (content en base64). Validaciones:
- `path_hash` debe coincidir con `compute_path_hash(relative_path)`.
- Content-Type debe ser `application/json`.
- Tamaño máximo: 100 MiB (configurable con `SF_MAX_UPLOAD_BYTES`).
**200:** `accepted: true` · **400:** `INVALID_PATH`, `INVALID_CONTENT`, `INVALID_PATH_HASH`, `CONFLICT` (409)

### POST /sync/download
```json
{ "session_id": "...", "device_id": "...", "file_id": "...", "path_hash": "...", "idempotency_key": "..." }
```
**200:** `DownloadResponse` (content en base64) · **400:** `NOT_FOUND`

### POST /sync/delete
**200:** `accepted: true` · **400:** `NOT_FOUND`

### POST /sync/rename
```json
{ "session_id": "...", "device_id": "...", "file_id": "...", "old_path": "docs/viejo.txt", "new_path": "docs/nuevo.txt", "idempotency_key": "..." }
```
**200:** `accepted: true` · **400:** `INVALID_PATH`, `NOT_FOUND`

### POST /sync/move
Igual que rename pero semánticamente un move (cambiar carpeta).

### POST /sync/copy
```json
{ "session_id": "...", "device_id": "...", "file_id": "...", "source_path": "docs/a.txt", "destination_path": "docs/b.txt", "idempotency_key": "..." }
```
Crea un nuevo `file_id` en el destino.

### GET /files/list
Query: `?include_deleted=true` (opcional).
**200:** `FilesListResponse`

### GET /queue
Query: `?device_id=...&status=pending&limit=50`.
**200:** `QueueResponse`

### GET /activity
Query: `?limit=100`.
**200:** `ActivityResponse`

### GET /conflicts
**200:** `ConflictsResponse`

### GET /devices
**200:** `DevicesResponse`

### POST /devices/revoke
```json
{ "session_id": "...", "device_id": "...", "target_device_id": "..." }
```
**200:** `{ "sessions_revoked": N }` · **400:** `INVALID_TARGET`, `NOT_FOUND`

### GET /storage/stats
**200:** `StorageStats`

### POST /conflicts/resolve
```json
{ "session_id": "...", "device_id": "...", "conflict_id": "...", "decision": "keep_local|keep_remote|rename|copy", "preserve_alternative": true, "new_name": "docs/nuevo.txt" }
```
**200:** `accepted: true` · **400:** `INVALID_DECISION`, `NOT_FOUND`

### GET /health/live
**200:** `{ "status": "alive" }`

### GET /health/ready
Chequea pool SQLite y escritura de healthcheck en storage.
**200:** `ready` · **503:** `not_ready`

## Errores

Toda respuesta de error incluye:
```json
{ "code": "ERROR_CODE", "message": "...", "retryable": false, "request_id": "..." }
```
- `request_id` es un UUID único por solicitud (útil para trazas).
- `retryable: true` → el cliente puede reintentar con backoff exponencial.