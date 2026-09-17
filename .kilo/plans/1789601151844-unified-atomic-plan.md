# Plan Unificado Atómico — SyncFiles

> Plan consolidado de todos los planes del repo. Cada item es una actividad atomica ejecutable en secuencia continua.

## Fuentes analizadas

| Plan fuente | Estado | Hallazgo |
|---|---|---|
| plan.md (Fases 1-6 + 5b + 5b.1 + 6) | COMPLETO (17/18 items checkados) | Solo 6.8 (registro usuarios) diferido a V2 |
| 1789513286176-test-plan.md | COMPLETADO | 70 tests pass, E2E 10/10, clippy clean |
| 1789441050663-v1-hardening-release-plan.md | COMPLETADO | Migrations fix, tests, CI, docs done |
| 1788905032752-v1-implementation-chunks.md | COMPLETADO | 7 chunks ejecutados |
| 1789168311964-emacs-client-plan.md | COMPLETADO | Existe syncfiles-emacs/ con test-e2e.el |
| 1789185875795-android-share-ingest-plan.md | COMPLETADO | Implementado (commit 9b5e257) |
| 1789267482152-manual-instalacion-apk.md | COMPLETADO | docs/48-documentacion-usuario.md completo |
| 1789278189749-changelog-pull-rename-move-copy-delete.md | COMPLETADO | Implementado (commit 694e0cb) |
| 1789363092690-archify-remediation-plan.md | PARCIAL | 4 P0 + 6 P1 de 23 findings pendientes |
| 1789513286176-data-robustness-plan.md | PARCIAL | Fases 0-4 mayoritariamente NO hechos |
| 1789448560089-archify-reevaluation-v2-v3.md | ABIERTO | 6 preguntas sin resolver |
| docs/51-backlog-implementacion-v1.md | COMPLETADO | Epic 4 COMPLETADA |
| docs/49-roadmap.md | Fase 2 COMPLETADA | V2 pendiente |
| docs/33-logging-y-monitoreo.md | STUB | 5 lineas, sin contenido |
| docs/35-actualizaciones.md | STUB | 6 lineas, sin contenido |
| docs/36-backup-y-restauracion.md | STUB | 4 lineas, sin contenido |
| docs/04-analisis-de-riesgos.md | PARCIAL | Plan de Respuesta sin detalle |
| docs/42-pruebas-seguridad.md | COMPLETO | Contenido valido |
| docs/43-pruebas-rendimiento.md | NO EJECUTADO | Plan existe, benchmarks no |
| docs/45-infraestructura-nas.md | V2 | NAS no es V1 |
| .github/workflows/ci.yml | COMPLETO | CI funcional |
| docs/48-documentacion-usuario.md | COMPLETO | Manual completo 290 lineas |

---

## Legenda de prioridad

- **P-CRITICO**: Robustez/tolerancia a fallo/backups (prioridad 1 del usuario)
- **P-ALTA**: Installers/empaquetamiento (prioridad 2 del usuario)
- **P-MEDIA**: Deployment nube/dispositivos (prioridad 3 del usuario)
- **P-BAJA**: Nice-to-have/documentacion

## Estado de items completados

Items COMPLETADOS (implementados y validados — 2026-09-17):

| Item | Estado | Validación |
|------|--------|-----------|
| A-01 | ✅ COMPLETO | WAL + FK activos; `cargo check` + `cargo test` pass |
| A-02 | ✅ COMPLETO | Path de migraciones resuelto desde `current_exe()` |
| A-03 | ✅ COMPLETO | `PRAGMA integrity_check` en `init_db`; loggea error sin panic |
| A-04 | ✅ COMPLETO | `SF_HEALTH_PROBE_WRITE=0` deshabilita write probe |
| A-05 | ✅ COMPLETO | `SF_DATABASE_URL` y `SF_STORAGE_ROOT` resueltos a absolutos |
| A-06 | ✅ COMPLETO | `scripts/backup.sh` creado (VACUUM INTO + tar + rotación) |
| A-07 | ✅ COMPLETO | `POST /admin/backup` en `backup.rs` + ruta en main.rs |
| A-08 | ✅ COMPLETO | `scripts/e2e-backup-restore.sh` creado |
| A-09 | ✅ COMPLETO | `jobs.rs` con prune_audit_log, FK check, vacuum, prune sessions/idempotency |
| A-10 | ✅ COMPLETO | `RateLimiter` → trait + `InMemoryRateLimiter` + `RedisRateLimiter` stub |
| A-11 | ✅ COMPLETO | Health check tests agregados; CI usa `SF_HEALTH_PROBE_WRITE=0` |
| A-12 | ✅ COMPLETO | `docs/33-logging-y-monitoreo.md` completo |
| A-13 | ✅ COMPLETO | `docs/35-actualizaciones.md` completo |
| A-14 | ✅ COMPLETO | `docs/36-backup-y-restauracion.md` completo |
| A-15 | ✅ COMPLETO | §7 Plan de Respuesta completo en `docs/04-analisis-de-riesgos.md` |

