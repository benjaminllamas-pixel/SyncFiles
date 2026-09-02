# Arquitectura del Cliente

## 1. Objetivo y alcance

El cliente de SyncFiles es la capa responsable de:
- autenticar al usuario en un dispositivo válido,
- observar cambios locales en el árbol sincronizado,
- encolar operaciones persistentes,
- comunicar con el servidor por REST/JSON sobre TLS,
- recuperar deltas remotos mediante polling,
- aplicar decisiones de sincronización y conflicto,
- mantener estado local y permitir recuperación tras reinicio.

Este diseño cubre la versión V1 del producto, con las siguientes decisiones ya cerradas:
- single-node backend,
- SQLite como base local de metadatos,
- almacenamiento local mediante `LocalDiskStorageProvider`,
- `REST/JSON + polling` como mecanismo de sincronización,
- servidor como fuente de verdad,
- conflicto por defecto: archivo más reciente gana, siempre que el usuario confirme y pueda conservar la alternativa.

Fuera de alcance del cliente V1:
- E2E de contenido,
- sincronización offline obligatoria,
- WebSocket/gRPC como requisito,
- versionado formal del archivo,
- colaboración multiusuario.

## 2. Principios del cliente

- Local-first con validación remota: el cliente puede detectar cambios y ponerlos en cola, pero la decisión final de sincronización se valida en servidor.
- Persistencia antes de envío: cada cambio se registra en disco antes de intentar una operación remota.
- Idempotencia por operación: todo item de cola usa `idempotency_key`.
- Fallar en cerrado para seguridad: si la sesión Caduca o el TLS falla, la app no continúa con operación remota.
- Transparencia para el usuario: estados visibles claros para sincronización, conflicto y error.
- Portabilidad: el mismo flujo funcional debe funcionar en Windows, macOS, Linux y Android.

## 3. Estructura modular

El cliente se diseña como un conjunto de módulos con responsabilidades separadas y acoplamiento mínimo.

### 3.1 Módulo `AppShell`
Responsable de:
- inicializar la aplicación,
- crear el ciclo de vida de sesión,
- definir la configuración operativa,
- coordinar los módulos en arranque y apagado.

Submódulos:
- `ConfigManager`
- `SessionManager`
- `LifecycleController`

### 3.2 Módulo `Auth` / `Session`
Responsable de:
- login con cuenta precreada,
- almacenamiento seguro del token o secreto local,
- validación de sesión activa,
- expiración, renovación y cierre de sesión,
- asociación de sesión con `device_id`.

Mecanismos:
- almacenamiento seguro nativo del sistema operativo,
- token opaco y valida únicamente en servidor,
- sesión vinculada por `device_id`.

### 3.3 Módulo `FileSystemAdapter`
Responsable de:
- normalizar rutas,
- detectar cambios reales del sistema de archivos,
- preparar eventos de modificación, borrado, renombrado, mover, copiar,
- leer y escribir archivos bajo el directorio sincronizado,
- aplicar operaciones de conflicto y renombrado local.

Interfaces obligatorias:
- `watchTree(rootPath)`
- `readFile(path)`
- `writeFile(path, bytes)`
- `delete(path)`
- `rename(oldPath, newPath)`
- `copy(src, dst)`
- `hashFile(path)`

### 3.4 Módulo `LocalMetadataStore`
Responsable de:
- mantener la base de metadatos local con SQLite,
- conservar estado de archivos, cola, sesiones y conflictos,
- persistir estado de sincronización para recuperación tras reinicio.

Estado mínimo que debe guardar:
- files
- sync_queue
- conflicts
- sessions
- audit_log

### 3.5 Módulo `SyncEngine`
Responsable de:
- recibir eventos de cambios locales,
- generar operaciones de sincronización,
- ordenar acciones por prioridad,
- decidir cuándo hacer pull remoto y cuándo push local,
- reintentar fallos en cola,
- ejecutar resolución de conflicto cuando el usuario confirma.

### 3.6 Módulo `NetworkClient`
Responsable de:
- encapsular llamadas REST/JSON al backend,
- generar JSON con `idempotency_key`,
- manejar timeouts, 401, 403, 409, 429, 5xx,
- reintentar según política de backoff,
- transformar respuestas del servidor en eventos internos de la app.

