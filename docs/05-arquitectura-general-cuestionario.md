# Cuestionario de cierre de arquitectura SyncFiles

## Línea base definida

- V1: SQLite + almacenamiento local + REST/JSON + polling + criterio de archivo más nuevo con confirmación del usuario + posibilidad de conservar versiones alternativas por copia/renombrado.
- V2: NAS + PostgreSQL + Kafka + E2E + resolución de conflictos avanzada.
- WebSocket y gRPC quedan como alternativas de transporte a evaluar en fases posteriores; no son requisito obligatorio de v1 ni de v2.

## Sección 2 — Principios de diseño

### 2.1. ¿Cuál es la decisión exacta de conflicto en v1?  [RESPONDIDA]
- La versión más reciente gana solo por defecto.
- El usuario debe confirmar la resolución cuando hay conflicto real.
- Las opciones son: aceptar la versión más reciente, conservar la alternativa como copia en conflicto, renombrar la alternativa para revisión manual o dejar ambas versiones persistidas.

### 2.2. ¿Qué significa “archivo más nuevo” en la práctica? [RESPONDIDA]
- En v1, la regla base es `modified_at` con tolerancia de ±2–5 segundos.
- Si la diferencia temporal es ambigua, se puede revisar el `checksum` como diagnóstico, pero la decisión no depende del hash como criterio principal.
- La resolución sigue siendo por la versión más reciente por defecto, con confirmación del usuario antes de cerrar el conflicto.

### 2.3. ¿Qué debe ocurrir cuando ambos lados cambian el mismo archivo pero en rutas distintas? [RESPONDIDA]
- Si el contenido final difiere aunque la ruta cambie, se trata como conflicto de contenido y no como cambio inocuo.
- Si el contenido es idéntico y solo cambia la ruta, se considera renombrado o copia, no conflicto.
- En caso de conflicto, se conserva la alternativa como copia/renombrado y la resolución final requiere confirmación del usuario.

### 2.4. ¿Cómo se define el origen de verdad de metadatos en v1? [RESPONDIDA]
- El servidor siempre gana y es la fuente de verdad de metadatos.
- El cliente solo propone cambios; no reescribe ni valida autoridad de estado sin confirmar con el servidor.
- La metadata canónica es la del servidor: `checksum`, `modified_at`, `path_hash`, `device_id`, `last_sync` y estado final de sincronización.

### 2.5. ¿Qué reglas exactas debe seguir la cola local persistente? [RESPONDIDA]
- La cola se escribe antes de intentar la operación y cada item recibe un `idempotency_key`.
- Si la app reinicia durante un upload, la operación puede reintentarse sin duplicar efectos porque el servidor dedupea por la misma clave.
- El cliente mantiene estados `queued`/`in_flight`/`retry` para reintentar de forma segura.

### 2.6. ¿Qué tolera la arquitectura de v1 respecto a fallos parciales? [RESPONDIDA]
- La operación debe rechazarse y reintentarse; no se deja un archivo en estado intermedio no consistente.
- La definición de operación completa es aquella que tiene confirmación del servidor y metadatos actualizados.
- La operación queda incompleta si no existe confirmación final o si hubo error de escritura, red o timeout antes del commit.

### 2.7. ¿Qué se considera una “conflicto real” y qué un “cambio no conflictivo”? [RESPONDIDA]
- Un conflicto real ocurre cuando ambos lados cambiaron desde la última sincronización y el contenido final difiere por hash.
- Un cambio no conflictivo ocurre cuando solo un lado cambia el archivo, o cuando la ruta cambia y el contenido final sigue siendo idéntico.
- Un borrado local frente a modificacion remota se resuelve como borrado canónico si no hay una versión posterior que el usuario quiera conservar.

### 2.8. ¿Quién decide la resolución final del conflicto en v1? [RESPONDIDA]
- El usuario confirma la decisión final con opción de aceptar la versión más reciente o conservar la alternativa como copia/renombrado.
- El sistema no sobrescribe silenciosamente; el cliente y el servidor solo ejecutan la decisión confirmada.

### 2.9. ¿Qué tipo de decisiones quedan definidas como “regla por defecto” y cuáles deben ser decisiones manuales del usuario? [RESPONDIDA]
- Regla por defecto: la versión más reciente gana, siempre que el cambio sea inequívoco.
- Decisión manual del usuario: conservar la alternativa como copia en conflicto, renombrarla para revisión, mantener ambas versiones o aceptar la versión más reciente.
- La sobrescritura silenciosa queda prohibida en v1.