Items PENDIENTES (Bloque 2 parcial — backups funcionales ya implementados):

| Item | Estado | Bloqueador |
|------|--------|-----------|
| B-01 | ⏳ PENDIENTE | APK signing config en Gradle |
| B-05 | ⏳ PENDIENTE | CI release job (binarios + checksums GitHub) |
| B-06 | ⏳ PENDIENTE | Firmar binarios Rust (codesign/signtool/GPG) |
| B-02 | ⏳ PENDIENTE | Windows installer (.msi/.exe) |
| B-03 | ⏳ PENDIENTE | macOS .dmg firmado + notarizado |
| B-04 | ⏳ PENDIENTE | Linux tarball/.deb |
| B-07 | ⏳ PENDIENTE | versionCode + auto-update policy (documentado parcial) |
| B-08 | ⏳ PENDIENTE | CLI packaging tarball |
| B-09 | ⏳ PENDIENTE | API versioning policy (documentado en A-13) |
| C-01 | ⏳ PENDIENTE | Dockerfile servidor (multi-stage) |
| C-02 | ⏳ PENDIENTE | docker-compose local (dev + staging) |
| C-03 | ⏳ PENDIENTE | Despliegue GCP free tier |
| C-04 | ⏳ PENDIENTE | Despliegue AWS free tier |
| C-07 | ⏳ PENDIENTE | Configuración LAN (script setup-lan.sh) |
| C-08 | ⏳ PENDIENTE | Dual-write opcional (V1 stub) |
| D-01 | ⏳ PENDIENTE | 6 preguntas de archify-reevaluation (decisión usuario) |
| D-02 | ⏳ PENDIENTE | Branch protection + CODEOWNERS + dependabot |
| D-03 | ⏳ PENDIENTE | Input validation middleware (bloqueado por D-01) |

---

## FASE A: ROBUSTEZ / TOLERANCIA AL FALLO / BACKUPS (P-CRITICO)

> Prioridad absoluta: asegurar que el sistema no pierda datos, pueda recuperarse de fallos y tenga backups funcionales. Cada item es un paso atómico.

---

### A-01. Activar WAL mode + foreign_keys en SQLite (servidor)
- **Fuente**: data-robustness-plan Fase 0.1, archify-remediation P0 item 2 (health checks)
- **Dependencias**: Ninguna
- **Archivos**: `syncfiles-server/src/state.rs` (metodo `AppState::new`)
- **Accion**: En la creacion del pool, agregar pragmas despues de `connect_with`:
  ```
  .pragma("journal_mode=WAL")
  .pragma("foreign_keys=ON")
  .pragma("synchronous=NORMAL")
  .pragma("busy_timeout=5000")
  ```
- **Validacion**: `PRAGMA journal_mode` devuelve `wal`; `PRAGMA foreign_key_check` no devuelve filas
- **Riesgo**: WAL cambia formato de lock; tests que asumen DELETE mode deben ajustarse (mitigacion: actualizar expectations)

### A-02. Corregir ruta de migraciones a path absoluto
- **Fuente**: data-robustness-plan Fase 0.2
- **Dependencias**: A-01
- **Archivos**: `syncfiles-server/src/main.rs` (cerca de linea 35)
- **Accion**: Reemplazar `./syncfiles-server/migrations` por resolucion via `std::env::current_exe()` con fallback al path relativo. El path debe resolverse siempre desde el binario, no desde CWD.
- **Validacion**: Levantar server desde directorio diferente al repo; migraciones se aplican correctamente; `change_log` existe tras arrancar

