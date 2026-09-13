# Plan: Cliente Emacs para SyncFiles (`syncfiles.el`)

> Continuación de la sesión colgada `ses_f6d594c66ffeRAo6CBxlK26NTY` (2026-09-11):
> exploró el server y el contrato de hashes, se atascó en permisos y salió sin
> escribir nada. Este plan parte de cero: **no existe ningún código Emacs** en el repo.

## 0. Contexto y decisiones ya tomadas

- **Alcance**: cliente completo — login/logout, subir (buffer/archivo marcado/dired),
  descargar, listar, borrar, resolver conflictos.
- **Arquitectura**: Elisp puro monolito (`syncfiles.el`), **sin dependencias externas**
  (solo Emacs 29+: `url-retrieve`, `json-parse`, `base64-encode-string`, `sha256` no existe
  como primitive — ver abajo).
- **Sesión**: persistente en `~/.config/syncfiles/` (dir 0700) + auto-login
  (validación con `GET /session/status`, re-autenticación transparente si expiró).
  Email/password vía auth-source si existe; si no, prompt interactivo solo la primera vez.
- **Ubicación**: carpeta nueva `syncfiles-emacs/` con `syncfiles.el` + `README.md`,
  siguiendo el patrón de `syncfiles-cli/`, `syncfiles-android/`, etc.
- **Entorno verificado**: Emacs está instalado en `/opt/homebrew/bin/emacs` (Homebrew)
  y corriendo. El server escucha en `http://127.0.0.1:8080` con dashboard en `/`.

## 1. Contrato del server (verificado en código)

- Base URL: `{server_url}/api/v1`
- Auth: header `Authorization: Bearer <session_id>` en TODAS las llamadas
  (ver `syncfiles-server/src/handlers.rs:857-862`). Además, los cuerpos POST
  replican `session_id` y `device_id`.
- Login: `POST /auth/login` con `{email, password, device_id}` →
  `{session_id, expires_at, device_id, user_id}` (`LoginResponse`).
  Respuesta de error: `ApiResponse` con `error.code=AUTH_FAILED`, HTTP 400.
- Upload: `POST /sync/upload` con
  `{session_id, device_id, file_id, relative_path, path_hash, checksum, size_bytes,
   modified_at, idempotency_key, content}` — `content` en **base64**, `checksum` SHA-256
  hex del contenido, `path_hash` SHA-256 hex de `relative_path` **sin `/` inicial**
  (`compute_path_hash`, syncfiles-models/src/lib.rs:320), `file_id` puede ser `""`
  (el server genera uno nuevo), `idempotency_key` UUID v4, `modified_at` en ms epoch.
  - 409 si el checksum difiere del existente → conflicto (`data.conflict_id`).
  - 400 `INVALID_PATH_HASH` si `path_hash` no coincide con `relative_path`.
  - Normalización server: `a/b/../c` → `a/c`, `./a/` → `a`, `a//b` → `a/b`;
    rechaza `..` que escape de la raíz.
- Download: `POST /sync/download` con `{session_id, device_id, file_id, path_hash,
  idempotency_key}` → `DownloadResponse {checksum, content (base64), file_id, server_seq}`.
- Delete: `POST /sync/delete` con `{session_id, device_id, file_id, path_hash,
  idempotency_key}`.
- Files list: `GET /files/list?include_deleted=true` →
  `{files: [{file_id, relative_path, path_hash, checksum, size_bytes, modified_at,
  synced_at, status, deleted_at, device_id}], total, server_seq}`.
- Conflicts: `GET /conflicts` → `{conflicts: [...], total}`; resolver con
  `POST /conflicts/resolve` `{session_id, device_id, conflict_id, decision
  ("keep_local"|"keep_remote"), preserve_alternative (bool), new_name}`.
- Session status: `GET /session/status` → 200 con `Session` si válida, 401 si no.
- Logout: `POST /auth/logout` (con Bearer).
- LoginResponse y DownloadResponse **NO** siguen el patrón `ApiResponse`; el resto sí
  (`accepted`, `status`, `data`, `error`).

## 2. Tareas

### T1 — Esqueleto del paquete `syncfiles-emacs/syncfiles.el`
- Header con `;;; Commentary`, `;;; Code`, grupo `customize` `syncfiles` con:
  - `syncfiles-server-url` (default `http://127.0.0.1:8080`)
  - `syncfiles-device-id` (default: `(system-name)` con sufijo "-emacs")
  - `syncfiles-config-dir` (default `~/.config/syncfiles/`)
  - `syncfiles-default-remote-dir` (default `""` — raíz del storage del usuario)
- Variables internas: `syncfiles--session` (plist `:session_id :expires_at :user_id
  :device_id`), `syncfiles--credentials` cache en memoria.
- `;;; syncfiles.el ends here` footer.

