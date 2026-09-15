# Plan: Pull de rename/move/copy remotos (Tarea 3.3) + deletes locales + changelog del servidor

## Contexto y diagnóstico (verificado en código)

La "limitación conocida" de 3.3 no es solo un gap del cliente: **el server nunca emite `rename`/`move`/`copy` en el diff**. Hallazgos:

1. `diff_handler` (syncfiles-server/src/handlers.rs:111-124) deriva la operación del *estado* de `files`: solo produce `upload` o `delete`. Las ramas `"rename"|"move"` y `"copy"` de `syncfiles-client/src/sync.rs:215-223` son **código muerto**.
2. `db::rename_file` (syncfiles-server/src/db.rs:681) actualiza `relative_path`/`path_hash` pero **no `modified_at`** → un rename ni siquiera aparece en el diff actual (que filtra `modified_at > since`).
3. **Bug de semántica de `since`**: el cliente envía `since = last_server_seq` (contador: 1, 2, 3…) pero el server lo compara contra `modified_at` (epoch-millis) → **cada diff devuelve TODOS los archivos siempre**; el pull "funciona" re-descargando todo en cada ciclo.
4. **Gap extra (aprobado por el usuario)**: los deletes locales nunca se propagan. El watcher encola todo como `pending` upload, `reconcile_local_folder` no detecta desapariciones, y la rama `status == "deleted"` de `push_local` (sync.rs:289) es código muerto.
5. `resolve_conflict_handler` con decisión `rename`/`copy` muta paths/files sin notificar a otros dispositivos.

## Diseño (decisiones confirmadas con el usuario)

### Tabla `change_log` en el server (fuente de verdad del diff)

En vez de derivar el diff del estado de `files`, cada mutación **anexa una entrada** a un log:

```sql
CREATE TABLE IF NOT EXISTS change_log (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT NOT NULL,
    file_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    operation TEXT NOT NULL,          -- upload | delete | rename | move | copy
    path_hash TEXT NOT NULL,
    relative_path TEXT NOT NULL,     -- ruta destino (actual tras la mutación)
    old_path TEXT,                   -- solo rename/move
    checksum TEXT NOT NULL,
    size_bytes INTEGER,
    modified_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_change_log_user_seq ON change_log(user_id, seq);
```

- `since` del diff pasa a significar **cursor de `seq`** (coincide con docs/06 y docs/07 §7.2 "orden de aplicación por server_seq"; el cliente desktop y Android ya guardan `diff.server_seq` y lo reenvían — no cambia su lado).
- `diff_handler` consulta: `WHERE user_id = ? AND seq > since AND device_id != ? ORDER BY seq ASC LIMIT 500`.
  - `server_seq` de la respuesta = `MAX(seq)` de las filas devueltas; si no hay filas, `MAX(seq)` global (`COALESCE 0`). Si LIMIT trunca, el cursor queda en la última devuelta y el resto llega en el ciclo siguiente.
- El seq lo asigna AUTOINCREMENT (sin carrera entre handlers); las mutaciones devuelven ese seq como `server_seq` de su respuesta en lugar de `state.next_server_seq()`. En `state.rs`, inicializar el contador decorativo con `MAX(change_log.seq)` (los handlers de solo lectura siguen usándolo).
- El cliente sigue avanzando el cursor solo tras aplicar todo el ciclo (comportamiento actual: si un apply falla, `?` corta el ciclo y `set_last_server_seq` no se ejecuta → reintento).
- `ChangeEntry` (syncfiles-models) gana `old_path: Option<String>`:
  - serde: Option ausente → `None`; campos desconocidos ignorados → compatible en ambas direcciones.
  - Android Gson: añadir `val old_path: String? = null` — tolerante igualmente.

### Backfill en arranque (aprobado)

En `db::init_db`: si `change_log` está **vacío** y `files` tiene filas, un solo `INSERT..SELECT` crea una entrada por archivo (`upload`, o `delete` si `status='deleted'`, con `modified_at = COALESCE(deleted_at, modified_at)`). Así los clientes con cursor antiguo o nuevo reciben estado completo y los cambios pre-migración no se pierden.

### Handlers que anexan al log

| Handler | Entrada |
|---|---|
| `upload_handler` | `upload` (tras upsert; NO en replay idempotente — el early-return ya está antes) |
| `delete_handler` | `delete` (path antes del soft-delete) |
| `rename_handler` / `move_handler` | `rename` / `move` con `old_path` + `relative_path` nueva |
| `copy_handler` | `copy` con el **nuevo** file_id y path destino |
| `resolve_conflict_handler` | decisión `rename` → entrada `rename`; decisión `copy` → entrada `copy` (dispositivo = quien resuelve). `keep*` no genera entrada. |

Idealmente envolver mutación de `files` + append en una transacción sqlx cuando ambas son DB (upload/delete/rename/move/copy); `storage.*` (FS) queda fuera — documentado como gap conocido.

### Cliente desktop (syncfiles-client/src/sync.rs)

- `pull_remote`: las ramas ya existen; ajustes menores:
  - `apply_renaming`: usar `change.old_path` como fallback si no hay fila local por `file_id`.
  - `copy`: mantiene `download_remote_file` (descarga por file_id/path_hash destino).
