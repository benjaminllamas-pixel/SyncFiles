# Plan V1: Chunks de implementación listos para ejecutar

## Contexto actual
- **Rust server**: `LocalDiskStorageProvider`, handlers CRUD y auth implementados pero sin commit. Cambios en `handlers.rs`, `storage.rs`, `state.rs`, `config.rs`, `models.rs`, `main.rs`, `migrations/001_init.sql`.
- **Rust desktop client**: UI eframe + motor de sync + watcher + metadata store. Cambios en `main.rs`, `sync.rs`, `network.rs`, `auth.rs`, `config.rs`, `metadata.rs`, `watcher.rs` sin commit.
- **Android client**: Kotlin/Compose con login, diff, SAF upload, SyncWorker, cola local. Código presente pero con bugs críticos.
- **Shared models**: `syncfiles-models` con contratos de datos y schemas SQLite.

---

## Chunk 1 — Compilar workspace Rust y corregir errores base

Objetivo: asegurar que `syncfiles-models`, `syncfiles-server` y `syncfiles-client` compilen sin errores antes de continuar.

Acciones:
1. Revisar `syncfiles-models/Cargo.toml` y confirmar que `sha2` está declarado.
2. Ejecutar `cargo check` en `syncfiles-models`, `syncfiles-server` y `syncfiles-client`.
3. Corregir cualquier error de compilación en:
   - `syncfiles-models/src/lib.rs` (imports de `sha2` locales en funciones están bien, confirmar que no hay errores de módulo).
   - `syncfiles-server/src/models.rs` (re-export de `syncfiles_models::*`).
   - `syncfiles-server/src/handlers.rs` (uso de tipos re-exportados).
   - `syncfiles-server/src/storage.rs` (trait `StorageProvider` y `async_trait`).
   - `syncfiles-client/src/main.rs` (referencias a `SyncFilesUi` y módulos).
   - `syncfiles-client/src/sync.rs` (referencias a `MetadataStore` y `FileEntry`).
4. Si hay dependencias faltantes en `Cargo.toml`, agregarlas.

Criterio de aceptación: `cargo check` pasa en los tres crates.

---

## Chunk 2 — Commit de cambios pendientes del servidor

Objetivo: dejar el servidor en un estado limpio y funcional.

Acciones:
1. Hacer `git add` de los archivos modificados del servidor:
   - `syncfiles-server/src/storage.rs`
   - `syncfiles-server/src/handlers.rs`
   - `syncfiles-server/src/state.rs`
   - `syncfiles-server/src/config.rs`
   - `syncfiles-server/src/models.rs`
   - `syncfiles-server/src/main.rs`
   - `syncfiles-server/migrations/001_init.sql`
   - `syncfiles-server/Cargo.toml`
   - `syncfiles-server/Cargo.lock`
2. Ejecutar `cargo test` en `syncfiles-server` y confirmar que los tests de `storage.rs` pasan.
3. Hacer commit con mensaje: `feat(server): LocalDiskStorageProvider, handlers CRUD y estado` (o mensaje acorde al diff).
4. Verificar que el servidor inicia con `cargo run` y responde en `http://127.0.0.1:8080/api/v1`.

Criterio de aceptación: servidor compila, tests pasan, servidor corriendo responde a `/api/v1/session/status` con 401.

---

## Chunk 3 — Commit de cambios pendientes del cliente desktop

Objetivo: dejar el cliente Rust desktop en un estado limpio.

Acciones:
1. Hacer `git add` de los archivos modificados del cliente:
   - `syncfiles-client/src/main.rs`
   - `syncfiles-client/src/sync.rs`
   - `syncfiles-client/src/network.rs`
   - `syncfiles-client/src/auth.rs`
   - `syncfiles-client/src/config.rs`
   - `syncfiles-client/src/metadata.rs`
   - `syncfiles-client/src/watcher.rs`
   - `syncfiles-client/Cargo.toml`
   - `syncfiles-client/Cargo.lock`
2. Ejecutar `cargo check` y `cargo test` en `syncfiles-client`.
3. Hacer commit con mensaje: `feat(client): UI eframe, SyncEngine, watcher y metadata store`.
4. Verificar que el binario se compila con `cargo build`.

Criterio de aceptación: cliente compila y binario se genera.

---

## Chunk 4 — Corregir bugs críticos del cliente Android

Objetivo: dejar el cliente Android en un estado compilable y funcional para pruebas básicas.

Acciones:
1. **`SyncFilesApi.kt`**:
   - Agregar `data class DownloadRequest(...)` con los campos `session_id`, `device_id`, `file_id`, `path_hash`, `idempotency_key`.
   - Cambiar `@POST("sync/download") suspend fun download(@Body request: DeleteRequest)` por `@POST("sync/download") suspend fun download(@Body request: DownloadRequest)`.
2. **`SyncWorker.kt`**:
   - En `uploadFile`, reemplazar `val text = String(content, Charsets.UTF_8)` por codificación base64: `val text = Base64.encodeToString(content, Base64.NO_WRAP)`.
   - Agregar `import android.util.Base64`.
3. **`AndroidManifest.xml`**:
   - Agregar permiso: `<uses-permission android:name="android.permission.SCHEDULE_EXACT_ALARM" />`.
   - Agregar el worker dentro de `<application>`:
     ```xml
     <service
         android:name="androidx.work.impl.foreground.SystemForegroundService"
         android:foregroundServiceType="dataSync"
         android:exported="false" />
     ```