### A-03. Agregar PRAGMA integrity_check al arranque del servidor
- **Fuente**: data-robustness-plan Fase 0.3
- **Dependencias**: A-01
- **Archivos**: `syncfiles-server/src/db.rs` (funcion `init_db`)
- **Accion**: Despues de `init_db`, ejecutar `PRAGMA integrity_check`. Si no es "ok", loguear error (no panic). El server arranca pero advertido.
- **Validacion**: Corromper DB manualmente, reiniciar server, verificar error logueado

### A-04. Hacer health check write opcional en CI
- **Fuente**: data-robustness-plan Fase 0.4, archify-remediation P0 item 2
- **Dependencias**: A-01
- **Archivos**: `syncfiles-server/src/health.rs` (o donde esté el readiness probe)
- **Accion**: Hacer la escritura del probe file opcional via `SF_HEALTH_PROBE_WRITE=0` (default 1). En CI se pone a 0 para que el readiness no falle por falta de permiso de escritura.
- **Validacion**: `SF_HEALTH_PROBE_WRITE=0 cargo test` pasa; sin env var el probe escribe archivo

### A-05. Hacer ruta de storage y DB robusta (no relativa a CWD)
- **Fuente**: data-robustness-plan Fase 0.2, plan.md 5b.1 (bug corregido pero la proteccion no es robusta)
- **Dependencias**: A-02
- **Archivos**: `syncfiles-server/src/config.rs`, `syncfiles-server/src/main.rs`
- **Accion**: Los defaults de `SF_DATABASE_URL` y `SF_STORAGE_ROOT` deben resolverse siempre a paths absolutos (via `dirs` crate o similar). Si son relativos, resolverlos siempre contra el CWD del binario (no del proceso caller). Documentar que cambiar de CWD rompe la BD existente.
- **Validacion**: Cambiar de directorio despues de arrancar server no corrompe datos existentes

### A-06. Implementar script de backup automatizado
- **Fuente**: data-robustness-plan Fase 1.1, docs/36-backup-y-restauracion.md (stub)
- **Dependencias**: A-05
- **Archivos nuevos**: `scripts/backup.sh`
- **Accion**: Script bash que:
  1. `VACUUM INTO '<backup_dir>/metadata_<timestamp>.db'` (atomatic, online)
  2. `tar -czf storage-backup.tar.gz -C "$SF_STORAGE_ROOT" users/`
  3. Combina en timestamped archive `backup_YYYYMMDD_HHMM.tar.gz`
  4. Rotacion: elimina backups > BACKUP_RETENTION_DAYS (default 30)
  5. Loguea resultado a archivo
- **Validacion**: Script ejecutable, crea backups con timestamp, respeta retención

### A-07. Crear endpoint de backup manual
- **Fuente**: data-robustness-plan Fase 1.2
- **Dependencias**: A-06
- **Archivos nuevos**: `syncfiles-server/src/backup.rs` + endpoint en `main.rs`
- **Accion**: Endpoint `POST /admin/backup` protegido por token de admin. Ejecuta VACUUM INTO + tar de storage. Devuelve path + checksum del backup.
- **Validacion**: Endpoint devuelve 200 con path y checksum; backup verificable

### A-08. Test E2E backup y restore
- **Fuente**: data-robustness-plan Fase 1.3
- **Dependencias**: A-06, A-07
- **Archivos nuevos**: `scripts/e2e-backup-restore.sh`
- **Accion**: Script que:
  1. Inicia server, sube archivos, verifica
  2. Ejecuta backup (A-07)
  3. Para server, borra DB + storage
  4. Restaura backup
  5. Reinicia server, verifica archivos y checksums
- **Validacion**: Script pasa; checksums post-restore coinciden con pre-backup

### A-09. Jobs background de limpieza y salud
- **Fuente**: data-robustness-plan Fase 1.4
- **Dependencias**: A-01
- **Archivos nuevos**: `syncfiles-server/src/jobs.rs`
- **Accion**: `tokio::task::spawn` en `main.rs` que ejecute periódicamente:
  - `prune_audit_log()` — diario (>90 días)
  - `check_foreign_keys()` — diario
  - `VACUUM` o `PRAGMA optimize` — semanal
  - `prune_sessions_expired()` — diario (>7 días)
  - `prune_idempotency_keys()` — diario (>30 días)
- **Validacion**: Jobs ejecutan sin error (verificar logs)