### T2 — Capa HTTP: `syncfiles--request`
- Función sincrónica sobre `url-retrieve-synchronously` que:
  - arma URL `{server-url}/api/v1{path}`, method POST/GET,
    headers `Content-Type: application/json` + `Authorization: Bearer …`,
    body JSON via `json-encode` (snake_case: construir con alist/plist claves como
    símbolos `:session_id` etc. — `json-encode` respeta `:foo-bar` como `foo-bar`).
  - parsea respuesta con `json-read-from-string` del buffer resultante
    (buscar `\n\n` como separador headers/cuerpo).
  - devuelve plist `(:status N :body …)`; propaga errores HTTP ≥400 como signal
    `syncfiles-error` con `code`/`message` extraídos de `error.message` del body
    cuando exista.
- Decodificar content-type JSON; los tamaños de respuesta de download pueden ser
  grandes → usar `url-retrieve-synchronously` (bloquea; aceptable para V1) y
  documentar la limitación.
- Manejo de respuesta no-JSON (404 HTML del fallback estático) → error claro.

### T3 — Sesión persistente + auto-login
- `syncfiles--config-file` = `{config-dir}/session.json`; guardar con
  `write-region` tras `mkdir -p` con permisos 0700
  (`set-file-modes`); cargar al primer uso.
- Credenciales: `syncfiles--read-credentials` → si `~/.authinfo` tiene entrada
  `machine {host del server} user {email} port syncfiles` usarla; si no, prompt
  `read-passwd` solo la primera vez y recordar en memoria (no persistir password
  en disco nunca).
- `syncfiles-ensure-session` (defvar para no alertar del compilador):
  1. si hay sesión guardada y `GET /session/status` da 200 → usarla
  2. si falla → login con credenciales → guardar `session.json`
- `M-x syncfiles-login` interactivo (URL opcional), `M-x syncfiles-logout`
  (llama endpoint + borra `session.json`).
- `device_id`: persistir en `session.json` junto con `session_id` para reuso.

### T4 — Upload: `M-x syncfiles-upload` (y variantes)
- `syncfiles--upload-file (local-path relative-path)`:
  - lee bytes con `insert-file-contents-literally` en temp buffer,
  - `base64-encode-string` (con NOOP newline handling: usar
    `base64-encode-string` sobre `buffer-substring-no-properties` con
    `enable-multibyte-characters` nil / `string-as-unibyte` para binarios),
  - checksum: SHA-256 → Emacs 29 NO tiene `sha256` builtin. Opciones:
    `openssl dgst -sha256` via `call-process-region` (disponible en macOS),
    o `secure-hash` con `'sha256` — **`secure-hash` SÍ existe desde Emacs 25**
    y devuelve hex. Usar `(secure-hash 'sha256 CONTENT)` donde CONTENT puede ser
    string unibyte → **decisión: `secure-hash`**, cero subprocess.
  - `path_hash` = `(secure-hash 'sha256 (string-remove-prefix "/" relative-path))`.
  - `modified_at` = `(time-convert (file-attribute-modification-time
    (file-attributes local-path)) 'integer)` → ms (multiplicar por 1000).
  - `size_bytes` = `nth 7` de `file-attributes`.
  - `idempotency_key` = UUID: Emacs 29 no tiene `uuidgen` elisp builtin;
    generar con `(secure-hash 'sha256 (concat (format "%s%s%s%s%s" (random)
    (emacs-pid) (current-time-string) (system-name) total-process-time)) )` truncado,
    o más simple: usar `md5` de `org-id`? — decisión: **UUID v4 manual** con
    `random` seedeado 2 veces (`(random t)` en load) — formato canónico 8-4-4-4-12.
- Interfaz:
  - `M-x syncfiles-upload` (buffer file o archivo marcado; en dired: archivo bajo
    punto). Prefix arg `C-u` = subir archivo arbitrario con `read-file-name`.
  - `relative_path` = ruta relativa al proyecto (`project-current-root` si existe,
    fallback: nombre base del archivo o `read-string`).
  - Upload de región marcada → temp file con sufijo.
  - Siempre pedir confirmación con `y-or-n-p` mostrando `relative_path` y tamaño.
  - Resultado en `*SyncFiles*` buffer + `message` de éxito con `server_seq`.
  - Manejo 409: leer `data.conflict_id` y `data.alternative_preserved_path`,
    mostrar aviso claro con `display-warning` o buffer `*SyncFiles*` indicando
    que el archivo está en conflicto (la resolución se hace con T7).

### T5 — Listado y descarga
- `M-x syncfiles-list-files`: `GET /files/list` → tabla en buffer `*SyncFiles Files*`
  con `tabulated-list-mode` (columnas: ruta, tamaño humano (`file-size-human-readable`),
  modificado (`format-time-string`), estado). `include_deleted` con prefix arg.
  - keymap propio: `d` → delete bajo punto, `RET` → download bajo punto.
- `M-x syncfiles-download (file_id path_hash)`: `POST /sync/download`, decodificar
  `base64-decode-string`, validar checksum contra `checksum` de la respuesta,
  escribir a `read-file-name` destino con `write-region` binario-safe.
  - Punto de entrada interactivo: desde el buffer de listado; también
    `M-x syncfiles-download` standalone pidiendo `file_id` (con completing-read
    sobre el listado cacheado).

