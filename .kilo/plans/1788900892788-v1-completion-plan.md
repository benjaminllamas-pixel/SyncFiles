# Plan: Completar Sesiones Incompletas de SyncFiles V1

## Contexto

- Las sesiones 1 y 2 del plan original (`1788893078130-implementation-prompts.md`, `1788897386246-session-2-backend-storage-auth-api.md`) quedaron incompletas por límite de sessión.
- El código actual ya tiene base sólida:
  - `syncfiles-models` (crate compartido con models + schema SQL client/server).
  - `syncfiles-server` (actix-web): handlers, db, auth, storage, state, config, main, lib.
  - `syncfiles-client` (eframe desktop): main, auth, config, metadata, network, sync, watcher.
  - `syncfiles-android` (Kotlin/Compose): api, stores, SyncWorker, UI.
- Estado real (`git status`): cambios no committeados en client y server; `syncfiles-models/` y `syncfiles-server/src/lib.rs` sin trackear.
- Huecos detectados: sin `tests/` de integración, sin `.github/workflows/ci.yml`, vistas de UI incompletas (Conflicts/Devices), Android con bugs de `path_hash` y lectura binaria.

## Objetivo

Llevar SyncFiles V1 a estado estable, runnable y testable de punta a punta, en sesiones acotadas (una Fase por sessión como máximo).

## Alcance

- **IN:** server Rust, cliente desktop Rust, modelos compartidos, correcciones menores en Android, pruebas y CI.
- **OUT:** E2E, NAS, PostgreSQL, Kafka, empaquetado Android (.apk), UI pulida más allá de lo mínimo.

## Tareas ordenadas

### Fase 0 — Validación actual (sin cambios de lógica)
1. Ejecutar `cargo check` en `syncfiles-models`, `syncfiles-server`, `syncfiles-client`.
2. Corregir cualquier error de compilación que aparezca (ej. alinear `Session`/`SessionRecord` usados en `sync.rs` con los definidos en `metadata.rs`).
3. Verificar que `compute_path_hash` y `normalize_relative_path` coincidan entre server y client.

### Fase 1 — Backend hardening
4. `diff_handler`: confirmar que los archivos con `status='deleted'` y `deleted_at` reciente se devuelvan con operación `"delete"`.
5. `resolve_conflict_handler`: completar preservación de alternativa (copy/keep/rename) y asegurar `audit_log` con evento `conflict.resolved`.
6. Verificar que `rename`/`move`/`copy` registren y usen `idempotency_key` (ya lo hacen; solo validez).
7. Crear `syncfiles-server/tests/integration_tests.rs` con `actix-web::test`: flujo login → diff → upload → download → delete → rename → move → copy → resolve_conflict, SQLite en memoria + storage temporal con `tempfile`.

### Fase 2 — Cliente desktop
8. `sync.rs` `push_local`: usar `std::fs::read` (ya lo hace) y completar reintentos con backoff exponencial usando la cola `sync_queue` (`increment_attempts` + sleep).
9. `metadata.rs`: verificar que `upsert_file` no incluya columna `content` (schema client no la tiene) y que `get_files_by_status` devuelva `content: None`.
10. Integrar `watcher.rs` con la sync queue: al detectar cambio, encola upload/delete y dispara un ciclo de sincronización.
11. `network.rs`: añadir loop de reintentos ante 429/5xx usando el flag `retryable` de `ApiError`.
12. UI `main.rs`: implementar vista `Conflicts` y modal `ConflictResolve` (actualmente `draw_content` dice "Vista no implementada aún").

### Fase 3 — Android (correcciones menores, sin empaquetado)
13. `SyncWorker.kt`: corregir `path_hash` (usa `idempotencyKey` como path_hash — error; debe ser hash de la ruta relativa) y lectura binaria (`String(content, UTF_8)` corrompe binarios; usar base64).
14. Conectar `SyncWorker` con `LocalFileSyncStore` y `HomeViewModel` para el flujo login → diff → upload.
15. Confirmar uso de SAF `DocumentTree` como `StorageProvider` mapeando rutas a `Uri` persistente.

### Fase 4 — Pruebas y release V1
16. Crear `.github/workflows/ci.yml`: `cargo check`, `cargo test` en server y client, build release.
17. Ejecutar `cargo test` en `syncfiles-server` y `syncfiles-client`; dejar pipeline verde.
18. Definir empaquetado: `cargo build --release` para desktop (Android requiere Gradle, fuera de alcance esta sessión).

## Restricciones de límites (para futuras sesiones)

- Una Fase por sessión como máximo.
- No tocar `syncfiles-models` una vez estable (solo correcciones de compilación).
- No introducir nuevas dependencias sin verificar que el workspace ya las use.

## Validación

- `cargo check` pasa en los 3 crates.
- `cargo test` en `syncfiles-server` pasa, incluidas las nuevas integration tests.
- Servidor arranca con `cargo run` y responde a login/diff/upload/download/delete/rename/move/copy/conflicts.
- Cliente desktop compila y muestra dashboard.

## Preguntas abiertas (out of scope si no se responden)

- ¿Multi-tenant? No — V1 es single-user por account.
- ¿Storage configurable por entorno? Ya lo es vía `SF_STORAGE_ROOT`.