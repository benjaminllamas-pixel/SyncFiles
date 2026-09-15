# Backlog de Implementación V1

## 1. Objetivo

Este backlog convierte la arquitectura aprobada en un plan práctico de implementación para la versión V1 del producto.

La prioridad es entregar una versión estable, segura y testable con:
- SQLite como base local y operativa,
- almacenamiento local como capa inicial,
- REST/JSON más polling,
- sincronización bidireccional con conflictos guiados por usuario,
- estructura modular para crecer hacia V2 sin reescribir la base.

## 2. Principio de ejecución

Se implementará en orden de dependencia real:
1. base común y contratos,
2. persistencia y almacenamiento local,
3. autenticación y API server,
4. cliente + sincronización + conflictos,
5. pruebas y release.

## 3. Estado de ejecución

> Actualizado tras la validación end-to-end real (server + CLI + cliente desktop)
> ejecutada contra binarios compilados, no solo revisión de código.

### Epic 1: Fundamentos y contrato de sistema

#### Tarea 1.1 — Base del proyecto — COMPLETADA
- [x] Crear estructura de repositorio por dominio.
- [x] Definir contratos de datos compartidos (`syncfiles-models`).
- [x] Definir configuración por entorno (env vars `SF_*`).
- [x] Definir convenciones de naming y errores (`ApiResponse`/`ApiError`).

Criterio de aceptación: cumplido — los 4 crates (models, server, cli, client)
construyen sin duplicación funcional.

#### Tarea 1.2 — Modelo de datos base V1 — COMPLETADA
- [x] Migraciones de SQLite (`schema::SERVER_INIT_SQL` / `CLIENT_INIT_SQL`).
- [x] Modelado de usuarios, dispositivos, sesiones, archivos, cola, conflictos y auditoría.
- [x] Índices y constraints de negocio (verificado en runtime: `sync_queue`,
  `idempotency_keys`, `conflicts`, `audit_log` operativos).

Criterio de aceptación: cumplido — la base soporta auth, sync y auditoría mínima.

### Epic 2: Backend y API

#### Tarea 2.1 — Autenticación y sesiones — COMPLETADA
- [x] Login, logout, token de sesión (Bearer = session_id).
- [x] Validación de `device_id`.
- [x] Revocación y expiración (24 h, `expires_at` verificado en DB).
- [x] API de estado de sesión.

Criterio de aceptación: cumplido — sesión inválida rechazada con 401
`UNAUTHORIZED` ("Sesión inválida o expirada"); sesión válida rechaza
`session_id` no coincidente con el token.

#### Tarea 2.2 — Storage provider local — COMPLETADA
- [x] `LocalDiskStorageProvider` con aislamiento por usuario.
- [x] Rutas normalizadas (rechaza `..`, segmentos vacíos y `.`).
- [x] Validación de checksums (SHA-256 verificado byte a byte en download).
- [x] Guardar/leer archivos y metadatos de forma segura.

Criterio de aceptación: cumplido — upload binario de 64 KiB descargado con
SHA-256 idéntico; copias de conflicto preservadas en `users/{id}/conflicts/`.

#### Tarea 2.3 — REST API de sincronización — COMPLETADA
- [x] `POST /api/v1/sync/diff`
- [x] `POST /api/v1/sync/upload`
- [x] `POST /api/v1/sync/download`
- [x] `POST /api/v1/sync/delete`
- [x] `POST /api/v1/sync/rename`
- [x] `POST /api/v1/sync/move`
- [x] `POST /api/v1/sync/copy`
- [x] `POST /api/v1/conflicts/resolve`
- [x] Manejo de `idempotency_key`, 401/403/409/429/5xx.

Criterio de aceptación: cumplido — flujo completo verificado con CLI + cliente
desktop; conflicto 409 con `conflict_id` y `alternative_preserved_path`;
resolución `keep_local` auditada (`conflict.resolved` en `audit_log`).