### 2.10. ¿Qué pasa con la seguridad en v1 y en v2? [RESPONDIDA]
- **v1:** los metadatos operativos del servidor **son visibles** (`checksum`, `modified_at`, `path_hash`, `device_id`, `last_sync`, estado de sincronización). La encriptación E2E no es obligatoria en v1.
- **v2:** todos los datos serán cifrados E2E; el servidor no podrá leer metadatos sensibles.
- **Separación exacta:**
  - `display_name`: nombre legible del archivo (candidato a cifrado en v2).
  - `full_path`: ruta completa del archivo (candidato a cifrado en v2).
  - **Metadatos operativos** (visibles en v1): `checksum`, `modified_at`, `path_hash`, `device_id`, `last_sync`, estado final de sincronización, tamaño.
- **Regla:** desde v1 se separan los campos operacionales de los campos candidatos a cifrado para evitar reescritura de base de datos en v2.

## Sección 3 — Componentes principales

### 3.1. ¿Cuál es la topología exacta de v1? [RESPONDIDA]
- **Topología exacta:** un único backend monolítico, operado en un único nodo (single-node), con SQLite como base de metadatos y almacenamiento local mediante `LocalDiskStorageProvider`.
- **No hay clúster ni multi-instancia activa** en v1; la tolerancia a fallos se resuelve con backups, cola persistente y recuperación manual asistida.
- La arquitectura es monolítica modular, no un sistema distribuido.

### 3.2. ¿Qué módulos internos deben existir con responsabilidad clara? [RESPONDIDA]
- `Identity`
- `Sync Orchestrator`
- `Metadata Catalog`
- `Conflict Resolution`
- `Storage Gateway`
- `Background Jobs`
- `Audit`
- `Versioning` (stub o interfaz, no funcional en v1)
- **Activos en v1:** Identity, Sync Orchestrator, Metadata Catalog, Conflict Resolution, Storage Gateway, Background Jobs, Audit.
- **Stub en v1:** Versioning; queda preparado para uso posterior.

### 3.3. ¿Qué capa de almacenamiento es la real en v1? [RESPONDIDA]
- `LocalDiskStorageProvider` es la implementación concreta de v1.
- **Layout sugerido:**
  - `/data/users/{user_id}/files/...` para archivos de usuario.
  - `/data/users/{user_id}/conflicts/...` para copias/renombrados por conflicto.
  - `/data/users/{user_id}/staging/...` para operaciones en preparación o temporales (si aplica).
- La capa usa `StorageProvider` como abstracción para permitir NAS en v2 sin cambiar el núcleo del sistema.

### 3.4. ¿Qué layout de metadatos debe existir en SQLite? [RESPONDIDA]
- **Tablas mínimas sugeridas:**
  - `users`
  - `devices`
  - `sessions`
  - `files`
  - `sync_queue`
  - `conflicts`
  - `audit_log`
- **Columnas mínimas recomendadas en `files`:**
  - `file_id`, `user_id`, `device_id`, `path_hash`, `relative_path`, `checksum`, `size_bytes`, `modified_at`, `synced_at`, `status`, `last_seen_version`, `deleted_at`.
- **Columnas mínimas recomendadas en `sync_queue`:**
  - `queue_id`, `file_id`, `operation`, `status`, `attempts`, `last_error`, `created_at`, `updated_at`, `idempotency_key`.
- **En `conflicts`:**
  - `conflict_id`, `file_id`, `local_path`, `remote_path`, `strategy`, `created_at`, `resolved_at`, `resolved_by`.

### 3.5. ¿Qué define un archivo “sincronizado” vs “pendiente” vs “con conflicto”? [RESPONDIDA]
- **Sincronizado:** el archivo tiene un `checksum` aceptado, `modified_at` coherente con el servidor y el estado de sincronización actual es `synced`.
- **Pendiente:** existe una operación en `sync_queue` no resuelta o el archivo tiene cambios pendientes en cliente/servidor.
- **Con conflicto:** hubo cambio concurrente desde `last_sync` y el contenido final difiere; el sistema marca el caso como `conflict` y exige decisión del usuario o resolución explícita.
- La definición se hace con combinación de `checksum`, `modified_at`, `device_id`, `path_hash`, `status` y `last_sync`.

### 3.6. ¿Qué contrato de API debe existir en v1? [RESPONDIDA]
- **Base REST/JSON sobre TLS 1.3** con prefix `/v1`.
- **Endpoints sugeridos:**
  - `POST /v1/auth/login`
  - `POST /v1/auth/logout`
  - `GET /v1/sync/diff`
  - `POST /v1/sync/upload`
  - `POST /v1/sync/download`
  - `POST /v1/sync/delete`
  - `POST /v1/sync/rename`
  - `POST /v1/sync/move`
  - `POST /v1/sync/copy`
  - `GET /v1/conflicts`
  - `POST /v1/conflicts/resolve`
- **Payload mínimo recomendados:**
  - `user_id`, `device_id`, `session_id`, `idempotency_key`, `path_hash`, `relative_path`, `checksum`, `size_bytes`, `modified_at`, `operation`, `source_device`.
