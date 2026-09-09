# Sesión 2: Storage local, auth y API REST mínima

## Contexto
El backend necesita persistir archivos y autenticar. Esta Sesión se centra en el servidor y el flujo login→diff→upload→download→delete. La Sesión 1 ya cerró los modelos compartidos y el schema.

## Estado actual (código existente)
- `syncfiles-server/src/handlers.rs`: Todos los endpoints implementados (login, logout, session_status, diff, upload, download, delete, rename, move, copy, resolve_conflict)
- `syncfiles-server/src/auth.rs`: login, validate_session, logout implementados
- `syncfiles-server/src/storage.rs`: `LocalDiskStorageProvider` con normalización de rutas, CRUD, y tests unitarios
- `syncfiles-server/src/db.rs`: Consultas para rename/move/copy/conflicts/audit implementadas
- `syncfiles-server/migrations/001_init.sql`: Schema SQL completo
- Tests unitarios inline en `handlers.rs` (path_hash) y `storage.rs` (CRUD)

## Lanzar ( preparación )

1. **Revisar validación de `idempotency_key` en el servidor**
   - En `syncfiles-server/src/handlers.rs`, los handlers upload/delete/rename/move/copy reciben `idempotency_key` pero no lo almacenan ni usan para deduplicación.
   - Agregar una tabla `idempotency_keys` o usar el `sync_queue` para registrar las keys procesadas.
   - Antes de procesar una operación, verificar si la `idempotency_key` ya fue usada y devolver la respuesta anterior si es así.

2. **Validar que `session_id` del request coincida con el token de autorización**
   - En `syncfiles-server/src/handlers.rs`, los handlers de upload/download/delete/rename/move/copy extraen el token del header `Authorization` pero no comparan el `session_id` del body JSON contra el token.
   - Agregar validación: si `req.session_id != token_del_header`, rechazar con 401.

3. **Completar `resolve_conflict_handler`**
   - Actualmente solo marca la conflicto como resuelto en la BD.
   - Agregar lógica para preservar la alternativa (copy/keep/rename) según la decisión del usuario.
   - Emitir `audit_log` con el evento `conflict.resolved`.

4. **Revisar `diff_handler` para incluir eliminaciones**
   - Actualmente `get_files_modified_since` filtra por `modified_at > since`, pero los archivos eliminados (status='deleted') podrían no tener modified_at actualizado.
   - Asegurar que los archivos con `deleted_at` reciente sean incluidos en la respuesta de diff con operación "delete".

5. **Verificar cobertura de índices en `db.rs`**
   - Revisar que las consultas usen los índices definidos en el schema (idx_files_user_path, idx_sync_queue_user_status, etc.).
   - Agregar índices faltantes si los hay.

## Ejecutar ( implementación )

1. **Escribir pruebas de integración con `actix-web::test`**
   - Archivo: `syncfiles-server/tests/integration_tests.rs`
   - Flujo completo: login → diff → upload → download → delete → rename → move → copy → resolve_conflict
   - Usar una base de datos SQLite en memoria y un storage temporal con `tempfile`
   - Validar:
     - Login exitoso con credenciales válidas
     - Login fallido con contraseña incorrecta
     - Upload con path_hash incorrecto es rechazado
     - Upload sin Authorization header es rechazado
     - Download de archivo existente retorna contenido correcto
     - Download de archivo inexistente returns 404
     - Delete marca el archivo como eliminado
     - Rename actualiza relative_path y path_hash
     - Move y copy funcionan correctamente
     - Diff retorna cambios después de un upload
     - Resolve_conflict marca como resuelto y emite audit

2. **Validar que el cliente Rust autentique correctamente**
   - En `syncfiles-client/src/auth.rs`, el método `login_raw` usa `client.login()` que envía el token en el header correcto.
   - Verificar que `syncfiles-client/src/network.rs` envía `Authorization: Bearer <session_id>` en todas las peticiones autenticadas.
   - Ejecutar `cargo check` en ambos crates para confirmar compilación.

3. **Verificar que el servidor arranca de punta a punta**
   - Configurar variables de entorno: `SF_BIND_ADDRESS`, `SF_DATABASE_URL`, `SF_STORAGE_ROOT`, `SF_USER_0_EMAIL`, `SF_USER_0_PASSWORD`, `SF_USER_0_ID`
   - Ejecutar `cargo run` en `syncfiles-server`
   - Usar `curl` o el cliente para verificar los endpoints básicos.

4. **Validar `LocalDiskStorageProvider`**
   - Confirmar que la normalización de rutas funcione (ya hay tests)
   - Confirmar que la creación de directorios padres funcione
   - Confirmar que los errores IO se propaguen correctamente

5. **Verificar alineación de schema cliente-servidor**
   - El cliente usa `syncfiles_models::schema::CLIENT_INIT_SQL`
   - El servidor usa `syncfiles_models::schema::SERVER_INIT_SQL`
   - Ambos comparten los mismos nombres de tablas y columnas
   - Verificar que los UNIQUE constraints estén alineados

## Entregable
El servidor arranca, un cliente puede autenticar, y los endpoints CRUD responden correctamente. Las pruebas de integración cubren el flujo completo.