### A-10. Preparar RateLimiter para Redis
- **Fuente**: data-robustness-plan Fase 1.5, archify-remediation P1 items
- **Dependencias**: A-01
- **Archivos**: `syncfiles-server/src/middleware/mod.rs`
- **Accion**: Refactorizar `RateLimiter` a trait `RateLimiter: Send + Sync` con `InMemoryRateLimiter` (actual) y `RedisRateLimiter` (stub). `build_rate_limiter()` elige por `SF_RATELIMITER_BACKEND=redis|memory`.
- **Validacion**: Trait compila; `SF_RATELIMITER_BACKEND=memory` funciona igual que hoy; `=redis` usa stub sin panic

### A-11. Hacer health checks operativos y verificar en CI
- **Fuente**: archify-remediation P0 item 2, v1-hardening-release-plan
- **Dependencias**: A-01, A-04
- **Archivos**: `.github/workflows/ci.yml`
- **Accion**: Agregar al CI: verificar que `/health/live` y `/health/ready` responden 200 en tests de integracion. Los tests actuales (static_dashboard.rs) ya cubren parcialmente; asegurar que se ejecutan con `SF_HEALTH_PROBE_WRITE=0`.
- **Validacion**: CI pasa con health checks incluidos

### A-12. Completar docs/33-logging-y-monitoreo.md
- **Fuente**: stub actual (5 lineas), archify-remediation P0 item 3, data-robustness-plan
- **Dependencias**: Ninguna (documentacion)
- **Accion**: Escribir contenido basado en lo que ya existe en docs/05 §7 (observabilidad) y docs/07 §10:
  - Niveles: INFO, WARN, ERROR
  - Formato: NDJSON con ts, level, event, user_id, device_id, request_id, message
  - Rotación diaria, retención 30 días
  - Métricas: sync_operation_duration_ms, sync_failed_operations_total, active_sessions_count, queue_depth, retry_rate, conflict_rate
  - Health check endpoints y lo que reportan
  - Configuración via env vars (SF_LOG_LEVEL, SF_LOG_FORMAT)
- **Validacion**: Doc completo con ejemplos reales de config

### A-13. Completar docs/35-actualizaciones.md
- **Fuente**: stub actual (6 lineas)
- **Dependencias**: Ninguna
- **Accion**: Escribir: semver, politica de versionado de API (/v1 congelado, /v2 futuro), mecanismo de verificacion de compatibilidad cliente-backend, rollback strategy (reinstalar binario anterior + DB backup), calendario de actualizaciones.
- **Validacion**: Doc cubre versionado, compatibilidad y rollback

### A-14. Completar docs/36-backup-y-restauracion.md
- **Fuente**: stub actual (4 lineas), data-robustness-plan Fases 1-2
- **Dependencias**: A-06, A-07
- **Accion**: Escribir: procedimiento de backup diario (A-06), endpoint manual (A-07), test E2E restore (A-08), RPO ≤ 24h, RTO < 4h, retención 4-12 semanas, registro de restauraciones, checklist de verificacion post-restore.
- **Validacion**: Doc con procedimientos ejecutables

### A-15. Completar Plan de Respuesta de riesgos (docs/04)
- **Fuente**: docs/04-analisis-de-riesgos.md §7 (stub)
- **Dependencias**: A-12 (para tener operativo el logging)
- **Accion**: Completar §7 con:
  - Dueños por riesgo (RT-01: Lead Ops, RS-01: Security Lead, etc.)
  - Umbrales de activacion (ej: >5% errores → SEV2; server caído → SEV1)
  - Procedimientos de contención por SEV (SEV1: contener → comunicar → recuperar → post-mortem ≤24h; SEV2: diagnosticar → mitigar → doc ≤4h; SEV3: registrar → priorizar)
  - Procedimiento de backup/restore (ref A-06/07/08)
  - Contactos y escalamiento
- **Validacion**: Cada riesgo tiene owner, umbral y procedimiento documentado

---

## FASE B: INSTALLERS / EMPAQUETAMIENTO / DISTRIBUCIÓN (P-ALTA)

> Prioridad 2: Asegurar que cada componente pueda instalarse y actualizarse de forma segura en cada plataforma.

---