4. **`HomeViewModel.kt` y `SyncWorker.kt`**:
   - En `resolveRootFile` y `localRoot`, reemplazar la lógica `DocumentFile.fromTreeUri(...).uri.path` por un fallback seguro a `File(context.filesDir, "sync_root")` cuando `path` sea nulo o inválido.
5. Compilar con `./gradlew assembleDebug` (o Gradle disponible) y confirmar que no hay errores de compilación.

Criterio de aceptación: Android compila, APK debug se genera, upload no corrompe archivos binarios.

---

## Chunk 5 — Conectar watcher, corregir flujo de login y sync en desktop

Objetivo: que el cliente Rust desktop tenga sync real y login confiable.

Acciones:
1. **`syncfiles-client/src/main.rs`**:
   - Corregir `login`: en lugar de `std::thread::sleep(Duration::from_millis(800))` y setear `logged_in = true` a ciegas, usar un `oneshot` o `Arc<Mutex<Option<String>>>` para capturar el resultado de `login_raw` y solo setear `logged_in = true` cuando sea exitoso. Si falla, setear `login_error`.
   - Corregir `toggle_pause`: setear `self.syncing = !self.paused` en lugar de siempre `false`.
   - En `start_sync_engine`, pasar `device_id` y `user_id` correctos desde la sesión activa.
2. **`syncfiles-client/src/sync.rs`**:
   - En `pull_remote`, además de `upload` y `delete`, manejar `download` para cambios remotos que traen contenido.
   - En `push_local`, usar `path_hash` consistente con el server (ya está).
3. **`syncfiles-client/src/watcher.rs`**:
   - Exponer un método `start` que devuelva el `RecommendedWatcher` y un callback que inserte en la cola local de `MetadataStore` con estado `pending`.
   - Agregar debounce de 2 segundos por archivo para evitar eventos duplicados.
4. **`syncfiles-client/src/main.rs` (o nuevo módulo `watcher.rs`)**:
   - Inicializar `FileWatcher` cuando haya sesión activa y `sync_root` válido.
   - Al detectar cambio, marcar archivo como `pending` en metadata store.

Criterio de aceptación: login muestra error si las credenciales son inválidas; cambios locales aparecen en cola como `pending`; sync_engine procesa push/pull.

---

## Chunk 6 — Detección real de conflictos y flujo de resolución

Objetivo: evitar sobrescritura silenciosa y permitir resolución guiada por el usuario.

Acciones:
1. **Server `handlers.rs` (`upload_handler`)**:
   - Antes de escribir, consultar `crate::db::get_files_by_path_hash(&state.pool, &session.user_id, &expected_path_hash)`.
   - Si existe y el `checksum` difiere del checksum entrante, devolver `409 Conflict` con un cuerpo que indique `conflict_id` generado y `alternative_preserved_path`.
   - Registrar el conflicto en DB con `crate::db::mark_conflict`.
2. **Server `handlers.rs` (`resolve_conflict_handler`)**:
   - Ya existe; asegurar que acepte `decision` en `["keep_local", "keep_remote", "rename", "copy"]`.
   - Preservar la alternativa en storage cuando `preserve_alternative = true`.
3. **Cliente Rust `sync.rs` (`push_local`)**:
   - Al subir, si el server responde con `409`, marcar el archivo local como `conflict` en metadata store y registrar el `conflict_id`.
4. **Cliente Android `HomeViewModel.kt`**:
   - Al hacer `syncNow`, si la respuesta contiene conflictos (agregar campo `conflicts` a `DiffResponse` o endpoint separado), exponerlos en la UI.
   - Agregar botones de resolución simple (`keep_local`, `keep_remote`) que llamen a `api.resolveConflict(...)`.
5. **UI Desktop `main.rs`**:
   - Mostrar lista de conflictos en vista `Conflicts` con botones de resolución.

Criterio de aceptación: al editar el mismo archivo desde dos dispositivos, el segundo upload recibe 409 y el usuario puede resolver sin perder datos.

---

## Chunk 7 — Pruebas, validación end-to-end y cierre de V1

Objetivo: confirmar que los flujos críticos funcionan y preparar release.

Acciones:
1. Ejecutar `cargo test` en `syncfiles-models`, `syncfiles-server`, `syncfiles-client`.
2. Ejecutar `cargo run` en server y cliente Rust en dos terminales; probar:
   - Login con usuario demo.
   - Upload de archivo de texto y binario (imagen pequeña).
   - Download desde otro cliente.
   - Delete y rename.
   - Modificación concurrente para disparar conflicto.
3. En Android:
   - Instalar APK debug en emulador o dispositivo.
   - Probar login, selección de carpeta SAF, upload manual, syncNow.
4. Corregir cualquier bug encontrado en pruebas.
5. Actualizar `docs/51-backlog-implementacion-v1.md` tachando tareas completadas.
6. Hacer commit final de V1 con mensaje `chore: cierre V1 - sync funcional y pruebas básicas`.

Criterio de aceptación: server + cliente desktop + cliente Android sincronizan archivos sin corrupción; conflictos se resuelven con confirmación del usuario; cola recupera estado tras reinicio.

---

## Riesgos y contingencias

- **Compilación cruzada Android**: si no hay SDK/NDK configurado, validar solo con `cargo check` en crates Rust y código Kotlin en Android Studio.
- **SAF path nulo**: usar siempre fallback a `context.filesDir` cuando `DocumentFile.fromTreeUri` retorne `null`.
- **Session expirada**: implementar refresh automático en cliente antes de cada sync cycle.
- **Base64 en uploads grandes**: para archivos > 10MB, evaluar streaming; en V1 base64 es aceptable para archivos pequeños.
