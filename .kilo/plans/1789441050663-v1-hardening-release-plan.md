# Plan: Cerrar V1 — Pruebas, CI/CD y Documentación

> **Contexto:** La última sesión completó las 6 fases del `plan.md` (endpoints server, UI desktop, Android, web dashboard, E2E real, web functionality) y el commit `694e0cb` (change_log, diff por cursor seq, pull rename/move/copy, deletes locales). El proyecto tiene V1 funcional pero sin cerrar oficialmente.

> **Qué queda del último sesión:** El `docs/51-backlog-implementacion-v1.md` marca Epic 4 (Validación y release) como **PARCIAL/PENDIENTE**: tareas 4.1 (tests unitarios cliente), 4.2 (tests integración cliente), 4.3 (tests seguridad), 4.4 (release). Ninguna se ha empezado.

---

## 0. Alcance y criterios de salida

**Incluido:**
- Tests automatizados para `syncfiles-client` (unitarios + integración)
- Tests de seguridad automatizados (sesión expirada, rate limiting 429)
- Pipeline CI/CD (GitHub Actions) con `cargo check`/`cargo test` en todos los crates Rust + `assembleDebug` Android
- Cierre oficial de V1 (actualizar docs de estado)
- Corregir issue de migraciones duplicadas

**Excluido (V2):** encriptación E2E, NAS, PostgreSQL, Kafka, empaquetado firmado APK release.

**Criterios de éxito:**
- `cargo test` pasa en `syncfiles-server` (35 tests existentes + nuevos) y `syncfiles-client` (nuevos)
- `cargo check` pasa en los 4 crates
- `.github/workflows/ci.yml` ejecuta checks y tests en PR
- `docs/51-backlog-implementacion-v1.md` Epic 4 marcado COMPLETADA
- `docs/49-roadmap.md` Fase 2 marcada COMPLETADA

---

## 1. Fix: migraciones duplicadas en `syncfiles-server/migrations/`

**Archivos:** `001__init.sql` y `001_init.sql` — dos archivos con el mismo version `1` pero contenido distinto:
- `001__init.sql` (130 líneas): schema COMPLETO — incluye `change_log`, `metadata`, `idempotency_keys`, todos los índices (`idx_change_log_user_seq`, etc.)
- `001_init.sql` (110 líneas): schema antiguo — sin `change_log`, sin `metadata`, sin índices compuestos

El runner (`migrations.rs`) ordena alfabéticamente y aplica solo el primero por versión. `001__init.sql` gana (0x5F5F < 0x5F69) y es el correcto. El segundo se salta por `applied.contains(&version)`.

**Acción:** Eliminar `001_init.sql` (la versión incompleta — sin `change_log`, sin `metadata`, sin índices compuestos). Conservar `001__init.sql` que tiene el schema completo. El orden alfabético ya garantiza que se aplique el correcto (doble `_` < `_i`). No hay riesgo de perder funcionalidad.

- **Validación:** `cargo test --package syncfiles-server` pasa; server arranca con DB limpia y aplica migraciones correctamente (verificar que `change_log` existe tras arrancar).

---

## 2. Tests unitarios para cliente desktop (`syncfiles-client`)

**Referencia:** `docs/51-backlog-implementacion-v1.md` Tarea 4.1 (PARCIAL)

**Archivo:** `syncfiles-client/src/` — crear `tests/` directory o módulos inline

Agregar tests unitarios para:

### 2.1 `network.rs` — mock server tests
- `login()` con credenciales válidas → `LoginResponse` con `session_id` no vacío
- `login()` con credenciales inválidas → error
- `session_status()` con token válido vs inválido
- `upload()` round-trip texto y binario (base64)
- `download()` verifica checksum del contenido recuperado
- Reintentos ante 429/5xx (el `retryable` flag de `ApiError` se computa correctamente)

### 2.2 `sync.rs` — SyncEngine tests
- `reconcile_local_folder()` detecta archivos nuevos/creados/modificados/borrados en disco
- `push_local()` encola operaciones pendientes
- `resume_queued_ops()` reanuda operaciones con `status='queued'`/`'retry'`
- `apply_renaming()` usa `old_path` como fallback cuando no hay fila local
- `push_local()` propaga deletes locales (archivo no existe en disco → `status='deleted'`)

### 2.3 `metadata.rs` — MetadataStore tests
- `enqueue()` crea entrada con status `pending`
- `get_pending()` filtra solo `pending`/`retry`
- `resume_queued_ops()` cambia `retry` → `queued`
- `upsert_file()` no incluye columna `content` (schema client no la tiene)
- Persistencia entre instancias (SQLite on-disk)