- `reconcile_local_folder` — **detección de deletes locales**: tras escanear disco, toda fila con estado `synced` **o** `pending` cuyo archivo no esté en disco y cuyo último segmento no empiece con `.` (evita borrar dotfiles/descargas .conflict que scan ignora) → `update_file_status(relative, "deleted")` (ya resetea `synced_at = NULL` → queda "pendiente de push"). No tocar filas `conflict` ni `deleted`. Esto también hace converger renames hechos en Finder (delete+upload).
- `push_local` — nuevo loop de deletes: filas `status='deleted' AND synced_at IS NULL` → re-chequear existencia en disco (si reapareció, volver a `pending`); si no, `client.delete(session, file_id, path_hash)`; respuesta `NOT_FOUND` cuenta como éxito (archivo ya inexistente server-side); al éxito dejar `status='deleted'` y setear `synced_at` (evita re-envíos; `get_files_by_status("synced")` de reconcile no la re-detecta). Reemplazar la rama muerta `if file.status == "deleted"` del loop de pending.
- El watcher y `do_delete_file` de la UI quedan como están (el delete de UI ya no converge el estado local vía reconcile).

### Android (aprobado)

- `SyncFilesApi.kt`: `ChangeEntry` + `old_path: String? = null`.
- `SyncWorker.pullRemote`: ramas nuevas en el `when(change.operation)`:
  - `"rename"`, `"move"`: `old_path → relative_path`; `File.renameTo` con `mkdirs()` del padre; si el origen no existe (replay en instalación nueva), solo upsert de `LocalFile` con el path nuevo; `else ->` pasa de `Unit` silencioso a `Log.i` de op ignorada.
  - `"copy"`: igual que `upload` (download por file_id/path_hash → escribir en `relative_path` → upsert).
- `HomeViewModel` no cambia (solo muestra la lista).

## Tareas (orden de implementación)

1. **syncfiles-models**: `old_path: Option<String>` en `ChangeEntry` + test de roundtrip (patrón existente en `lib.rs` tests).
2. **syncfiles-server/schema**: DDL de `change_log` en `SERVER_INIT_SQL` (CREATE IF NOT EXISTS = migración automática, sin ALTER).
3. **syncfiles-server/db.rs**:
   - `append_change_log(pool, ...) -> Result<i64>` (RETURNING seq).
   - `get_changes_since(pool, user_id, since, exclude_device, limit) -> Vec<ChangeEntry>`.
   - `get_max_seq(pool) -> i64`.
   - Backfill en `init_db` (guard: solo si log vacío).
   - `rename_file`: bump de `modified_at`.
   - Eliminar `get_files_modified_since` (queda sin consumidores).
4. **syncfiles-server/handlers.rs + state.rs**: appends por handler (tabla arriba), `diff_handler` reescrito para leer `change_log`, seq inicial desde `change_log` en `state.rs`.
5. **syncfiles-server tests**: con `#[tokio::test]` + pool sqlite `:memory:` (actix-web dev-deps ya declarados): append→get_changes_since (orden y filtro por device), backfill se ejecuta una sola vez, rename produce entrada con old_path.
6. **syncfiles-client/sync.rs**: ajustes de pull (tarea arriba) + reconcile con detección de deletes + push_local con loop de deletes.
7. **syncfiles-client/metadata.rs**: helpers si hacen falta (`get_deleted_unsynced`, restaurar a pending); tests de store estilo existente.
8. **Android**: `ChangeEntry.old_path` + ramas rename/move/copy en `SyncWorker`.
9. **scripts/e2e-rename-move-copy.sh** (nuevo, modelado en `scripts/e2e-phase5.sh`, reusando sus helpers start_server/stop_server/cli):
   - upload A → rename A → diff B muestra `operation":"rename"` con old_path.
   - move → diff; copy → diff + download de destino con checksum idéntico.
   - delete → diff con `delete`.
   - Cursor: `diff since=<seq>` no repite entradas ya vistas.
   - Backfill/continuidad: reinicio de server → diff con cursor previo no devuelve lo ya aplicado.
   - Cliente desktop headless (`SF_HEADLESS=1`, HOME/SF_DATA_DIR aislados, como phase5): A crea archivo local → sube; CLI renombra en server → el ciclo de A mueve el archivo en su sync_root; A borra un archivo local → siguiente ciclo propaga delete (diff desde CLI lo muestra).
10. **Docs**: actualizar `docs/48-documentacion-usuario.md` (arg de `diff` = seq, no timestamp) y quitar la "Limitación conocida" de `docs/51-backlog-implementacion-v1.md` (Tarea 3.3).

## Validación

- `cargo check && cargo test` en syncfiles-models, syncfiles-server, syncfiles-client, syncfiles-cli.
- `bash scripts/e2e-rename-move-copy.sh` (verde end-to-end).
- Android: `./gradlew :app:assembleDebug` (o al menos `compileDebugKotlin`) si el entorno lo permite.

## Riesgos y limitaciones conocidas

- **Crecimiento ilimitado de `change_log`**: aceptado para V1 (SQLite en NAS, volumen bajo); pruning/compaction queda para V2.
- **Gap de atomicidad FS↔log**: `storage.*` (filesystem) queda fuera de la transacción DB; un crash entre write y log puede dejar un cambio invisible hasta la próxima mutación del archivo. Documentado; outbox real es V2.
- **Despliegue**: server y cliente desktop deben compilarse juntos (un cliente viejo contra el server nuevo saltearía los rename con el log "no implementada" y perdería el cursor).
- Renames hechos fuera de la UI convergen como delete+upload (no como rename atómico) — estado final correcto en todos los dispositivos.
- Rename remoto con edición local pendiente → el push posterior al path nuevo produce 409 y entra al flujo de conflictos existente (correcto, no silencioso).

## Fuera de alcance

- Rate limiting 429 (Tarea 4.3), tests de integración completos (Tarea 4.2), empaquetado/CI (4.4), pruning del changelog, detección de renames locales como operación atómica, refresh de sesión.