### T6 — Delete: `M-x syncfiles-delete`
- Desde el buffer de listado (`d` sobre línea) o standalone con completing-read
  sobre el listado. Confirmación `y-or-n-p` con ruta. Requiere `file_id` + `path_hash`
  del listado.

### T7 — Conflictos: `M-x syncfiles-conflicts`
- `GET /conflicts` → buffer `*SyncFiles Conflicts*` tabulado (id corto, archivo
  vía lookup en listado o mostrando `file_id`, checksums locales/remotos acortados,
  fecha, tipo).
- Acciones por línea: `l` = keep_local, `r` = keep_remote, `p` = keep_local con
  `preserve_alternative=true`. POST `/conflicts/resolve` y refresco del buffer.
  (Tras resolución el server deja el conflicto resuelto; `get_conflicts` ya no
  lo lista.)

### T8 — README de `syncfiles-emacs/`
- Instalación: `use-package` con `:load-path`, config mínima
  (`syncfiles-server-url`), primer uso (auto-login con auth-source o prompt),
  tabla de comandos, sección troubleshooting (server no levanta, 401, conflictos).
- Nota sobre sync completo: el paquete es cliente de operaciones ad-hoc,
  NO sincroniza carpetas completas (eso es `syncfiles-client` desktop).

### T9 — Integración repo + docs
- Añadir `/syncfiles-emacs/` a la sección de ignorados si corresponde (no hay
  builds; nada que ignorar).
- Actualizar `README.md` raíz: lista de componentes con `syncfiles-emacs/` y
  comando de instalación (cargar .el, no cargo).
- Añadir `docs/46-deployment-clientes.md` sección "Cliente Emacs": instalación,
  comandos, limitaciones (ad-hoc, no motor de sincronización).

### T10 — Validación (E2E manual con batch + server real)
1. `cargo run --manifest-path syncfiles-server/Cargo.toml` (o release ya
   construido).
2. Cargar el paquete y verificar byte-compile sin warnings:
   `emacs --batch -Q -L syncfiles-emacs -f batch-byte-compile syncfiles-emacs/syncfiles.el`
3. Script E2E en `syncfiles-emacs/test-e2e.el` (script batch contra server real,
   sin framework de tests — misma filosofía que `scripts/e2e-test.sh`):
   - Levanta el server como lo hace `scripts/e2e-test.sh`: binario debug
     `syncfiles-server/target/debug/syncfiles-server` con env
     `SF_BIND_ADDRESS=127.0.0.1:8081 SF_DATABASE_URL=/tmp/... SF_STORAGE_ROOT=...
     SF_USER_0_EMAIL=admin@syncfiles.local SF_USER_0_PASSWORD=syncfiles
     SF_USER_0_ID=user-001`, Workdir `/tmp/syncfiles-emacs-e2e`, wait-for-401 loop.
   - Se ejecuta con `emacs --batch -L syncfiles-emacs -l syncfiles.el -l test-e2e.el`
     leyendo `SF_SERVER_URL` y `SF_EMAIL`/`SF_PASSWORD` del entorno.
   - Pasos: login → upload texto + binario aleatorio (`random` 4KB) → files/list
     contiene ambos → download round-trip byte-idéntico (comparar buffers unibyte)
     → upload modificado de segunda sesión → 409 conflicto esperado →
     `GET /conflicts` lista 1 → resolve keep_local → conflicto desaparece →
     delete → files/list ya no lo lista → logout.
   - Salida PASS/FAIL por paso; exit code no-cero si falla.
5. Criterio de aceptación: script E2E todo PASS + `git status` limpio de
   artefactos generados.

## 3. Riesgos y mitigaciones

| Riesgo | Mitigación |
|---|---|
| `url-retrieve` y JSON con caracteres no-ASCII en rutas | Enviar `file-name` codificado UTF-8; probar con acentos en E2E (`tortita ñoña.txt`) |
| Base64 de binarios corrupto por multibyte | Usar `string-as-unibyte` / buffer literal antes de encode |
| Checksum mismatch por CRLF o codificación | Leer siempre literal (`insert-file-contents-literally`); E2E con binario |
| Descargas grandes bloquean UI (`url-retrieve-synchronously`) | Aceptado V1; documentar; opcional `run-at-time` async en V2 |
| Sesión expira a mitad de operación | `ensure-session` antes de cada comando; si 401 intermedio → re-login una vez y retry |
| `random` no seedeado → idempotency_key colisiones | `(random t)` en load + combinar `(emacs-pid)` + timestamp |
| Server no corriendo | Error claro con `user-error` indicando URL y `cargo run` hint |
| 404 HTML (fallback estático) parseado como JSON | Detectar content-type no-JSON y reportar "endpoint inexistente o server sin API" |

## 4. Fuera de alcance (V2)

- Sincronización de carpetas completas / motor de polling / watcher
- Upload de archivos >10MB eficientes (streaming/base64 pesado)
- Rename/move/copy remotos desde Emacs (los endpoints existen; añadir si hay demanda)
- Interfaz async (no bloqueante) con notificaciones
- MELPA / paquete distribuido