Notas de la validación (bugs corregidos en esta fase):
- sqlx 0.7 no crea la DB por defecto: `state.rs` ahora usa
  `SqliteConnectOptions::create_if_missing(true)` cuando el archivo no existe
  (antes el server no podía arrancar en un entorno limpio).
- `rename`/`move` derivan `old_path` desde la DB por `file_id` cuando el
  cliente la envía vacía (antes fallaba con `STORAGE_ERROR: Invalid path`).

### Epic 3: Cliente

#### Tarea 3.1 — App shell y configuración — COMPLETADA
- [x] Inicialización de app (eframe/egui).
- [x] Manejo de configuración (`config.json` + env `SF_*`).
- [x] Almacenamiento local seguro (SQLite local en data dir).
- [x] Lifecycle del cliente.
- [x] Sesión persistida (auto-login verificado tras reinicio).

Criterio de aceptación: cumplido — el cliente arranca, recupera sesión
activa de la DB local y continúa sincronizando tras reinicio.

#### Tarea 3.2 — Watcher y cola local — COMPLETADA
- [x] Detector de cambios del sistema de archivos (notify/kqueue).
- [x] Debounce por archivo (2 s).
- [x] Normalización de rutas (rutas canónicas + relativas).
- [x] Persistencia de `sync_queue`.
- [x] Recuperación de cola tras reinicio (`resume_queued_ops`).

Criterio de aceptación: cumplido — un cambio local queda en cola y se
recupera tras un reinicio abrupto (verificado: operación encolada pre-crash
se reanuda y sube al server en el primer ciclo post-reinicio).

Notas de la validación (bugs corregidos en esta fase):
- El `RecommendedWatcher` devuelto por `FileWatcher::start` se descartaba:
  la UI ahora retiene el handle en `watcher_handle` (sin esto kqueue moría
  inmediatamente y ningún evento llegaba).
- macOS resuelve `/tmp` como `/private/tmp`: el watcher ahora canonicaliza
  root y rutas de evento antes de filtrar y calcula la ruta relativa
  (antes todos los eventos se descartaban por el filtro de prefijo).
- `push_local` ahora recalcula checksum y tamaño desde el contenido real
  (el `enqueue` del watcher dejaba checksum vacío, lo que generaba 409
  falsos en la segunda subida del mismo archivo).

#### Tarea 3.3 — Sincronización funcional — COMPLETADA
- [x] Polling de cambios (`/sync/diff` con `since = last_server_seq`).
- [x] Pull remoto y push local (ciclo pull→push verificado en logs).
- [x] Control de estado `queued`, `in_flight`, `retry`, `synced`,
  `conflict`, `failed` (cola con `queued`/`retry`/`done` verificada en DB).

Criterio de aceptación: cumplido — la app sincroniza archivos entre cliente
y servidor sin pérdida de intención (watcher→cola→upload→server verificado
end-to-end con contenido íntegro).

Nota de la validación: `SyncClient::request` y `upload` generaban URLs con
doble slash (`//api/v1/...`) que Actix resolvía como 404 — el cliente solo
podía hacer push y nunca pull. Corregido en `network.rs`; todos los
endpoints ahora responden 200 desde el cliente desktop.

Ampliación (pull de rename/move/copy + deletes locales): el server ahora
mantiene un `change_log` (fuente de verdad del diff) donde cada mutación
anexa una entrada (`upload`/`delete`/`rename`/`move`/`copy` con `old_path`);
el `diff` consulta ese log por cursor de `seq` (no por timestamp — antes
`since` se comparaba contra `modified_at`, re-descargando todo en cada
ciclo). El cliente desktop aplica renames/moves remotos con `old_path` como
fallback, detecta deletes locales en `reconcile` (estados `synced`/`pending`
sin archivo en disco, ignorando dotfiles) y los propaga con `NOT_FOUND`
como éxito idempotente. El cliente Android aplica `rename`/`move`/`copy`
en `SyncWorker`. Backfill de arranque: si `change_log` está vacío se crea
una entrada por archivo existente (una sola vez). Verificado con
`scripts/e2e-rename-move-copy.sh` (27 checks).

