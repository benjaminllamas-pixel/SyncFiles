# Prompts de Implementación — SyncFiles V1

## Sesión 1: Fundamentos, modelos compartidos y base de datos
**Contexto:** Backend y cliente comparten contratos. Primero se cierra el modelo de datos y el schema SQLite para que ambos módulos compileen contra lo mismo.

**Prompt:**
> Implementa la base común y el schema V1 para SyncFiles.
>
> 1. Crea un crate `syncfiles-models` (o un módulo shared) con los contratos base: `User`, `Device`, `Session`, `FileEntry`, `SyncQueueEntry`, `Conflict`, `AuditEntry`, `DiffRequest`, `DiffResponse`, `ChangeEntry`, `UploadRequest`, `DownloadRequest`, `ResolveConflictRequest`.
> 2. En `syncfiles-server`, revisa `migrations/001_init.sql` y el código de `src/db.rs`, `src/models.rs`, `src/storage.rs`, `src/state.rs`, `src/config.rs`, `src/auth.rs`, `src/handlers.rs`, `src/main.rs`. Corrige cualquier desviación contra el contrato compartido.
> 3. Asegura que la inicialización de BD, migraciones y arranque del servidor funcionan de punta a punta.
> 4. Verifica que el cliente Rust en `syncfiles-client/src/{main.rs,config.rs,metadata.rs,auth.rs,network.rs,sync.rs,watcher.rs}` use exactamente los mismos nombres de tablas y contratos (ajusta el cliente si hace falta).
>
> Entregable: servidor y cliente compilan, `cargo check` pasa en ambos crates, y el schema está alineado.

---

## Sesión 2: Storage local, auth y API REST mínima
**Contexto:** El backend necesita persistir archivos y autenticar. Esta sesión se centra en el servidor y el flujo login→diff→upload→download→delete.

**Prompt:**
> Completa y valida el backend V1 de SyncFiles.
>
> 1. En `syncfiles-server/src/handlers.rs` y `syncfiles-server/src/auth.rs`, revisa login, logout, session_status, diff, upload, download, delete, rename, move, copy, resolve_conflict. Asegura validación estricta de `path_hash`, `idempotency_key`, y Authorization header.
> 2. Corrige el bug de `diff_handler`: `extract_session` sobre el JSON body no existe; el token debe venir del header `Authorization: Bearer <session_id>`.
> 3. En `syncfiles-server/src/storage.rs`, valida `LocalDiskStorageProvider`: normalización de rutas, creación de directorios padres, y manejo de errores IO.
> 4. En `syncfiles-server/src/db.rs`, implementa las consultas faltantes para rename/move/copy/conflicts/audit si aún no están, y revisa índices.
> 5. Escribe pruebas unitarias del storage y del handler de path_hash.
>
> Entregable: el servidor arranca, un cliente puede autenticar, y los endpoints CRUD responden correctamente.

---

## Sesión 3: Cliente desktop — App shell, sesión persistida y sync engine básico
**Contexto:** El cliente Rust/eframe necesita funcionar como prototipo usable. Esta sesión prioriza el flujo real de sincronización.

**Prompt:**
> Refactoriza el cliente Rust de SyncFiles para que sea funcional y seguro.
>
> 1. En `syncfiles-client/src/main.rs`, elimina la duplicación de `fn logout` y limpia el manejo de estado UI.
> 2. En `syncfiles-client/src/sync.rs`, corrige `push_local`: usa `std::fs::read` en lugar de `read_to_string` para no corromper binarios. Implementa reintentos con backoff exponencial usando la cola `sync_queue`.
> 3. En `syncfiles-client/src/metadata.rs`, alinea el schema local con el servidor (tablas, índices, UNIQUE constraints).
> 4. Integra el file watcher de `watcher.rs` con la sync queue: al detectar un cambio, encola la operación correspondiente (upload/delete) y dispara un ciclo de sincronización.
> 5. En `syncfiles-client/src/network.rs`, implementa el manejo de `idempotency_key` y retry ante 429/5xx.
>
> Entregable: el cliente desktop sincroniza archivos de verdad entre dos instancias contra el servidor.

---

## Sesión 4: Detección de conflictos y resolución guiada
**Contexto:** V1 requiere que los conflictos no se resuelvan silenciosamente; el usuario debe decidir.

**Prompt:**
> Implementa detección y resolución de conflictos en el cliente y servidor.
>
> 1. En `syncfiles-client/src/sync.rs`, después de `pull_remote`, compara `checksum` del cambio remoto contra el `checksum` local del mismo `path_hash`. Si difieren y ambos no están eliminados, marca conflicto en vez de sobrescribir.
> 2. En `syncfiles-client/src/metadata.rs`, agrega métodos para marcar conflicto, listar conflictos abiertos y resolver conflicto (copy/keep/rename).
> 3. En la UI de `main.rs`, agrega la vista `Conflicts` y un modal `ConflictResolve` que muestre checksums locales/remotos y permita elegir.
> 4. En `syncfiles-server/src/handlers.rs` y `syncfiles-server/src/db.rs`, implementa `POST /conflicts/resolve` y valida que la resolución preserve la alternativa elegida.
> 5. Asegura que el servidor emita `audit_log` por cada resolución.
>
> Entregable: al modificar el mismo archivo desde dos dispositivos, ambos ven el conflicto y lo resuelven sin pérdida de datos.

---

## Sesión 5: Cliente Android — Conexión real y sync básico
**Contexto:** El cliente Android Kotlin/Compose ya tiene estructura; hay que conectarlo con el backend y hacer el flujo end-to-end.

**Prompt:**
> Conecta el cliente Android con el backend SyncFiles.
>
> 1. Revisa `syncfiles-android/app/src/main/java/com/syncfiles/client/android/data/api/SyncFilesApi.kt`, `ApiClientFactory.kt`, `LocalFileSyncStore.kt`, `SyncQueueStore.kt`, `FileWatcher.kt`, `SessionStore.kt`, `Hashing.kt`, `SyncWorker.kt`, `SyncFilesApplication.kt`.
> 2. Implementa el flujo: login → guardar sesión → polling diff → upload/download → actualizar `LocalFileSyncStore`.
> 3. Usa SAF (`DocumentTree`) como `StorageProvider` en Android, mapeando rutas a `Uri` persistente.
> 4. Integra `SyncWorker` con cola persistente, backoff y recuperación tras muerte del proceso.
> 5. Asegura que `path_hash`, `checksum` y `idempotency_key` coincidan con el backend.
>
> Entregable: un archivo editado en Android aparece sincronizado en el servidor y en otro cliente.

---

## Sesión 6: Pruebas, CI/CD básico y release V1
**Contexto:** V1 cierra cuando hay pruebas, pipeline y paquetes entregables.

**Prompt:**
> Agrega pruebas automatizadas y empaquetado para V1.
>
> 1. En `syncfiles-server`, agrega tests de integración con `actix-web::test` para login, diff, upload, download, delete, rename, move, copy, conflict resolve.
> 2. En `syncfiles-client`, agrega tests unitarios para hashing, path normalization, backoff, idempotency, session renewal, y sync engine end-to-end contra un servidor en memoria o mock.
> 3. En `syncfiles-android`, agrega instrumented tests para login, sync diff y watcher.
> 4. Crea `.github/workflows/ci.yml` que haga `cargo check`, `cargo test` y build de binarios.
> 5. Define empaquetado por plataforma: `cargo build --release` para desktop, `.apk` para Android, y script de release.
>
> Entregable: pipeline verde, artefactos firmados, y checklist de release V1 cumplido.