- **Error estándar:** `code`, `message`, `retryable`, `details`, `request_id`.

### 3.7. ¿Qué debe incluir el `idempotency_key`? [RESPONDIDA]
- Se genera **por operación**, no por archivo.
- La deduplicación dura **24 horas** en servidor.
- Si la misma clave se reutiliza con contenido distinto, se rechaza con `409 Conflict` o `422 Unprocessable Entity` para evitar corrupción.
- El cliente reintenta con la misma clave; el servidor debe devolver el mismo resultado sin duplicar efectos.

### 3.8. ¿Cuál es la política de polling exacta en v1? [RESPONDIDA]
- **Frecuencia base:** 30 segundos.
- **Adaptación opcional:** 15–60 segundos según carga, latencia o estado de cola.
- El cliente solicita **deltas por usuario y dispositivo**, no por archivo individual como fuente primaria del polling.
- El polling es la base de v1; WebSocket no es obligatorio.

### 3.9. ¿Qué hace el cliente en segundo plano? [RESPONDIDA]
- Guarda cada cambio en cola persistente antes de enviarlo.
- Reintenta automáticamente con backoff exponencial + jitter.
- Mantiene una base local de metadata + cola para recuperar tras cierre o reinicio.
- Si el cliente cae, al arrancar reanuda la cola con validación del servidor y deduplicación por `idempotency_key`.

### 3.10. ¿Qué es exactamente un “conflicto” desde la vista del cliente y del servidor? [RESPONDIDA]
- **Conflicto real:** ambos lados modificaron el mismo archivo desde la última sincronización y el contenido final difiere por hash.
- **No es conflicto:** solo un lado cambió, o el contenido es idéntico y la diferencia es solo la ruta/renombrado.
- **Delete vs modify:** si hay `delete` en un lado y `modify` en otro, el borrado puede ser canónico por defecto, pero el usuario puede decidir conservar la versión alternativa como copia de conflicto.

### 3.11. ¿Qué debemos mostrar en la UI en cada estado? [RESPONDIDA]
- Estados mínimos: `Sincronizado`, `Sincronizando`, `Con conflictos`, `Error`, `Pausado`.
- **Datos por archivo:** nombre, estado, tamaño, fecha de última sincronización, `checksum`, origen del último cambio, cantidad de cambios pendientes.
- **Datos por dispositivo:** nombre del dispositivo, estado de conexión, último sync local/remote, número de operaciones pendientes.
- **Botones de acción para conflicto:** `Aceptar más reciente`, `Guardar copia/renombrar`, `Mantener ambas versiones`, `Reintentar`.

### 3.12. ¿Cuáles son los mensajes mínimos de auditoría y observabilidad en v1? [RESPONDIDA]
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
- `auth.session_expired`
- La auditoría debe incluir `user_id`, `device_id`, `request_id`, `op_id`, `timestamp`, `result`, `message`.

### 3.13. ¿Qué debe estar definido para cerrar la parte operativa de v1? [RESPONDIDA]
- **Reintentos:** backoff exponencial + jitter, límite de 5 intentos por operación.
- **Umbrales de cola:** si la cola supera X operaciones o el elemento más antiguo supera Y minutos, se marca el sistema como `degraded` y se informa al usuario.
- **Modo degradado:** se sigue sincronizando con prioridad por tipo de operación, sin bloquear la app completa.
- **Acción humana:** si la cola supera el umbral, el usuario puede pausar, reintentar o revisar conflictos.

### 3.14. ¿Qué debe estar definido para cerrar la parte de v2? [RESPONDIDA]
- **NAS obligatorio:** sí, para v2.
- **PostgreSQL:** sí, como persistencia principal de metadatos.
- **Kafka:** sí, para desacoplamiento y event streaming de alto volumen.
- **E2E:** sí, obligatorio en v2.
- **Resolución de conflicto avanzada:** sí, merge asistido o policy más sofisticada.
- **WebSocket/gRPC:** son opciones de transporte para fases posteriores, no requisitos obligatorios de v2 por defecto.

## Cierre recomendado

Antes de avanzar a diseño detallado, estas 8 decisiones deben quedar cerradas en texto formal:

1. Reglas de conflicto v1.
2. Decisión de quien gana entre servidor y cliente.
3. Política de copia/renombrado en conflicto.
4. Modelo de metadatos y estados del archivo.
5. Estructura de SQLite y layout de almacenamiento local.
6. Contrato de API y payloads mínimos.
7. Política de reintentos y recuperación.
8. Línea base V1 vs V2 y la evolución de transporte.

Estas preguntas deben responderse de forma explícita, una por una, antes de iniciar el diseño de detalle del cliente y del servidor.