#### Tarea 3.4 — Resolución de conflicto — COMPLETADA
- [x] Detectar conflictos reales (checksum mismatch → 409).
- [x] Preservar alternativa local/remota (`.conflict_{id}_{ts}`).
- [x] Mostrar confirmación de resolución (vista "Conflictos" en la UI).
- [x] Soportar copiar, renombrar o conservar versión por decisión del
  usuario (`keep`/`keep_local`/`keep_remote`/`rename`/`copy`).

Criterio de aceptación: cumplido — no hay sobrescritura silenciosa; el
usuario decide explícitamente (resolución `keep_local` verificada vía API
con alternativa preservada y audit).

### Epic 4: Validación y release — COMPLETADA

#### Tarea 4.1 — Pruebas unitarias — COMPLETADA
- [x] Hashing, rutas, conflict policy (6 tests en `syncfiles-server`: storage
  CRUD, paths inválidos, normalización, path_hash/relative_path).
- [x] `syncfiles-client`: 31 tests — `network.rs` (9: login/logout, session
  status, upload texto/binario con checksum verificado, 409 conflicto, flag
  retryable por 5xx/4xx, diff por cursor), `sync.rs` (6: scan con prefijo de
  subdirectorios — bugfix real: las claves perdían la carpeta padre —,
  reconcile con nuevos/modificados/deletes, dotfiles no se marcan deleted,
  apply_renaming con fallback old_path, apply_download/apply_delete),
  `metadata.rs` (14: cola persistente, reanudación tras reinicio, conflictos,
  deletes locales) y `config.rs` (2: round-trip, defaults).
- [x] `syncfiles-models`: 15 tests inline (hashes, normalización,
  serialización de payloads).

#### Tarea 4.2 — Pruebas de integración — COMPLETADA
- [x] Flujo completo cliente-servidor validado manualmente (login, upload
  texto/binario, download con verificación de checksum, rename, delete,
  conflicto 409, resolución, reinicio de server y de cliente).
- [x] Tests automatizados: 22 tests en `syncfiles-server/tests/api_endpoints.rs`
  (upload/download/delete, conflicto 409, resolución, rename/move/copy, diff
  por cursor seq, revocación de dispositivos) + 8 en `static_dashboard.rs`.
- [x] Scripts E2E: `scripts/e2e-test.sh`, `scripts/e2e-phase5.sh` (17 checks),
  `scripts/e2e-rename-move-copy.sh` (27 checks).

#### Tarea 4.3 — Pruebas de seguridad — COMPLETADA
- [x] Autenticación/autorización verificada (401 con token inválido, 400 sin
  token, session_id ≠ token rechazado).
- [x] Validación de rutas (`..` rechazado, path_hash ≠ relative_path
  rechazado; **fix de seguridad**: rutas absolutas `/etc/...` ahora
  rechazadas — antes se normalizaban silenciosamente a ruta relativa).
- [x] Sesión expirada: test automático (`expired_session_gets_401_everywhere`)
  que expira la sesión en DB y verifica 401 en status, upload y GETs.
- [x] Rate limiting: middleware token bucket por IP (`RateLimiter`, 60 rps,
  burst 120, configurable con `SF_RATE_LIMIT_RPS`/`SF_RATE_LIMIT_BURST`) con
  test `rate_limiter_blocks_excess_requests` (burst, clave aislada por IP,
  renovación de ventana). 429 con `Retry-After`.
- [x] Content-Type: middleware que exige `application/json` en `/api/` con
  test (`content_type_middleware_rejects_non_json`).

#### Tarea 4.4 — Release V1 — COMPLETADA
- [x] Empaquetado por plataforma (server binario release, cliente desktop,
  APK Android debug + release firmado — ver `docs/48-documentacion-usuario.md`).
