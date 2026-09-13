;;; syncfiles.el --- Cliente Emacs para SyncFiles          -*- lexical-binding: t; -*-

;; Copyright (C) 2026 SyncFiles

;; Author: SyncFiles
;; Keywords: files, sync, tools
;; Package-Requires: ((emacs "29.1"))
;; Version: 0.1.0

;; This program is free software; you can redistribute it and/or modify
;; it under the terms of the GNU General Public License as published by
;; the Free Software Foundation, either version 3 of the License, or
;; (at your option) any later version.

;; This program is distributed in the hope that it will be useful,
;; but WITHOUT ANY WARRANTY; without even the implied warranty of
;; MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
;; GNU General Public License for more details.

;; You should have received a copy of the GNU General Public License
;; along with this program.  If not, see <https://www.gnu.org/licenses/>.

;;; Commentary:

;; Cliente Emacs para el servidor SyncFiles.  Permite subir (buffer,
;; archivo o región), descargar, listar, borrar archivos y resolver
;; conflictos desde Emacs, con sesión persistente y auto-login.
;;
;; Comandos principales:
;;
;;   M-x syncfiles-login            Inicia sesión (valida/renueva).
;;   M-x syncfiles-logout           Cierra sesión y borra la sesión local.
;;   M-x syncfiles-upload           Sube el archivo del buffer actual
;;                                  (o el archivo bajo el punto en dired).
;;                                  Con prefijo C-u, pide archivo arbitrario.
;;   M-x syncfiles-upload-region    Sube la región marcada como archivo.
;;   M-x syncfiles-list-files       Lista archivos remotos en un buffer
;;                                  tabulado (RET descarga, d borra).
;;   M-x syncfiles-download         Descarga un archivo por file_id.
;;   M-x syncfiles-delete           Borra un archivo remoto por file_id.
;;   M-x syncfiles-conflicts        Lista conflictos y resuélvelos
;;                                  (l = keep_local, r = keep_remote,
;;                                   p = keep_local preservando alternativa).
;;
;; Limitaciones V1: las operaciones HTTP son síncronas (bloquean Emacs
;; mientras duran) y no es un motor de sincronización de carpetas; para
;; sincronización completa usa el cliente desktop `syncfiles-client'.

;;; Code:

(require 'json)
(require 'url)
(require 'url-http)
(require 'auth-source)
(require 'dired)
(require 'tabulated-list)

(defgroup syncfiles nil
  "Cliente Emacs para SyncFiles."
  :group 'tools
  :prefix "syncfiles-")

(defcustom syncfiles-server-url "http://127.0.0.1:8080"
  "URL base del servidor SyncFiles."
  :type 'string
  :group 'syncfiles)

(defcustom syncfiles-device-id (concat (system-name) "-emacs")
  "Identificador de dispositivo reportado al servidor."
  :type 'string
  :group 'syncfiles)

(defcustom syncfiles-config-dir "~/.config/syncfiles/"
  "Directorio (0700) donde se persiste session.json.
Contiene session_id, device_id y user_id.  La contraseña nunca se
persiste en disco."
  :type 'directory
  :group 'syncfiles)

(defcustom syncfiles-default-remote-dir ""
  "Prefijo de directorio remoto por defecto para uploads.
\"\" significa la raíz del storage del usuario."
  :type 'string
  :group 'syncfiles)

(defvar syncfiles--session nil
  "Sesión activa como plist (:session_id :expires_at :user_id :device_id).
Se carga desde session.json y se refresca con `syncfiles-ensure-session'.")