### 2.4 `config.rs` tests
- `Config::load()` con defaults y env vars
- `Config::save()`/`load()` round-trip
- `Config::data_dir()` respeta `SF_DATA_DIR`

**Patrón:** Usar `#[cfg(test)]` módulos inline en cada archivo (estilo que ya existe en `syncfiles-models`). Para tests de `network.rs` que necesitan un servidor, usar `#[tokio::test]` con ` warp` o `mockito` (si está en dev-deps) o levantando un `actix-web` test app inline.

**Validación:** `cargo test --package syncfiles-client` pasa (mínimo 15 tests nuevos).

---

## 3. Tests de seguridad automatizados

**Referencia:** `docs/51-backlog-implementacion-v1.md` Tarea 4.3 (PARCIAL)

Agregar a `syncfiles-server/tests/api_endpoints.rs` (o `static_dashboard.rs` si hay espacio):

### 3.1 Sesión expirada
- Crear sesión directamente en DB con `expires_at` en el pasado
- `GET /session/status` con ese token → 401
- Cualquier mutation con token expirado → 401

### 3.2 Rate limiting 429
- Enviar > 60 requests/segundo (burst 120) a un endpoint protegido
- Verificar que las peticiones excedentes retornan 429 `TOO_MANY_REQUESTS`
- Esperar que el rate limiter se recupere (window slide)

### 3.3 Validación de path
- `POST /sync/upload` con `path_hash` ≠ hash de `relative_path` → 400 `INVALID_PATH_HASH`
- `POST /sync/upload` con `relative_path` conteniendo `..` → 400
- `POST /sync/upload` con `relative_path` absoluta → 400

**Validación:** Nuevos tests pasan, `cargo test --package syncfiles-server` global pasa.

---

## 4. Pipeline CI/CD

**Archivo a crear:** `.github/workflows/ci.yml`

```yaml
name: CI
on: [push, pull_request]
jobs:
  check-and-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: clippy, rustfmt }
      - name: Cargo check (all crates)
        run: cargo check --workspace --all-targets
      - name: Cargo test (server)
        run: cargo test --package syncfiles-server
      - name: Cargo test (models)
        run: cargo test --package syncfiles-models
      - name: Cargo test (client) [if tests exist]
        run: cargo test --package syncfiles-client
      - name: Cargo clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
      - name: Android build
        if: matrix.include-android
        run: cd syncfiles-android && ./gradlew assembleDebug
```

**Pasos:**
1. Crear `.github/workflows/ci.yml` con la configuración mínima (check + test para server + models; client si hay tests)
2. Verificar que `cargo check --workspace --all-targets` pasa sin warnings nuevos
3. Verificar que `cargo clippy --workspace` pasa (o documentar whitelist de lint)

---

## 5. Actualizar documentación de estado del proyecto

### 5.1 `README.md`
- **Línea 7:** Cambiar "Fase de Diseño y Arquitectura" → "V1 — Prototipo funcional (sincronización operativa)"
- **Línea 88:** El badge ya dice "Prototipo V1 en implementación" → cambiar a "V1 funcional"
- **Líneas 111-114** (Próximos Pases): Actualizar para reflejar que sync está funcional, los próximos pasos son hardening + CI/CD
- **Líneas 116-131** (Dashboard UI): Verificar que las instrucciones de lanzamiento siguen siendo correctas

### 5.2 `docs/51-backlog-implementacion-v1.md`
- Tarea 4.1 (Pruebas unitarias): marcar **COMPLETADA** al terminar los tests del cliente
- Tarea 4.2 (Pruebas de integración): marcar **COMPLETADA** (server ya tiene 35 tests; client los nuevos)
- Tarea 4.3 (Pruebas de seguridad): marcar **COMPLETADA** al terminar tests de sesión+rate limit
- Tarea 4.4 (Release V1): marcar **COMPLETADA** al tener CI/CD funcional
- Sección de Validación E2E: confirmar que la evidencia sigue siendo válida

### 5.3 `docs/49-roadmap.md`
- Fase 2 (estabilización y hardening): marcar **COMPLETADA**
- Fase 3 (V2 preparación): dejar como PENDING (no aplica aún)

### 5.4 `docs/37-interfaz-usuario.md`
- Verificar que la documentación de pantallas refleja el estado actual (fue actualizada en Fase 5/6 del plan.md)

---