### 3.7 Módulo `ConflictManager`
Responsable de:
- identificar conflicto real,
- proponer resolución con regla por defecto de `modified_at` más reciente,
- permitir al usuario conservar la alternativa en copia o renombrado,
- confirmar la resolución y ejecutar la operación local.

### 3.8 Módulo `UIStateController`
Responsable de:
- exponer el estado global del cliente,
- transmitir los estados a la capa visual,
- mostrar errores, progreso y conflicto,
- permitir acciones manuales del usuario.

### 3.9 Módulo `AuditLogger`
Responsable de:
- registrar eventos del flujo local,
- guardar `ts`, `event`, `device_id`, `user_id`, `result`, `message`,
- soportar rotación y retención operativa.

## 4. Modelo de datos local

La metadata local debe reflejar exactamente la semántica del servidor, aunque como vista derivada de la verdad remota.

### 4.1 Entidades mínimas

```sql
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
  status TEXT NOT NULL,
  last_sync_version INTEGER DEFAULT 0,
  deleted_at INTEGER
);

CREATE TABLE sync_queue (
  queue_id TEXT PRIMARY KEY,
  file_id TEXT NOT NULL,
  operation TEXT NOT NULL,
  status TEXT NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0,
  idempotency_key TEXT NOT NULL,
  payload_json TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  last_error TEXT
);

CREATE TABLE conflicts (
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

CREATE TABLE sessions (
  session_id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  device_id TEXT NOT NULL,
  token_hash TEXT NOT NULL,
  expires_at INTEGER NOT NULL,
  revoked_at INTEGER,
  status TEXT NOT NULL
);

CREATE TABLE audit_log (
  audit_id TEXT PRIMARY KEY,
  user_id TEXT,
  device_id TEXT,
  event_name TEXT NOT NULL,
  payload_json TEXT,
  created_at INTEGER NOT NULL
);
```

### 4.2 Estados de archivo

Los estados mínimos del cliente deben ser:
- `pending`: cambio detectado, lista para enviar.
- `synced`: versión actual válida en servidor.
- `conflict`: conflicto detectado y necesita resolución del usuario.
- `quarantined`: error persistente de integridad o resolución inválida.
- `deleted`: archivo eliminado en la vista actual.

## 5. Flujo operativo del cliente

### 5.1 Arranque

1. La app carga configuración y entorno del sistema.
2. Inicializa SQLite local.
3. Revisa la cola persistente y reanuda operaciones pendientes.
4. Intenta restaurar la sesión activa si existe.
5. Si la sesión no es válida, requiere login.
6. Inicia polling de cambios del servidor.

### 5.2 Login

1. El usuario entra credenciales precreadas.
2. `Auth` envía `POST /v1/auth/login` con `device_id`.
3. El servidor valida identidad y permisos.
4. El cliente guarda la sesión en almacenamiento seguro del sistema.
5. El cliente inicia el ciclo de sincronización.

### 5.3 Detección local de cambios

Se activa por:
- creación de archivo,
- edición de archivo,
- borrado,
- renombrado,
- mover,
- copiar,
- cambios de contenido con timestamp actualizado.

Política del cliente:
- debounce por archivo de ~1 segundo,
- deduplicación por path + hash + timestamp,
- transformación a evento de sincronización,
- persistencia inmediata en `sync_queue` antes del envío.

### 5.4 Pull remoto (polling)

Cada 30 segundos el cliente ejecuta:
- `GET /v1/sync/diff?since={last_server_seq}`
- recibe cambios pendientes por usuario/dispositivo.
- ordena las operaciones por `server_seq`.
- aplica el delta localmente solo si la operación es segura y consistente.

Reglas:
- si el delta es un upload del mismo archivo, se valida el `checksum`;
- si hay un conflicto local, se marca como `conflict` y no se sobrescribe;
- si hay una operación de delete, no se borra el archivo si el usuario tiene una versión alternativa en conflicto.

### 5.5 Push local

Antes de enviar una operación:
1. se valida que el archivo exista,
2. se calcula `checksum` y `size_bytes`,
3. se normaliza `relative_path`,
4. se genera `idempotency_key`,
5. se guarda en la cola persistente,
6. se envía por REST,
7. el servidor confirma o devuelve error.

### 5.6 Resolución de conflicto local

Cuando el servidor indica conflicto o el cliente detecta ambos lados cambiados:
- se marca el archivo como `conflict`;
- se conserva la versión alternativa como copia o se renombra;
- la versión canónica queda según la regla de la más reciente;
- el usuario decide:
  - aceptar la versión más reciente,
  - conservar la versión alternativa,
  - renombrarla para revisión,
  - mantener ambas versiones.