(defvar syncfiles--credentials nil
  "Credenciales en memoria como plist (:email :password).
Cache de `syncfiles--read-credentials'; nunca se persisten en disco.")

;; Seed del PRNG para idempotency_key / UUIDs; ver `syncfiles--uuid'.
(random t)

;;; Error type

(define-error 'syncfiles-error
  "Error del cliente SyncFiles" '(user-error))

(defun syncfiles--signal (message &optional code status)
  "Signal `syncfiles-error' con MESSAGE y detalles CODE y STATUS HTTP."
  (signal 'syncfiles-error
          (list (if code
                    (format "%s [%s]" message code)
                  message)
                code status)))

;;; HTTP layer

(defvar syncfiles--response-timeout 60
  "Timeout en segundos de `url-retrieve-synchronously'.")

(cl-defun syncfiles--request (path &key (method "GET") body token noerror)
  "Hace una petición HTTP a PATH del API de SyncFiles.
METHOD es \"GET\" o \"POST\".  BODY, si es non-nil, es un objeto Lisp
que se codifica como JSON.  TOKEN es el session_id para el header
Authorization; si es nil se envía un Bearer anónimo para evitar que
url-http lance el prompt interactivo de autenticación en un 401.
Con NOERROR non-nil, los estados HTTP >= 400 no se señalan como
error y se devuelven tal cual (para tratar p.ej. el 409 de upload).
Devuelve una plist (:status NÚMERO :body PLIST-JSON).
Errores HTTP >= 400 (sin NOERROR) se señalan como `syncfiles-error'
extrayendo error.message del cuerpo cuando existe."
  (let* ((url (concat (string-remove-suffix "/" syncfiles-server-url)
                      "/api/v1" path))
         (url-request-method method)
         (url-request-extra-headers
          `(("Content-Type" . "application/json")
            ("Authorization" . ,(concat "Bearer " (or token "anonymous")))))
         (url-request-data
          (when body
            ;; url-http rechaza strings multibyte en el cuerpo; el JSON
            ;; va codificado UTF-8 puro (unibyte).
            (encode-coding-string (syncfiles--json-encode body) 'utf-8)))
         (buffer (url-retrieve-synchronously url t syncfiles--response-timeout))
         status headers content-type body-text)
    (unless buffer
      (syncfiles--signal
       (format "Sin respuesta de %s (timeout de %ss).  ¿Está corriendo el servidor? (cargo run --manifest-path syncfiles-server/Cargo.toml)"
               url syncfiles--response-timeout)))
    (unwind-protect
        (with-current-buffer buffer
          (goto-char (point-min))
          ;; url-http deja el punto tras el separador headers/cuerpo.
          (unless (re-search-forward "^HTTP/1\\.[01] +\\([0-9]+\\)" nil t)
            (syncfiles--signal
             (format "Respuesta HTTP inválida de %s (¿proxy en medio?)" url)))
          (setq status (string-to-number (match-string 1)))
          (re-search-forward "^\r?\n" nil t)
          (setq headers (buffer-substring (point-min) (match-beginning 0)))
          (setq body-text (buffer-substring (point) (point-max)))
          (when (string-match-p "content-type: *application/json" headers)
            (setq content-type 'json))
          (when content-type
            ;; El cuerpo llega como bytes crudos; decodificamos UTF-8.
            (setq body-text (decode-coding-string
                             (encode-coding-string body-text 'utf-8)
                             'utf-8))))
      (kill-buffer buffer))
    (unless content-type
      (syncfiles--signal
       (format "El servidor devolvió un cuerpo no-JSON (HTTP %d).  ¿Endpoint inexistente o servidor sin API?" status)
       nil status))
    (let ((parsed (syncfiles--json-read body-text)))
      (when (and (>= status 400) (not noerror))
        ;; Dos formatos de error: ApiResponse con error.message, o el
        ;; plano de actix {code, message} (p.ej. UNAUTHORIZED).
        (let ((err-msg (plist-get (plist-get parsed :error) :message))
              (err-code (plist-get (plist-get parsed :error) :code))
              (flat-msg (plist-get parsed :message))
              (flat-code (plist-get parsed :code)))
          (syncfiles--signal (cond
                             ((and err-msg
                                   (stringp err-msg)
                                   (not (equal err-msg "")))
                              err-msg)
                             ((and flat-msg (stringp flat-msg))
                              flat-msg)
                             (t (format "HTTP %d en %s" status path)))
                             (or err-code flat-code)
                             status)))
      (list :status status :body parsed))))

(defun syncfiles--json-read (text)
  "Parsea TEXT como JSON devolviendo plists con claves :snake_case."
  (let ((json-object-type 'plist)
        (json-array-type 'list)
        (json-false :false)
        (json-null :null))
    (json-read-from-string text)))

(defun syncfiles--json-encode (object)
  "Codifica OBJECT a JSON usando las convenciones del paquete.
Los booleans se codifican con los valores de json-false/json-null."
  (let ((json-false :false)
        (json-null :null))
    (json-encode object)))

;;; Session persistence

(defun syncfiles--config-file ()
  "Ruta de session.json dentro de `syncfiles-config-dir'."
  (expand-file-name "session.json" syncfiles-config-dir))

(defun syncfiles--save-session (session)
  "Persiste SESSION (plist) en session.json con permisos 0700."
  (let ((dir (file-name-as-directory
              (expand-file-name syncfiles-config-dir))))
    (unless (file-exists-p dir)
      (make-directory dir t))
    (set-file-modes dir #o700)
    (let ((text (syncfiles--json-encode
                 `((session_id . ,(plist-get session :session_id))
                   (device_id . ,(plist-get session :device_id))
                   (user_id . ,(plist-get session :user_id))
                   (expires_at . ,(plist-get session :expires_at))))))
      (let ((coding-system-for-write 'utf-8))
        (write-region text nil (syncfiles--config-file) nil 'quiet))
      (set-file-modes (syncfiles--config-file) #o600))))

(defun syncfiles--load-session ()
  "Carga session.json si existe; devuelve plist o nil."
  (let ((file (syncfiles--config-file)))
    (when (file-exists-p file)
      (ignore-errors
        (syncfiles--json-read
         (with-temp-buffer
           (insert-file-contents-literally file)
           (buffer-string)))))))

(defun syncfiles--clear-session ()
  "Borra session.json del disco."
  (let ((file (syncfiles--config-file)))
    (when (file-exists-p file)
      (delete-file file))))

;;; Credentials (auth-source o prompt)

(defun syncfiles--auth-source-lookup ()
  "Busca credenciales en auth-source para el host de `syncfiles-server-url'.
Requiere una línea del estilo:
  machine 127.0.0.1 port syncfiles user admin@syncfiles.local password s3cr3t
Devuelve (:email ... :password ...) o nil."
  (ignore-errors
    (let* ((url-obj (url-generic-parse-url syncfiles-server-url))
           (host (url-host url-obj))
           (found (auth-source-search :host host
                                      :port "syncfiles"
                                      :require '(:user :secret)
                                      :max 1)))
      (when found
        (let ((entry (car found)))
          (list :email (plist-get entry :user)
                :password
                (let ((secret (plist-get entry :secret)))
                  (if (functionp secret) (funcall secret) secret))))))))

(defun syncfiles--read-credentials ()
  "Devuelve credenciales (:email :password) con cache en memoria.
Primero auth-source (machine HOST port syncfiles); si no existe,
prompt interactivo solo la primera vez por sesión de Emacs."
  (or syncfiles--credentials
      (let ((found (syncfiles--auth-source-lookup)))
        (setq syncfiles--credentials
              (or found
                  (let ((email (read-string "SyncFiles email: "))
                        (password (read-passwd "SyncFiles password: ")))
                    (list :email email :password password)))))))

;;; Auto-login

(defun syncfiles--session-valid-p (session)
  "Devuelve t si SESSION es usable (hay session_id y no expiró)."
  (and session
       (stringp (plist-get session :session_id))
       (not (string-empty-p (plist-get session :session_id)))))

(defun syncfiles--login (email password device-id)
  "Login contra el server; devuelve plist de sesión."
  (let* ((response (syncfiles--request
                    "/auth/login"
                    :method "POST"
                    :body `((email . ,email)
                            (password . ,password)
                            (device_id . ,device-id))))
         (body (plist-get response :body)))
    (unless (plist-get body :session_id)
      (syncfiles--signal "Login no devolvió session_id"))
    body))

(defun syncfiles--check-session-status (session)
  "Devuelve t si SESSION sigue viva según GET /session/status."
  (= (plist-get (syncfiles--request
                 "/session/status"
                 :token (plist-get session :session_id))
                :status)
     200))

(defun syncfiles-ensure-session ()
  "Asegura una sesión válida, re-autenticando si expiró.
Devuelve la plist de sesión (:session_id :expires_at :user_id :device_id).
Signal `syncfiles-error' si no hay forma de autenticar."
  (or
   ;; 1. Sesión en memoria: validar remotamente.
   (and (syncfiles--session-valid-p syncfiles--session)
        (or (and (syncfiles--check-session-status syncfiles--session)
                 syncfiles--session)
            ;; Expiró o fue revocada: descartar.
            (progn
              (setq syncfiles--session nil)
              (syncfiles--clear-session)
              nil)))
   ;; 2. Sesión guardada en disco: validar remotamente.
   (let ((saved (syncfiles--load-session)))
     (when (syncfiles--session-valid-p saved)
       (if (syncfiles--check-session-status saved)
           (setq syncfiles--session saved)
         (syncfiles--clear-session)
         nil)))
   ;; 3. Login de verdad con credenciales.
   (syncfiles--do-login)))

(defun syncfiles--do-login ()
  "Login con credenciales (auth-source o prompt) y persiste la sesión."
  (let* ((credentials (syncfiles--read-credentials))
         (session (syncfiles--login
                   (plist-get credentials :email)
                   (plist-get credentials :password)
                   syncfiles-device-id)))
    (setq syncfiles--session session)
    (syncfiles--save-session session)
    session))

(defun syncfiles--retry-once-on-401 (form)
  "Evalúa FORM; si signal syncfiles-error con status 401, re-login y retry."
  (condition-case err
      (funcall form)
    (syncfiles-error
     (if (and (listp (cdr err))
              (eq (nth 2 (cdr err)) 401))
         (progn
           (setq syncfiles--session nil)
           (syncfiles--clear-session)
           (syncfiles-ensure-session)
           (funcall form))
       (signal (car err) (cdr err))))))

;;;###autoload
(defun syncfiles-login (&optional url)
  "Inicia sesión en SyncFiles (interactivo).
Con prefijo o URL no-nil, cambia `syncfiles-server-url' primero."
  (interactive
   (when current-prefix-arg
     (list (read-string "SyncFiles server URL: "
                       syncfiles-server-url))))
  (when url
    (setq syncfiles-server-url url))
  (let ((session (syncfiles-ensure-session)))
    (message "SyncFiles: sesión activa como %s (expira %s)"
             (or (plist-get session :user_id) "?")
             (if (plist-get session :expires_at)
                 (format-time-string "%Y-%m-%d %H:%M"
                                     (floor (plist-get session :expires_at) 1e3))
               "?"))))

;;;###autoload
(defun syncfiles-logout ()
  "Cierra la sesión en el server y borra la sesión local."
  (interactive)
  (when (syncfiles--session-valid-p syncfiles--session)
    (condition-case nil
        (syncfiles--request "/auth/logout"
                            :method "POST"
                            :token (plist-get syncfiles--session :session_id))
      (error nil)))
  (setq syncfiles--session nil)
  (setq syncfiles--credentials nil)
  (syncfiles--clear-session)
  (message "SyncFiles: sesión cerrada"))

;;; Helpers: hash, uuid, tamaño

(defun syncfiles--uuid ()
  "Genera un UUID v4 (string 8-4-4-4-12) con `random'."
  (format "%08x-%04x-%4x-%04x-%08x%04x"
          (random #x100000000)
          (random #x10000)
          (logior #x4000 (random #x1000))     ; versión 4
          (logior #x8000 (random #x4000))     ; variante RFC 4122
          (random #x100000000)
          (random #x10000)))

(defun syncfiles--path-hash (relative-path)
  "SHA-256 hex de RELATIVE-PATH sin '/' inicial (contrato del server)."
  (secure-hash 'sha256
               (encode-coding-string
                (string-remove-prefix "/" relative-path) 'utf-8)))

(defun syncfiles--read-bytes (local-path)
  "Lee LOCAL-PATH como string unibyte (binario-safe)."
  (with-temp-buffer
    (set-buffer-multibyte nil)
    (insert-file-contents-literally local-path)
    (buffer-string)))

(defun syncfiles--buffer-bytes ()
  "Devuelve el contenido del buffer actual como string unibyte."
  (if enable-multibyte-characters
      (encode-coding-string (buffer-substring-no-properties
                             (point-min) (point-max))
                            buffer-file-coding-system)
    (buffer-substring-no-properties (point-min) (point-max))))

(defun syncfiles--now-ms ()
  "Timestamp actual en milisegundos epoch (entero)."
  (floor (* (float-time) 1000)))

(defun syncfiles--file-modified-ms (local-path)
  "mtime de LOCAL-PATH en milisegundos epoch."
  (let ((mtime (file-attribute-modification-time
               (file-attributes local-path))))
    (if mtime
        (floor (* (float-time mtime) 1000))
      (syncfiles--now-ms))))

(defun syncfiles--human-size (n)
  "N bytes en formato humano (uso `file-size-human-readable' si N es entero)."
  (if (integerp n)
      (file-size-human-readable n)
    (format "%s" n)))

;;; Upload

(defun syncfiles--upload-file (local-path relative-path &optional file-id)
  "Sube LOCAL-PATH al RELATIVE-PATH remoto.
FILE-ID, si non-nil, reutiliza el id existente (update en sitio).
Devuelve la plist de respuesta del server.  Signal `syncfiles-error'
en conflictos (409), errores de path, etc."
  (let* ((content (syncfiles--read-bytes local-path))
         (checksum (secure-hash 'sha256 content)))
    (syncfiles--upload-content content checksum relative-path file-id)))

(defun syncfiles--upload-content (content checksum relative-path &optional file-id)
  "Sube CONTENT (string unibyte) con CHECKSUM a RELATIVE-PATH.
Helper común para archivos y regiones.  Un 409 (conflicto) NO se
señala como error: se devuelve la respuesta con :status 409 para
que la capa superior lo presente."
  (let ((session (syncfiles-ensure-session)))
    (syncfiles--retry-once-on-401
     (lambda ()
       (syncfiles--request
        "/sync/upload"
        :method "POST"
        :token (plist-get session :session_id)
        :noerror t
        :body `((session_id . ,(plist-get session :session_id))
                (device_id . ,(plist-get session :device_id))
                (file_id . ,(or file-id ""))
                (relative_path . ,relative-path)
                (path_hash . ,(syncfiles--path-hash relative-path))
                (checksum . ,checksum)
                (size_bytes . ,(string-bytes content))
                (modified_at . ,(syncfiles--now-ms))
                (idempotency_key . ,(syncfiles--uuid))
                (content . ,(base64-encode-string content t))))))))

(defun syncfiles--relative-path-for (local-path)
  "Calcula el relative_path remoto para LOCAL-PATH.
Regla: ruta relativa a la raíz del proyecto actual; si no hay
proyecto, el nombre base del archivo."
  (let* ((abs (expand-file-name local-path))
         (root (and (fboundp 'project-root)
                    (condition-case nil
                        (project-root (project-current))
                      (error nil)))))
    (if (and root (string-prefix-p (expand-file-name root) abs))
        (substring abs (length (file-name-as-directory (expand-file-name root))))
      (file-name-nondirectory abs))))

(defun syncfiles--default-remote-path (relative-path)
  "Prefija RELATIVE-PATH con `syncfiles-default-remote-dir'."
  (if (or (null syncfiles-default-remote-dir)
          (string-empty-p syncfiles-default-remote-dir))
      relative-path
    (concat (string-remove-suffix "/"
                                 (string-remove-prefix "/"
                                                       syncfiles-default-remote-dir))
            "/" relative-path)))

(defun syncfiles--report (fmt &rest args)
  "Escribe FMT/ARGS en el buffer *SyncFiles* y en el minibuffer."
  (let ((text (apply #'format fmt args)))
    (with-current-buffer (get-buffer-create "*SyncFiles*")
      (let ((inhibit-read-only t))
        (goto-char (point-max))
        (insert text "\n"))
      (display-buffer (current-buffer)))
    (message "%s" text)))

(cl-defun syncfiles--handle-upload-response (response relative-path)
  "Procesa RESPONSE de upload mostrando éxito o conflicto."
  (let ((body (plist-get response :body))
        (status (plist-get response :status)))
    (if (or (equal (plist-get body :status) "conflict")
            (= status 409))
        (let ((data (or (plist-get body :data) '())))
          (syncfiles--report
           "Conflicto en %s (conflict_id %s, alternativa preservada: %s).
Usa M-x syncfiles-conflicts para resolver."
           relative-path
           (or (plist-get data :conflict_id) "?")
           (or (plist-get data :alternative_preserved_path) "?")))
      ;; Aceptado (status "ok").
      (syncfiles--report
       "SyncFiles: subido %s (server_seq %s)"
       relative-path (or (plist-get body :server_seq) "?")))))

(defun syncfiles-upload (arg)
  "Sube el archivo del buffer actual a SyncFiles.
Con ARG (C-u) interactivo, pide un archivo arbitrario con
`read-file-name'.  En dired, sube el archivo bajo el punto.
Pide confirmación con la ruta remota y el tamaño antes de subir."
  (interactive "P")
  (let* ((local-path
          (cond
           ;; C-u: archivo arbitrario
           (arg (read-file-name "Subir archivo: "))
           ;; dired: archivo bajo el punto
           ((derived-mode-p 'dired-mode)
            (or (dired-get-filename 'no-dir-if-in-dir-locals)
                (user-error "No hay archivo bajo el punto")))
           ;; Buffer de archivo
           ((buffer-file-name)
            (if (and (buffer-modified-p)
                     (y-or-n-p "Buffer modificado, ¿guardar antes de subir? "))
                (progn (save-buffer) (buffer-file-name))
              (buffer-file-name)))
           (t (user-error "El buffer no visita un archivo; usa C-u M-x syncfiles-upload o syncfiles-upload-region"))))
         (relative-path (syncfiles--default-remote-path
                         (syncfiles--relative-path-for local-path)))
         (size (nth 7 (file-attributes local-path))))
    (unless (y-or-n-p (format "¿Subir %s a SyncFiles como %s (%s)? "
                             local-path relative-path
                             (syncfiles--human-size (or size 0))))
      (user-error "Subida cancelada"))
    (syncfiles--handle-upload-response
     (syncfiles--upload-file local-path relative-path)
     relative-path)))

(defun syncfiles-upload-region (start end)
  "Sube la región START..END como archivo remoto.
La región se escribe en un archivo temporal con sufijo -region y se
sube con el nombre que el usuario elija."
  (interactive "r")
  (when (use-region-p)
    (let* ((default-name (or (and (buffer-file-name)
                                  (concat (file-name-sans-extension
                                           (file-name-nondirectory
                                            (buffer-file-name)))
                                          "-region"
                                          (file-name-extension
                                           (buffer-file-name) t)))
                             "region.txt"))
           (relative-path (syncfiles--default-remote-path
                           (read-string "Nombre remoto del archivo: "
                                        (concat default-name))))
           (content (if enable-multibyte-characters
                         (encode-coding-string
                          (buffer-substring-no-properties start end)
                          (or buffer-file-coding-system 'utf-8))
                       (buffer-substring-no-properties start end)))
           (checksum (secure-hash 'sha256 content)))
      (unless (y-or-n-p (format "¿Subir región como %s (%s)? "
                                relative-path
                                (syncfiles--human-size (string-bytes content))))
        (user-error "Subida cancelada"))
      (syncfiles--handle-upload-response
       (syncfiles--upload-content content checksum relative-path)
       relative-path))))

;;; Files list y download

(defvar syncfiles--files-cache nil
  "Cache del último files/list como plist (:files (...) :total N).")

(defun syncfiles--list-files (&optional include-deleted)
  "GET /files/list; devuelve la plist (:files ... :total ...).
Actualiza `syncfiles--files-cache'."
  (let* ((session (syncfiles-ensure-session))
         (path (if include-deleted
                   "/files/list?include_deleted=true"
                 "/files/list")))
    (syncfiles--retry-once-on-401
     (lambda ()
       (let* ((response (syncfiles--request path
                                            :method "GET"
                                            :token (plist-get session :session_id)))
              (body (plist-get response :body)))
         (setq syncfiles--files-cache body)
         body)))))

(defun syncfiles--shorten (s &optional n)
  "Primeros N chars (default 8) de S + \"…\"."
  (let ((n (or n 8)))
    (if (and s (> (length s) n))
        (concat (substring s 0 n) "…")
      (or s "?"))))

(defun syncfiles--files-entries ()
  "Convierte `syncfiles--files-cache' a entradas tabulated-list."
  (mapcar (lambda (f)
            (let ((file-id (or (plist-get f :file_id) "?"))
                  (rel (or (plist-get f :relative_path) "?"))
                  (size (or (plist-get f :size_bytes) 0))
                  (mod (plist-get f :modified_at))
                  (status (or (plist-get f :status) "?"))
                  (checksum (plist-get f :checksum)))
              (list (propertize file-id 'syncfiles-file f)
                    (vector rel
                            (syncfiles--human-size size)
                            (if mod
                                (format-time-string
                                 "%Y-%m-%d %H:%M" (floor mod 1e3))
                              "?")
                            status
                            (syncfiles--shorten checksum 12)))))
          (plist-get syncfiles--files-cache :files)))

(defun syncfiles--files-buffer ()
  "Devuelve (y crea si no existe) el buffer de listado de archivos."
  (or (get-buffer "*SyncFiles Files*")
      (with-current-buffer (get-buffer-create "*SyncFiles Files*")
        (syncfiles-files-mode)
        (current-buffer))))

(define-derived-mode syncfiles-files-mode tabulated-list-mode
  "SyncFiles-Files"
  "Buffer de archivos remotos de SyncFiles."
  (setq tabulated-list-format [("Ruta" 40 t)
                               ("Tamaño" 10 tabulated-list-sort-numerically-2)
                               ("Modificado" 17 t)
                               ("Estado" 9 t)
                               ("Checksum" 14 nil)]
        tabulated-list-sort-key '("Ruta" . nil))
  (tabulated-list-init-header)
  (local-set-key (kbd "RET") #'syncfiles--files-download-at-point)
  (local-set-key (kbd "d") #'syncfiles--files-delete-at-point)
  (local-set-key (kbd "g") #'syncfiles-list-files))

;;;###autoload
(defun syncfiles-list-files (&optional include-deleted)
  "Lista los archivos remotos en el buffer *SyncFiles Files*.
Con INCLUDE-DELETED (prefijo), incluye los borrados."
  (interactive "P")
  (syncfiles--list-files include-deleted)
  (with-current-buffer (syncfiles--files-buffer)
    (setq tabulated-list-entries (syncfiles--files-entries))
    (tabulated-list-print)
    (display-buffer (current-buffer))
    (message "SyncFiles: %s archivo(s)"
             (or (plist-get syncfiles--files-cache :total) 0))))

(defun syncfiles--file-at-point ()
  "Plist del archivo en la línea del punto en el buffer de listado."
  (or (get-text-property (point) 'syncfiles-file)
      (save-excursion
        (beginning-of-line)
        (get-text-property (point) 'syncfiles-file))
      (user-error "No hay archivo bajo el punto")))

(defun syncfiles--files-download-at-point ()
  "Descarga el archivo bajo el punto."
  (interactive)
  (let ((f (syncfiles--file-at-point)))
    (syncfiles-download (plist-get f :file_id) (plist-get f :path_hash))))

(defun syncfiles--files-delete-at-point ()
  "Borra el archivo remoto bajo el punto."
  (interactive)
  (let ((f (syncfiles--file-at-point)))
    (syncfiles-delete (plist-get f :file_id)
                      (plist-get f :path_hash)
                      (plist-get f :relative_path))))

(defun syncfiles--file-candidates ()
  "Lista de candidatos \"ruta  file_id\" para completing-read."
  (mapcar (lambda (f)
            (format "%s  %s"
                    (or (plist-get f :relative_path) "?")
                    (or (plist-get f :file_id) "?")))
          (plist-get (or syncfiles--files-cache
                         (syncfiles--list-files))
                     :files)))

(defun syncfiles--read-file-id ()
  "Pide file_id al usuario con completing-read sobre el listado."
  (unless (plist-get (or syncfiles--files-cache (syncfiles--list-files)) :files)
    (user-error "No hay archivos remotos"))
  (let* ((candidates (syncfiles--file-candidates))
         (chosen (completing-read "Archivo (ruta  file_id): " candidates nil t))
         (entry (and (string-match "  \\(.+\\)\\'" chosen)
                     (match-string 1 chosen))))
    (or entry (user-error "Selección inválida"))))

;;;###autoload
(defun syncfiles-download (file-id path-hash)
  "Descarga FILE-ID (con PATH-HASH) del server y lo guarda localmente.
Pregunta el destino con `read-file-name'; verifica el checksum tras
decodificar base64.  Interactivo, usa completing-read sobre el
listado cacheado."
  (interactive
   (progn
     (unless syncfiles--files-cache
       (syncfiles--list-files))
     (let* ((file-id (syncfiles--read-file-id))
            (entry (seq-find (lambda (f)
                               (equal (plist-get f :file_id) file-id))
                             (plist-get syncfiles--files-cache :files))))
       (list file-id (plist-get (or entry
                                    (user-error "file_id no encontrado"))
                                :path_hash)))))
  (unless (and file-id (stringp file-id))
    (user-error "syncfiles-download: falta file_id"))
  (let* ((session (syncfiles-ensure-session))
         (response (syncfiles--retry-once-on-401
                    (lambda ()
                      (syncfiles--request
                       "/sync/download"
                       :method "POST"
                       :token (plist-get session :session_id)
                       :body `((session_id . ,(plist-get session :session_id))
                               (device_id . ,(plist-get session :device_id))
                               (file_id . ,file-id)
                               (path_hash . ,(or path-hash ""))
                               (idempotency_key . ,(syncfiles--uuid)))))))
         (body (plist-get response :body))
         (b64 (plist-get body :content))
         (remote-checksum (plist-get body :checksum)))
    (unless b64
      (syncfiles--signal "Download no devolvió contenido"))
    (let* ((content (base64-decode-string b64))
           (local-checksum (secure-hash 'sha256 content))
           (relative-path (or (plist-get (seq-find
                                          (lambda (f)
                                            (equal (plist-get f :file_id) file-id))
                                          (plist-get (or syncfiles--files-cache
                                                         (syncfiles--list-files))
                                                     :files))
                                         :relative_path)
                              file-id))
           (dest (read-file-name "Guardar como: "
                                 nil
                                 (file-name-nondirectory relative-path))))
      (unless (equal local-checksum remote-checksum)
        (syncfiles--signal
         (format "Checksum mismatch al descargar %s (esperado %s, obtenido %s)"
                 file-id remote-checksum (syncfiles--shorten local-checksum 12))))
      (let ((coding-system-for-write 'no-conversion))
        (write-region content nil dest nil 'quiet))
      (syncfiles--report "SyncFiles: %s descargado en %s (server_seq %s)"
                         relative-path dest (or (plist-get body :server_seq) "?"))
      dest)))

;;;###autoload
(defun syncfiles-delete (file-id path-hash &optional relative-path)
  "Borra FILE-ID del server (con PATH-HASH para el cuerpo).
RELATIVE-PATH es solo para el mensaje de confirmación.  Interactivo,
usa completing-read sobre el listado cacheado."
  (interactive
   (progn
     (unless syncfiles--files-cache
       (syncfiles--list-files))
     (let* ((file-id (syncfiles--read-file-id))
            (entry (seq-find (lambda (f)
                               (equal (plist-get f :file_id) file-id))
                             (plist-get syncfiles--files-cache :files))))
       (list file-id
             (plist-get (or entry (user-error "file_id no encontrado"))
                        :path_hash)
             (plist-get (or entry (user-error "file_id no encontrado"))
                        :relative_path)))))
  (when (and (not noninteractive)
             (not (y-or-n-p (format "¿Borrar %s de SyncFiles? "
                                    (or relative-path file-id)))))
    (user-error "Borrado cancelado"))
  (let* ((session (syncfiles-ensure-session)))
    (syncfiles--retry-once-on-401
     (lambda ()
       (syncfiles--request
        "/sync/delete"
        :method "POST"
        :token (plist-get session :session_id)
        :body `((session_id . ,(plist-get session :session_id))
                (device_id . ,(plist-get session :device_id))
                (file_id . ,file-id)
                (path_hash . ,(or path-hash ""))
                (idempotency_key . ,(syncfiles--uuid)))))))
  (syncfiles--report "SyncFiles: %s borrado" (or relative-path file-id))
  ;; Refrescar cache para que listados posteriores no muestren el archivo.
  (when (plist-get syncfiles--files-cache :files)
    (setq syncfiles--files-cache nil)))

;;; Conflicts

(defvar syncfiles--conflicts-cache nil
  "Cache del último GET /conflicts como plist (:conflicts (...) :total N).")

(defun syncfiles--list-conflicts ()
  "GET /conflicts; devuelve la plist del body y cachea."
  (let* ((session (syncfiles-ensure-session)))
    (syncfiles--retry-once-on-401
     (lambda ()
       (let ((body (plist-get
                    (syncfiles--request "/conflicts"
                                        :method "GET"
                                        :token (plist-get session :session_id))
                    :body)))
         (setq syncfiles--conflicts-cache body)
         body)))))

(defun syncfiles--conflict-file-path (file-id)
  "Ruta relativa de FILE-ID según el listado cacheado, o nil."
  (plist-get (seq-find (lambda (f)
                         (equal (plist-get f :file_id) file-id))
                       (plist-get (or syncfiles--files-cache
                                      (ignore-errors (syncfiles--list-files)))
                                  :files))
             :relative_path))

(defun syncfiles--conflicts-entries ()
  "Entradas tabulated-list a partir de `syncfiles--conflicts-cache'."
  (mapcar (lambda (c)
            (let ((cid (or (plist-get c :conflict_id) "?"))
                  (file-id (plist-get c :file_id))
                  (created (plist-get c :created_at)))
              (list (propertize cid 'syncfiles-conflict c)
                    (vector (syncfiles--shorten cid)
                            (or (syncfiles--conflict-file-path file-id)
                                (syncfiles--shorten file-id 10))
                            (syncfiles--shorten (plist-get c :local_checksum))
                            (syncfiles--shorten (plist-get c :remote_checksum))
                            (if created
                                (format-time-string
                                 "%Y-%m-%d %H:%M" (floor created 1e3))
                              "?")
                            (or (plist-get c :conflict_type) "?")))))
          (plist-get syncfiles--conflicts-cache :conflicts)))

(define-derived-mode syncfiles-conflicts-mode tabulated-list-mode
  "SyncFiles-Conflicts"
  "Buffer de conflictos de SyncFiles."
  (setq tabulated-list-format [("Id" 10 t)
                               ("Archivo" 30 t)
                               ("Local" 10 nil)
                               ("Remoto" 10 nil)
                               ("Creado" 17 t)
                               ("Tipo" 18 t)]
        tabulated-list-sort-key '("Creado" . t))
  (tabulated-list-init-header)
  (local-set-key (kbd "l") #'syncfiles--conflict-keep-local)
  (local-set-key (kbd "r") #'syncfiles--conflict-keep-remote)
  (local-set-key (kbd "p") #'syncfiles--conflict-keep-local-preserve)
  (local-set-key (kbd "g") #'syncfiles-conflicts))

(defun syncfiles--conflicts-buffer ()
  "Devuelve (creando si hace falta) el buffer de conflictos."
  (or (get-buffer "*SyncFiles Conflicts*")
      (with-current-buffer (get-buffer-create "*SyncFiles Conflicts*")
        (syncfiles-conflicts-mode)
        (current-buffer))))

;;;###autoload
(defun syncfiles-conflicts ()
  "Lista los conflictos pendientes en *SyncFiles Conflicts*.
Claves: l = keep_local, r = keep_remote, p = keep_local preservando
la alternativa como copia .conflict_*, g = refrescar."
  (interactive)
  (syncfiles--list-conflicts)
  (with-current-buffer (syncfiles--conflicts-buffer)
    (setq tabulated-list-entries (syncfiles--conflicts-entries))
    (tabulated-list-print)
    (display-buffer (current-buffer))
    (message "SyncFiles: %s conflicto(s)"
             (or (plist-get syncfiles--conflicts-cache :total) 0))))

(defun syncfiles--conflict-at-point ()
  "Plist del conflicto en la línea del punto."
  (or (get-text-property (point) 'syncfiles-conflict)
      (save-excursion
        (beginning-of-line)
        (get-text-property (point) 'syncfiles-conflict))
      (user-error "No hay conflicto bajo el punto")))

(defun syncfiles--resolve-conflict (conflict-id decision &optional preserve)
  "POST /conflicts/resolve con CONFLICT-ID y DECISION.
PRESERVE activa preserve_alternative."
  (let* ((session (syncfiles-ensure-session)))
    (syncfiles--retry-once-on-401
     (lambda ()
       (syncfiles--request
        "/conflicts/resolve"
        :method "POST"
        :token (plist-get session :session_id)
        :body `((session_id . ,(plist-get session :session_id))
                (device_id . ,(plist-get session :device_id))
                (conflict_id . ,conflict-id)
                (decision . ,decision)
                (preserve_alternative . ,(if preserve :false t))))))))

(cl-defun syncfiles--conflict-resolve-at-point (decision preserve)
  "Resuelve el conflicto bajo el punto con DECISION y PRESERVE."
  (let ((c (syncfiles--conflict-at-point)))
    (let ((cid (plist-get c :conflict_id))
          (file-id (plist-get c :file_id)))
      (when (y-or-n-p
             (format "¿Resolver conflicto %s sobre %s con %s? "
                     (syncfiles--shorten cid)
                     (or (syncfiles--conflict-file-path file-id)
                         (syncfiles--shorten file-id 10))
                     (pcase decision
                       ("keep_local" (if preserve "keep_local + preservar alternativa"
                                       "keep_local"))
                       ("keep_remote" "keep_remote")
                       (_ decision))))
        (syncfiles--resolve-conflict cid decision preserve)
        (syncfiles--report "SyncFiles: conflicto %s resuelto (%s)"
                           (syncfiles--shorten cid) decision)
        ;; Refrescar el buffer de conflictos tras resolver.
        (syncfiles-conflicts)))))

(defun syncfiles--conflict-keep-local ()
  "Resuelve el conflicto bajo el punto con keep_local."
  (interactive)
  (syncfiles--conflict-resolve-at-point "keep_local" nil))

(defun syncfiles--conflict-keep-remote ()
  "Resuelve el conflicto bajo el punto con keep_remote."
  (interactive)
  (syncfiles--conflict-resolve-at-point "keep_remote" nil))

(defun syncfiles--conflict-keep-local-preserve ()
  "Resuelve con keep_local preservando la alternativa (copia .conflict_*)."
  (interactive)
  (syncfiles--conflict-resolve-at-point "keep_local" t))

(provide 'syncfiles)
;;; syncfiles.el ends here