## 6. Verificación de consistencia post-changelog

Verificar que las decisiones del commit `694e0cb` (change_log) están consistentes en todas las partes:

1. **Server:** `diff_handler` consulta `change_log` por cursor seq (no `modified_at`)
2. **Server:** `get_max_seq` inicializa `server_seq` desde `MAX(change_log.seq)`
3. **Server:** Cada handler de mutación anexa al `change_log` (upload/delete/rename/move/copy)
4. **Server:** `rename_file` actualiza `modified_at`
5. **Client desktop:** `pull_remote` maneja `rename`/`move`/`copy` desde `ChangeEntry.old_path`
6. **Client desktop:** `reconcile_local_folder` detecta deletes locales
7. **Client desktop:** `push_local` propaga deletes (`NOT_FOUND` = éxito)
8. **Android:** `ChangeEntry` tiene `old_path: Option<String>`; `SyncWorker` aplica rename/move/copy
9. **Modelos:** `ChangeEntry` en `syncfiles-models/src/lib.rs` tiene `old_path: Option<String>`
10. **Scripts:** `scripts/e2e-rename-move-copy.sh` pasa 27 checks
11. **Docs:** `docs/48-documentacion-usuario.md` dice que `diff <seq>` usa cursor (no timestamp)
12. **Docs:** `docs/51-backlog-implementacion-v1.md` ya no tiene "Limitación conocida" de 3.3

---

## 7. Orden de ejecución

| # | Tarea | Dependencias | Entregable |
|---|---|---|---|
| 1 | Fix migraciones duplicadas | Ninguna | Un solo archivo `001_init.sql` |
| 2 | Tests unitarios cliente (`syncfiles-client`) | 1 | `cargo test --package syncfiles-client` pasa |
| 3 | Tests de seguridad (sesión expirada + rate limit) | 1 | Tests nuevos en `api_endpoints.rs` |
| 4 | Pipeline CI/CD | 2, 3 | `.github/workflows/ci.yml` |
| 5 | Actualizar docs de estado | 2, 3, 4 | README, backlog, roadmap actualizados |
| 6 | Verificación consistencia post-changelog | 1 | Check manual/automático |

---

## 8. Validación final

- [ ] `cargo check --workspace --all-targets` pasa sin warnings nuevos
- [ ] `cargo test --package syncfiles-models` pasa (15 tests existentes)
- [ ] `cargo test --package syncfiles-server` pasa (35+ tests, incluidos nuevos de seguridad)
- [ ] `cargo test --package syncfiles-client` pasa (15+ tests nuevos)
- [ ] `bash scripts/e2e-phase5.sh` → 17 PASS / 0 FAIL (regresión)
- [ ] `bash scripts/e2e-test.sh` pasa (regresión)
- [ ] `bash scripts/e2e-rename-move-copy.sh` pasa (27 checks)
- [ ] `cargo clippy --workspace --all-targets` pasa o tiene solo whitelist
- [ ] `.github/workflows/ci.yml` válido (verificar con `act` o push de prueba)
- [ ] Epics del backlog marcados COMPLETADA

---

## 9. Referencias

| Qué | Dónde |
|---|---|
| Backlog V1 (estado actual) | `docs/51-backlog-implementacion-v1.md` |
| Roadmap | `docs/49-roadmap.md` |
| Plan completo (fases 1-6, todas `[x]`) | `plan.md` |
| Tests servidor existentes | `syncfiles-server/tests/api_endpoints.rs` (27 tests), `static_dashboard.rs` (8 tests) |
| Tests models existentes | `syncfiles-models/src/lib.rs` (15 tests inline) |
| Tests E2E | `scripts/e2e-test.sh`, `scripts/e2e-phase5.sh`, `scripts/e2e-rename-move-copy.sh` |
| Config server | `syncfiles-server/src/config.rs` (rate limit: 60 rps, burst 120) |
| Middleware rate limit | `syncfiles-server/src/middleware/` |
| Modelo ChangeEntry | `syncfiles-models/src/lib.rs` (ya tiene `old_path: Option<String>`) |
| Schema server | `syncfiles-server/migrations/001_init.sql` |
| Schema client | `syncfiles-models/src/lib.rs` (`schema::CLIENT_INIT_SQL`) |
| Android API | `syncfiles-android/.../SyncFilesApi.kt` |
| Cliente sync | `syncfiles-client/src/sync.rs` |
| Cliente network | `syncfiles-client/src/network.rs` |
| Cliente metadata | `syncfiles-client/src/metadata.rs` |