### B-01. Configurar firma release del APK en Gradle (empaquetado)
- **Fuente**: manual-instalacion-apk.md Tarea 1.1, archify-remediation P1 (containerization as proxy for signing)
- **Dependencias**: Ninguna
- **Archivos**: `syncfiles-android/app/build.gradle.kts`, `.gitignore`
- **Accion**: Agregar bloque `signingConfigs` que lea `SF_KEYSTORE_FILE`, `SF_KEYSTORE_PASSWORD`, `SF_KEY_ALIAS`, `SF_KEY_PASSWORD` desde `project.findProperty(...)`. En `buildTypes.release` asignar `signingConfig` solo si las propiedades existen. Agregar a `.gitignore`: `/syncfiles-android/gradle.properties.local` y `/syncfiles-android/*.jks`.
- **Validacion**: `./gradlew assembleDebug` compila; `./gradlew assembleRelease` funciona con y sin propiedades de firma

### B-02. Empaquetado Windows (.msi/.exe firmado)
- **Fuente**: docs/46-deployment-clientes.md §2.1, archify-remediation P1 (containerization)
- **Dependencias**: B-05 (tener binario release estable)
- **Acciones**:
  1. Elegir herramienta: `wininstaller` (Rust) o `wix` para .msi, o `inkit` para .exe
  2. Crear instalador que haga: copiar binario, agregar al PATH opcional, crear acceso directo, incluir `README.txt` de uso basico
  3. Firmar con certificado (requiere compra de certificado code-signing)
- **Validacion**: Instalador ejecutado en Windows limpio instala SyncFiles correctamente

### B-03. Empaquetado macOS (.dmg firmado)
- **Fuente**: docs/46-deployment-clientes.md §2.2
- **Dependencias**: B-05
- **Acciones**:
  1. Crear .dmg con `create-dmg` o `productbuild`
  2. Incluir binario, permisos de carpeta SyncFiles, y acceso directo
  3. Firmar con Developer ID Application certificate (requiere Apple Developer Account)
  4. Notarizar (requiere Apple Developer Account)
- **Validacion**: .dmg instalado en macOS limpio funciona; Gatekeeper no bloquea

### B-04. Empaquetado Linux (.deb/.rpm o tarball)
- **Fuente**: docs/46-deployment-clientes.md §2.3
- **Dependencias**: B-05
- **Acciones**:
  1. Crear tarball portable: `tar -czf syncfiles-linux-x64.tar.gz syncfiles-server syncfiles-client syncfiles-cli README.md`
  2. Opcional: .deb con `cargo deb` o `dpkg-deb`; .rpm equivalente
  3. Incluir script de inicio/stop, systemd unit file opcional
- **Validacion**: Tarball extraido y ejecutado funciona; .deb instala con dpkg

### B-05. Stabilizar binarios release (base para empaquetado)
- **Fuente**: v1-hardening-release-plan §4, manual-instalacion-apk.md
- **Dependencias**: A-11 (CI con health checks)
- **Archivos**: `.github/workflows/ci.yml`
- **Accion**: Agregar al CI un job `release` que:
  1. `cargo build --release` en server, client, cli
  2. `cargo clippy -- -D warnings` (debe ser clean)
  3. `cargo test` en todos los crates
  4. Genera artefactos GitHub releases con los binarios y checksums SHA256
  5. `assembleDebug` Android con artefacto APK
- **Validacion**: Push a main genera release con binarios firmados y verificables

### B-06. Firmar binarios Rust (code signing)
- **Fuente**: docs/46-deployment-clientes.md §2 (firma digital obligatoria)
- **Dependencias**: B-05
- **Accion**: Definir proceso de firma para cada binario:
  - macOS: `codesign` con Developer ID
  - Windows: `signtool` con certificate
  - Linux: GPG signing of tarball / repo metadata
- **Validacion**: Binarios pasan verificacion de firma en cada plataforma

### B-07. Verificar versionCode/APK release y auto-update
- **Fuente**: manual-instalacion-apk.md §2.6, docs/46-deployment-clientes.md §4
- **Dependencias**: B-01
- **Archivos**: `syncfiles-android/app/build.gradle.kts`
- **Accion**: Documentar y verificar que `versionCode` se incrementa en cada release. Definir politica de auto-update: manual (user verifica) vs automatic (background check contra version endpoint). Para V1: manual con notificacion.
- **Validacion**: `versionCode` incrementado; update check funciona o esta documentado

