# syncfiles-emacs

Cliente Emacs para SyncFiles. Sube, descarga, lista y borra archivos del
servidor, y resuelve conflictos, todo desde Emacs. Sin dependencias externas:
solo Emacs 29+ con `url-retrieve` + `json` (built-in).

El paquete hace operaciones **ad-hoc** sobre el storage del usuario; NO es un
motor de sincronización de carpetas completas (eso es `syncfiles-client`, el
cliente desktop).

## Instalación

```elisp
;; Con use-package:
(use-package syncfiles
  :load-path "~/ruta/al/repo/syncfiles-emacs"
  :custom
  (syncfiles-server-url "http://127.0.0.1:8080"))

;; O a mano:
(add-to-list 'load-path "~/ruta/al/repo/syncfiles-emacs")
(require 'syncfiles)
(setq syncfiles-server-url "http://127.0.0.1:8080")
```

## Primer uso

Cualquier comando autentica si hace falta (`syncfiles-ensure-session`).
Hay dos formas de dar credenciales:

1. **auth-source** (recomendado): añade a `~/.authinfo` (o `~/.authinfo.gpg`):

   ```
   machine 127.0.0.1 port syncfiles login admin@syncfiles.local password tu-password
   ```

   donde `127.0.0.1` es el host de `syncfiles-server-url`.

2. **Prompt interactivo**: si auth-source no tiene entrada, Emacs pregunta
   email y contraseña solo la primera vez (queda en memoria; la contraseña
   nunca se escribe en disco).

La sesión (`session_id`, `device_id`, `user_id`) se persiste en
`~/.config/syncfiles/session.json` con permisos 0600 (directorio 0700) y se
revalida automáticamente con `GET /session/status`; si expiró, el cliente
re-autentica de forma transparente.

## Comandos

| Comando | Descripción |
|---|---|
| `M-x syncfiles-login` | Inicia/renueva sesión. Con `C-u`, pide URL del server primero. |
| `M-x syncfiles-logout` | Cierra sesión en el server y borra session.json local. |
| `M-x syncfiles-upload` | Sube el archivo del buffer actual. En dired, el archivo bajo el punto. Con `C-u`, pide archivo arbitrario. |
| `M-x syncfiles-upload-region` | Sube la región marcada como archivo remoto. |
| `M-x syncfiles-list-files` | Lista archivos remotos (tabulado). Con `C-u`, incluye borrados. `RET` descarga, `d` borra, `g` refresca. |
| `M-x syncfiles-download` | Descarga por file_id (completing-read sobre el listado). Verifica checksum. |
| `M-x syncfiles-delete` | Borra por file_id (completing-read sobre el listado). |
| `M-x syncfiles-conflicts` | Lista conflictos. `l` keep_local, `r` keep_remote, `p` keep_local preservando la alternativa, `g` refresca. |

La ruta remota por defecto es relativa a la raíz del proyecto actual
(`project-current-root`) o, si no hay proyecto, el nombre base del archivo. El
prefijo `syncfiles-default-remote-dir` (default: raíz) agrupa los uploads.

## Tests

```bash
# Byte-compile sin warnings:
emacs --batch -Q -L syncfiles-emacs -f batch-byte-compile syncfiles-emacs/syncfiles.el

# E2E contra un server real (levanta el server como scripts/e2e-test.sh):
SF_BIND_ADDRESS=127.0.0.1:8081 \
SF_DATABASE_URL=sqlite:/tmp/sf-e2e/data/syncfiles.db \
SF_SERVER_URL=http://127.0.0.1:8081 \
SF_STORAGE_ROOT=/tmp/sf-e2e/storage \
SF_USER_0_EMAIL=admin@syncfiles.local \
SF_USER_0_PASSWORD=syncfiles \
SF_USER_0_ID=user-001 \
  ./syncfiles-server/target/debug/syncfiles-server &

SF_SERVER_URL=http://127.0.0.1:8081 \
SF_EMAIL=admin@syncfiles.local SF_PASSWORD=syncfiles \
  emacs --batch -Q -L syncfiles-emacs -l syncfiles.el -l syncfiles-emacs/test-e2e.el
```

El E2E valida: login + persistencia, upload texto UTF-8 (con acentos/ñ) y
binario, listado, download round-trip byte-idéntico, conflicto 409, resolución
keep_local, delete y logout. Sale con exit code 0 si todo pasa.

## Limitaciones (V1)

- Las operaciones HTTP son síncronas (`url-retrieve-synchronously`): Emacs
  bloquea mientras duran. Para archivos grandes, la codificación base64 en
  memoria pesa ~1.33× el tamaño del archivo.
- No sincroniza carpetas completas ni vigila cambios; es un cliente de
  operaciones ad-hoc.
- Rename/move/copy remotos no están expuestos (los endpoints existen; se
  pueden añadir si hay demanda).

## Troubleshooting

- **"Sin respuesta de … (timeout)"**: el server no está corriendo en
  `syncfiles-server-url`. Arráncalo con
  `cargo run --manifest-path syncfiles-server/Cargo.toml`.
- **"Sesión inválida o expirada" (401)**: la sesión murió; el siguiente
  comando re-autentica solo. Si persiste, revisa la contraseña de auth-source
  o haz `M-x syncfiles-logout` seguido de cualquier comando.
- **"El servidor devolvió un cuerpo no-JSON"**: la URL apunta al dashboard
  estático (fallback) o a un endpoint que no es del API; comprueba
  `syncfiles-server-url` y que el server tenga `/api/v1`.
- **Conflicto al subir (409)**: el archivo remoto tiene otro checksum. El
  server preserva la versión alternativa como `<ruta>.conflict_<id>_<ts>`;
  resuélvelo con `M-x syncfiles-conflicts`.