- [x] Firma de APK release documentada (keystore + `signingConfig`).
- [x] CI/CD: `.github/workflows/ci.yml` — cargo check + test (server, client
  single-thread, cli, models) + clippy informativo + `assembleDebug` Android
  con artefacto APK, en cada push/PR a `main`.
- [x] Hardening adicional (plan archify): health checks (`/health/live`,
  `/health/ready` con chequeo DB+storage), migraciones versionadas
  (`syncfiles-server/migrations/` con tabla `_migrations`, idempotentes),
  logging JSON estructurado con correlation ID (`X-Request-Id`/request_id),
  OpenAPI en `GET /api/v1/openapi`.
- [ ] Rollout/rollback orquestado — fuera de alcance V1 (un solo despliegue).

### Validación end-to-end ejecutada (evidencia)

Escenario: server Actix en `127.0.0.1:8082` con SQLite + storage local,
CLI `syncfiles-cli` y cliente desktop `syncfiles-client` con HOME aislado.

1. Login → session_id emitido, `auth.login_success` en audit_log.
2. Upload texto (48 B) y binario (64 KiB) → `accepted:true`, server_seq++.
3. Download binario → SHA-256 idéntico al original.
4. Diff desde 0 → cambios con path/checksum/operation correctos.
5. Rename → path actualizado en diff y en storage del server.
6. Conflicto: mismo path, contenido distinto desde otro device → 409 con
   `conflict_id` + alternativa preservada en storage.
7. Resolución `keep_local` con `preserve_alternative` → `conflict.resolved`
   en audit_log, alternativa intacta en disco.
8. Delete → soft-delete en DB (`deleted`, `deleted_at` seteado), diff
   reporta `operation:delete`.
9. Reinicio de server → datos persistidos, diff consistente.
10. Cliente desktop: auto-login desde sesión persistida, ciclo pull→push
    cada N segundos, watcher detecta archivos nuevos y los sube con
    checksum correcto.
11. Reinicio de cliente con operación encolada → `resume_queued_ops` la
    reanuda y sube al server (contenido íntegro).

## 4. Orden recomendado de implementación

1. ~~Base del proyecto y SQLite~~ (completado)
2. ~~Storage local y autenticación~~ (completado)
3. ~~API REST de sincronización~~ (completado)
4. ~~Cliente shell y sesión~~ (completado)
5. ~~Watcher, cola y sincronización~~ (completado)
6. ~~Conflictos y UX~~ (completado)
7. ~~Pruebas automáticas~~ (completado: 31 client + 39 server + 15 models + E2E scripts)
8. ~~Release~~ (completado: CI/CD en `.github/workflows/ci.yml`, hardening archify aplicado)

## 5. Dependencias clave

- Autenticación depende de la base y del modelo de sesión.
- API depende de almacenamiento y autenticación.
- Cliente shell depende de la API y de la sesión.
- Watcher depende de la API y de la cola local.
- Conflictos depende de la sincronización funcional.
- Release depende de pruebas y hardening final.

## 6. Criterios de cierre de la fase V1

La V1 queda cerrada cuando:
- [x] el cliente y el servidor sincronizan archivos sin corrupción,
- [x] los conflictos se resuelven con confirmación del usuario,
- [x] la cola recupera estados tras reinicios o caídas,
- [ ] las pruebas críticas pasan de forma automatizada (hoy son manuales),
- [ ] el release puede desplegarse sin riesgo crítico (falta CI/CD y
  empaquetado).

## 7. Siguiente paso concreto

1. Escribir tests de integración HTTP en `syncfiles-server/tests/` para los
   flujos ya validados manualmente: upload/download/delete, conflicto 409,
   `conflicts/resolve`, idempotencia y rutas inválidas.
2. Migrar esa misma cobertura al cliente (`syncfiles-client`) para
   `resume_queued_ops` y resolución de conflictos.
3. Configurar CI (GitHub Actions) con `cargo check`/`cargo test` en los 4
   crates y `./gradlew assembleDebug` para Android.
4. Empaquetado por plataforma y release V1.