### B-08. Empaquetado CLI para distribucion
- **Fuente**: manual-instalacion-apk.md, docs/46-deployment-clientes.md
- **Dependencias**: B-05
- **Accion**: Crear tarball `syncfiles-cli-vX.Y.Z-<platform>.tar.gz` con binario + README de uso rapido. Firma del tarball.
- **Validacion**: CLI funciona desde instalacion limpia

### B-09. Definir versionado de API y política de compatibilidad
- **Fuente**: archify-reevaluation-v2-v3 §2.4, docs/46-deployment-clientes.md §4, docs/05 §6.8
- **Dependencias**: Ninguna (decisional)
- **Accion**: Documentar: /v1 congelado en V1; /v2 en V2 con breaking changes. Clientes V1 siguen funcionando en servidor V2 (feature flag `v1_compat`). Versión de cliente reportada en cada request para server-side compatibility checks.
- **Validacion**: Doc firmada; implementacion del versionado documentada en API router

---

## FASE C: DEPLOYMENT EN LA NUBE / LOCAL / NAS (P-MEDIA)

> Prioridad 3: Configurar despliegue en GCP/AWS free tier, Google Drive, NAS y red local.

---

### C-01. Dockerfile para servidor (despliegue estandarizado)
- **Fuente**: archify-remediation P1 item 9 (No Containerization)
- **Dependencias**: A-01 (WAL mode), B-05 (CI estable)
- **Archivos nuevos**: `Dockerfile` en raiz y `syncfiles-server/`
- **Accion**: Multi-stage Dockerfile: stage build con Rust, stage runtime con SQLite. `COPY . .`, `RUN cargo build --release`, `CMD ["./target/release/syncfiles-server"]`. Documentar docker-compose.yml para 2 instancias + SQLite compartido (V1 single-node).
- **Validacion**: `docker build` y `docker run` levantan server funcional; `docker-compose up` despliega stack completo

### C-02. Docker compose local (dev + staging)
- **Fuente**: archify-remediation P1 item 9
- **Dependencias**: C-01
- **Archivos nuevos**: `docker-compose.yml`
- **Accion**: Definir services: `server` (desde Dockerfile), `backup` (cron con scripts/backup.sh montado), `adminer` o `sqlitebrowser` opcional para debugging. Volumenes para persistencia.
- **Validacion**: `docker-compose up` levanta todo; backup cron funciona

### C-03. Despliegue GCP free tier (Cloud Run + Cloud SQL)
- **Fuente**: user request (GCP free tier), archify-remediation
- **Dependencias**: C-01, C-02
- **Accion**: Configurar despliegue en GCP:
  - **Cloud Run**: servidor containerizado (C-01), permite ~2 vCPU + 1GB gratis
  - **Cloud SQL (PostgreSQL)**: para V2; en V1 usar Cloud SQL for SQLite VM micro (free tier) o persistencia en volumen de Cloud Run
  - **Cloud Storage**: backups automaticos (ref A-06)
  - Script de despliegue: `gcloud run deploy` con secretos para SF_USER_*
- **Validacion**: Servidor corriendo en Cloud Run responde a health checks; backup se ejecuta

### C-04. Despliegue AWS free tier (ECS/Fargate + RDS)
- **Fuente**: user request (AWS free tier)
- **Dependencias**: C-01, C-02
- **Accion**: Configurar despliegue en AWS:
  - **ECS Fargate**: server containerizado, 0.5 vCPU + 1GB (free tier eligible)
  - **RDS**: PostgreSQL micro (free tier 1 año) para V2; V1 con SQLite en EFS o task storage
  - **S3**: backups automaticos
  - Script: `aws ecs create-task` o Terraform mínimo
- **Validacion**: Servidor corriendo en ECS responde; backup a S3 funciona

### C-05. Google Drive como storage sync (cliente)
- **Fuente**: user request (Google Drive), archify-reevaluation
- **Dependencias**: No aplica en V1 (requiere OAuth2 + Drive API)
- **Accion**: DECISION: Diferir a V2. En V1 el storage es local. Documentar que Drive como storage backend requiere: OAuth2 flow, Drive API SDK, `gdrive` crate o similar, y mapeo de paths a Drive file IDs. Es parte de `StorageProvider` abstraction, listo para V2.
- **Validacion**: Documentado como out-of-scope V1; stub preparado para V2