No está permitido sobrescribir silenciosamente un archivo con la resolución automática sin confirmación del usuario.

## 6. Flujo de control interno

### 6.1 Orden de prioridad

1. autenticación y sesión,
2. reintento de cola local,
3. lectura de delta remoto,
4. push de cambios locales,
5. resolución manual de conflictos,
6. limpieza operativa.

### 6.2 Reintentos

La política del cliente es:
- backoff exponencial con jitter,
- máximo 5 intentos por operación,
- clasificación:
  - reintentable: timeout, 408, 429, 5xx,
  - terminal: 400, 401, 403, 404, 422.

Si es terminal:
- no se reintenta automáticamente,
- se marca el archivo como `error` o `quarantined`,
- se notifica al usuario.

## 7. Estado visible para la UI

La aplicación debe mostrar información de estado para todo el árbol sincronizado.

### 7.1 Estados globales
- `Sincronizado`
- `Sincronizando`
- `Con conflictos`
- `Error`
- `Pausado`

### 7.2 Estados por archivo
- nombre
- ruta relativa
- tamaño
- estado
- checksum
- fecha de última sincronización
- dispositivo origen
- cantidad de operaciones pendientes

### 7.3 Estados por dispositivo
- nombre del dispositivo
- sesión activa/inactiva
- último sync
- conexión estable o degradada

### 7.4 Acciones del usuario
- `Sincronizar ahora`
- `Reintentar`
- `Pausar`
- `Resolver conflicto`
- `Ver historial`
- `Guardar copia alternativa`

## 8. Diferencias por plataforma

### 8.1 Windows / macOS / Linux (desktop)
- File watcher nativo del sistema operativo.
- Uso de paths del sistema.
- Persistencia local en `~/.syncfiles` o equivalente.
- Soporte de permisos del sistema de archivos.
- UI con ventana principal y panel de sincronización.

### 8.2 Android
- use of file access APIs and scoped storage,
- detector de cambios en directorios sincronizados,
- servicio en segundo plano para reintentar cuadras pendientes,
- almacenamiento seguro para token de sesión,
- notificaciones de conflictos y errores.

### 8.3 Compatibilidad funcional
- El mismo contrato de sincronización se debe cumplir en desktop y Android.
- Las diferencias son de integración con sistema operativo, no de lógica de negocio.

## 9. Seguridad del cliente

- Tokens y secretos en almacenamiento seguro nativo del sistema.
- TLS 1.3 obligatorio para toda comunicación.
- Rechazo de operación si la validación TLS falla.
- El cliente no puede asumir permisos ni ownership; solo refleja la respuesta del servidor.
- Los archivos temporales deben limpiarse tras cada operación.
- Logs deben excluir secretos y contenido sensible.

## 10. Observabilidad del cliente

Los eventos mínimos del cliente son:
- `auth.login_success`
- `auth.login_failed`
- `sync.file_uploaded`
- `sync.file_downloaded`
- `sync.conflict_detected`
- `sync.retry_scheduled`
- `sync.quarantined`

Cada evento requiere:
- `ts`
- `event`
- `user_id`
- `device_id`
- `result`
- `message`

## 11. Criterios de aceptación del cliente V1

La implementación del cliente queda aceptada cuando cumple lo siguiente:
- inicia sesión con cuentas precreadas,
- detecta cambios reales del sistema de archivos,
- persiste la cola antes de enviar,
- sincroniza con el servidor usando REST + polling,
- aplica reintentos con backoff,
- marca conflictos y permite resolución manual,
- conserva la alternativa del conflicto sin sobrescritura silenciosa,
- reanuda la cola tras reinicio,
- mantiene el estado visual del sistema para el usuario.

## 12. Evolución post-V1

Tras la entrega de V1, el cliente se prepara para:
- NAS y almacenamiento remoto,
- PostgreSQL como back-end de metadatos,
- E2E para contenido sensible,
- resolución de conflicto avanzada,
- WebSocket/gRPC para transporte más reactivo,
- soporte offline primero y sincronización de trabajo desconectado.

Este diseño de cliente deja claro que V1 debe ser estable, predecible y operable, con los conflictos manejados de forma explícita y con confirmación del usuario. 