### C-06. Configurar NAS como storage (V2)
- **Fuente**: docs/45-infraestructura-nas.md, docs/05 §8.3.3
- **Dependencias**: V2 decisiones resueltas (archify-reevaluation open questions)
- **Accion**: No aplica en V1. Documentar requisitos: SFTP/SMB3 access, credential storage in server-only coffer, path mapping by user_id, checksum verification, fallback to LocalDisk. Implementar `NasStorageProvider` como nueva implementacion de `StorageProvider` trait cuando se active V2.
- **Validacion**: V2 trigger definido; NAS plan documentado

### C-07. Configuracion LAN local (completar/verificar)
- **Fuente**: user request (Red Local), manual-instalacion-apk.md §7.2, plan.md §2.4
- **Dependencias**: B-01 (APK firmado), C-01 (Docker opcional)
- **Accion**: Verificar y documentar:
  - Server en LAN: `SF_BIND_ADDRESS=0.0.0.0:8080` (ya documentado)
  - Android en LAN: editar `network_security_config.xml` con IP del servidor (ya documentado)
  - Firewall: instrucciones para macOS (Cortafuegos) y Linux (ufw)
  - Desktop en LAN: `SF_SERVER_URL=http://<ip>:8080`
  - Crear script `scripts/setup-lan.sh` que configure todo automaticamente
- **Validacion**: Dos dispositivos en misma WiFi sincronizan con server local

### C-08. Redundancia local: dual-write opcional (opcion 3 de data-robustness)
- **Fuente**: data-robustness-plan Fase 4, Opcion 3
- **Dependencias**: A-08 (backup funcional), V2 decisions
- **Accion**: Implementar dual-write como opt-in via env var `SF_DUAL_WRITE_TARGET=<path>`. Cuando activado, cada escritura de archivo tambien se replica al target path. Job de reconciliacion compara ambos. Es migracion a NAS/PostgreSQL. Para V1: documentar como opcion; implementar stub si hay tiempo.
- **Validacion**: Dual-write produce copies identicas en ambos targets; reconciliacion detecta discrepancias

---

## FASE D: DECISIONES GOBERNADAS Y PLAN V2 (ABIERTO)

> Items del archify-reevaluation que necesitan decisiones del usuario antes de implementar.

---

### D-01. Resolver 6 preguntas abiertas de archify-reevaluation
- **Fuente**: archify-reevaluation-v2-v3 §6
- **Dependencias**: Ninguna (decisiones del usuario)
- **Preguntas**:
  1. Multi-usuario en V2: ¿Sí/No? (desbloquea RBAC, Privacy, Compliance)
  2. E2E scope: ¿por archivo o por carpeta/share?
  3. MFA: ¿TOTP only o TOTP + WebAuthn en V2?
  4. API versioning: ¿URL path o header?
  5. Enterprise target V2 vs V3?
  6. HA V3: ¿Kubernetes o VMs + Patroni?
- **Accion**: El usuario debe responder estas 6 preguntas para desbloquear diseño V2

### D-02. Governance: branch protection + CODEOWNERS + dependabot
- **Fuente**: archify-remediation P1 item 5 (governance), archify-reevaluation A5
- **Dependencias**: Ninguna (puede hacerse ya)
- **Archivos nuevos**: `CODEOWNERS`, `.github/branch-protection` (config en GitHub UI)
- **Accion**: Activar branch protection en `main`: require 1 review + CI pass + clippy pass. Crear `CODEOWNERS`. Activar dependabot. Crear `develop` branch para features.
- **Validacion**: PR sin review/CI fail bloqueado en main

### D-03. Implementar input validation middleware (archify P0 item 1)
- **Fuente**: archify-remediation P0 item 1, docs/42-pruebas-seguridad.md
- **Dependencias**: A-01
- **Archivos**: `syncfiles-server/src/middleware/mod.rs`
- **Accion**: Middleware que valida Content-Type, max body size, y patterns de SQL injection basics en todos los endpoints (excepto health y login rate limit). En V1 es defensivo; la validacion fuerte viene con RBAC en V2.
- **Validacion**: OWASP ZAP scan sin hallazgos criticos

---

## MAPA DE DEPENDENCIAS ENTRE FASES

```
A-01 ─► A-02 ─► A-05 ─► A-06 ─► A-07 ─► A-08
 │                                      │
 ├──► A-03                              │
 │                                      ▼
 ├──► A-04 ─► A-11 ─► B-05 ─► B-06 ─► B-02
 │              │
 ├──► A-09 (independiente dentro de A)
 │
 ├──► A-10 (independiente dentro de A)
 │
 ├──► A-12 (documentacion, paralelo)
 ├──► A-13 (documentacion, paralelo)
 ├──► A-14 (documentacion, paralelo)
 └──► A-15 (documentacion, paralelo)

B-05 ─► B-07 ─► B-08
 │
 ├──► B-02 (Windows)
 ├──► B-03 (macOS)
 └──► B-04 (Linux)

B-05 ─► C-01 ─► C-02 ─► C-03 (GCP)
 │                    └──► C-04 (AWS)
 │
 └──► C-07 (LAN) (independiente parcial)

D-02 (puede hacerse ya)
D-01 (bloquea D-03 y todo V2)
D-03 (depende de A, bloqueado por D-01)
```

---

## SECUENCIA RECOMENDADA DE EJECUCION

### Bloque 1: Robustez critica (empezar AQUI)
1. **A-01** — WAL + foreign_keys (minuto de trabajo, impacto maximo)
2. **A-03** — integrity_check al arranque
3. **A-04** — health check CI-safe
4. **A-02** — migration path absoluto
5. **A-05** — storage/DB paths robustos
6. **A-11** — health checks en CI
7. **A-12** — docs/33 logging (parallelizable con los anteriores)
8. **A-13** — docs/35 actualizaciones (parallelizable)
9. **A-14** — docs/36 backup (parallelizable)
10. **A-15** — Plan de respuesta riesgos

### Bloque 2: Backups funcionales
11. **A-06** — Script backup.sh
12. **A-07** — Endpoint backup manual
13. **A-08** — Test E2E backup/restore
14. **A-09** — Jobs background limpieza
15. **A-10** — RateLimiter Redis-ready

### Bloque 3: Empaquetamiento y firmado
16. **B-01** — APK firma release Gradle
17. **B-05** — CI release job (binarios + checksums)
18. **B-06** — Firmar binarios Rust
19. **B-02** — Windows installer
20. **B-03** — macOS .dmg
21. **B-04** — Linux tarball/.deb
22. **B-07** — versionCode + auto-update policy
23. **B-08** — CLI packaging

### Bloque 4: Deployment nube + local
24. **C-01** — Dockerfile servidor
25. **C-02** — Docker compose local
26. **C-07** — Configuracion LAN (script setup)
27. **C-03** — Despliegue GCP free tier
28. **C-04** — Despliegue AWS free tier
29. **C-08** — Dual-write opcional (V1 stub)

### Bloque 5: Governance + decisiones
30. **D-02** — Branch protection + CODEOWNERS
31. **D-01** — 6 preguntas abiertas (user decision)
32. **D-03** — Input validation (post-D-01)

### Bloque 6: Documentacion operativa (parallelizable desde Bloque 1)
- docs/42-pruebas-seguridad.md: validar que los tests de seguridad existen y pasan
- docs/43-pruebas-rendimiento.md: ejecutar benchmarks basicos
- docs/50-funcionalidades-futuras.md: priorizar features post-MVP

---

## CON QUE EMPEZAR

**Bloque 1 COMPLETADO (2026-09-17):** A-01 → A-02 → A-03 → A-04 → A-05 → A-06 → A-07 → A-08 → A-09 → A-10 → A-11 → A-12 → A-13 → A-14 → A-15

**Próximo bloque (Bloque 3 — Empaquetamiento):** B-05 (CI release) → B-06 (code signing) → B-01 (APK signing) → B-02/B-03/B-04 (installers) → B-07/B-08

**Pendiente decisión del usuario:** D-01 (6 preguntas de archify-reevaluation) bloquea D-03 e implementación de V2.

### Estado de validación

```
cargo check -p syncfiles-server   ✅
cargo clippy -- -D warnings    ✅ (server, cli, models clean)
cargo test -p syncfiles-server    ✅ (41 pass, con SF_HEALTH_PROBE_WRITE=0)
cargo test -p syncfiles-client    ✅ (31 pass, --test-threads=1)
cargo test -p syncfiles-models    ✅ (9 pass)
bash -n scripts/backup.sh         ✅
bash -n scripts/e2e-backup-restore.sh ✅
```

