;;; test-e2e.el --- E2E batch test for syncfiles.el -*- lexical-binding: t; -*-

;; Ejecutar contra un servidor real:
;;
;;   1. Levanta el server como scripts/e2e-test.sh (binario debug),
;;      con SF_BIND_ADDRESS=127.0.0.1:8081, SF_USER_0_EMAIL/PASSWORD/ID.
;;   2. emacs --batch -Q -L syncfiles-emacs -l syncfiles.el -l test-e2e.el
;;
;; Variables de entorno:
;;   SF_SERVER_URL   (default http://127.0.0.1:8081)
;;   SF_EMAIL        (default admin@syncfiles.local)
;;   SF_PASSWORD     (default syncfiles)
;;
;; Sale con código 0 si todos los pasos son PASS; 1 si alguno falla.

;;; Code:

(require 'syncfiles)

(defvar sf-e2e--failures 0)
(defvar sf-e2e--workdir
  (let ((dir (expand-file-name
              "emacs-e2e"
              (or (getenv "SF_E2E_TMPDIR") "/tmp/syncfiles-e2e"))))
    (make-directory dir t)
    dir))

(defun sf-e2e--env (name default)
  (or (getenv name) default))

(setq syncfiles-server-url (sf-e2e--env "SF_SERVER_URL" "http://127.0.0.1:8081")
      syncfiles-device-id  "emacs-e2e"
      ;; session.json de la prueba aislado en el workdir, no en ~/.config.
      syncfiles-config-dir (concat sf-e2e--workdir "/config"))

;; Credenciales: sin auth-source, sin prompts.  Inyectamos la cache.
(setq syncfiles--credentials
      (list :email (sf-e2e--env "SF_EMAIL" "admin@syncfiles.local")
            :password (sf-e2e--env "SF_PASSWORD" "syncfiles")))

(defmacro sf-e2e--step (name &rest body)
  "Ejecuta un paso NAME con BODY; imprime PASS/FAIL y acumula fallos."
  (declare (indent 1))
  `(let ((ok nil) (detail ""))
     (condition-case err
         (progn ,@body (setq ok t))
       (error (setq detail (format "%S" err))))
     (if ok
         (message "PASS  %s" ,name)
       (setq sf-e2e--failures (1+ sf-e2e--failures))
       (message "FAIL  %s  -- %s" ,name detail))
     ok))

(defun sf-e2e--login-raw (email password device-id)
  "Login directo sin capa de sesión (para la segunda sesión del test)."
  (plist-get (syncfiles--request
              "/auth/login"
              :method "POST"
              :body `((email . ,email)
                      (password . ,password)
                      (device_id . ,device-id)))
             :body))

(defun sf-e2e--upload-raw (session relative-path content)
  "Upload directo con SESSION ya existente (evita re-login/ensure).
Un 409 (conflicto) no se señala: se devuelve la respuesta completa."
  (syncfiles--request
   "/sync/upload"
   :method "POST"
   :token (plist-get session :session_id)
   :noerror t
   :body `((session_id . ,(plist-get session :session_id))
           (device_id . ,(plist-get session :device_id))
           (file_id . "")
           (relative_path . ,relative-path)
           (path_hash . ,(syncfiles--path-hash relative-path))
           (checksum . ,(secure-hash 'sha256 content))
           (size_bytes . ,(string-bytes content))
           (modified_at . ,(syncfiles--now-ms))
           (idempotency_key . ,(syncfiles--uuid))
           (content . ,(base64-encode-string content t)))))

(defun sf-e2e--write-file (path content)
  "Escribe CONTENT (unibyte) a PATH binario-safe; devuelve PATH."
  (let ((coding-system-for-write 'no-conversion))
    (write-region content nil path nil 'quiet))
  path)

(defun sf-e2e--read-file (path)
  "Lee PATH como string unibyte."
  (with-temp-buffer
    (set-buffer-multibyte nil)
    (insert-file-contents-literally path)
    (buffer-string)))

(defun sf-e2e--download-to (file-id path-hash dest)
  "Descarga FILE-ID con PATH-HASH a DEST usando `syncfiles-download'."
  (cl-letf (((symbol-function 'read-file-name)
             (lambda (&rest _) dest)))
    (syncfiles-download file-id path-hash)))

;;; Pasos E2E

(message "=== SyncFiles Emacs E2E contra %s ===" syncfiles-server-url)

;; 1. Login (auto-login + session.json persistente)
(sf-e2e--step "login+persist"
  (let ((session (syncfiles-ensure-session)))
    (unless (and (stringp (plist-get session :session_id))
                 (> (length (plist-get session :session_id)) 0))
      (error "sin session_id"))
    (unless (file-exists-p (syncfiles--config-file))
      (error "session.json no se persistió"))
    ;; La segunda llamada usa la cache/validación y no debe fallar.
    (syncfiles-ensure-session)))

;; 2. Upload texto (con acentos/ñ en la ruta)
(let* ((text (encode-coding-string "Hola E2E con acentos: áéíóú ñ" 'utf-8))
       (rel-path "emacs-e2e/tortita ñoña.txt")
       (local (sf-e2e--write-file
               (concat sf-e2e--workdir "/texto.txt") text)))
  (sf-e2e--step "upload-texto-utf8"
    (let ((resp (syncfiles--upload-file local rel-path)))
      (unless (equal (plist-get (plist-get resp :body) :status) "ok")
        (error "status != ok: %S" resp)))))

;; 3. Upload binario aleatorio 4KB
(let* ((bin (apply #'unibyte-string
                   (mapcar (lambda (_) (random 256)) (make-list 4096 nil))))
       (rel-path "emacs-e2e/binary.bin")
       (local (sf-e2e--write-file
               (concat sf-e2e--workdir "/binary.bin") bin)))
  (sf-e2e--step "upload-binario-4k"
    (let ((resp (syncfiles--upload-file local rel-path)))
      (unless (equal (plist-get (plist-get resp :body) :status) "ok")
        (error "status != ok: %S" resp)))))

;; 4. files/list contiene ambos
(let* ((list-body (syncfiles--list-files))
       (paths (mapcar (lambda (f) (plist-get f :relative_path))
                      (plist-get list-body :files))))
  (sf-e2e--step "list-contiene-uploads"
    (unless (member "emacs-e2e/tortita ñoña.txt" paths)
      (error "no está el archivo de texto: %S" paths))
    (unless (member "emacs-e2e/binary.bin" paths)
      (error "no está el binario: %S" paths))))

;; 5. Download round-trip byte-idéntico (texto y binario)
(let* ((list-body (syncfiles--list-files))
       (files (plist-get list-body :files))
       (text-entry (seq-find (lambda (f)
                               (equal (plist-get f :relative_path)
                                      "emacs-e2e/tortita ñoña.txt"))
                             files))
       (bin-entry (seq-find (lambda (f)
                              (equal (plist-get f :relative_path)
                                     "emacs-e2e/binary.bin"))
                            files)))
  (sf-e2e--step "download-roundtrip-texto"
    (sf-e2e--download-to (plist-get text-entry :file_id)
                         (plist-get text-entry :path_hash)
                         (concat sf-e2e--workdir "/dl-texto.txt"))
    (unless (string-equal
             (sf-e2e--read-file (concat sf-e2e--workdir "/texto.txt"))
             (sf-e2e--read-file (concat sf-e2e--workdir "/dl-texto.txt")))
      (error "contenido texto difiere")))
  (sf-e2e--step "download-roundtrip-binario"
    (sf-e2e--download-to (plist-get bin-entry :file_id)
                         (plist-get bin-entry :path_hash)
                         (concat sf-e2e--workdir "/dl-binary.bin"))
    (unless (string-equal
             (sf-e2e--read-file (concat sf-e2e--workdir "/binary.bin"))
             (sf-e2e--read-file (concat sf-e2e--workdir "/dl-binary.bin")))
      (error "contenido binario difiere"))))

;; 6. Conflicto: segunda sesión sube contenido distinto a la misma ruta
(let* ((text2 (encode-coding-string "Contenido MODIFICADO para conflicto" 'utf-8))
       (session2 (sf-e2e--login-raw
                  (plist-get syncfiles--credentials :email)
                  (plist-get syncfiles--credentials :password)
                  "emacs-e2e-segunda")))
  (sf-e2e--step "upload-conflicto-409"
    (let* ((resp (sf-e2e--upload-raw session2 "emacs-e2e/tortita ñoña.txt" text2))
           (body (plist-get resp :body)))
      (unless (= (plist-get resp :status) 409)
        (error "esperaba HTTP 409, obtuve: %S" resp))
      (unless (equal (plist-get body :status) "conflict")
        (error "esperaba status=conflict, obtuve: %S" body))
      (unless (plist-get (plist-get body :data) :conflict_id)
        (error "sin conflict_id: %S" body)))))

;; 7. GET /conflicts lista >= 1
(let ((conflicts-body (syncfiles--list-conflicts)))
  (sf-e2e--step "conflicts-listados"
    (unless (>= (or (plist-get conflicts-body :total) 0) 1)
      (error "total < 1: %S" conflicts-body))))

;; 8. Resolver conflicto keep_local y verificar que desaparece
(let* ((conflicts-body (syncfiles--list-conflicts))
       (conflict (car (plist-get conflicts-body :conflicts))))
  (sf-e2e--step "resolve-keep-local"
    (let ((resp (syncfiles--resolve-conflict
                 (plist-get conflict :conflict_id) "keep_local" nil)))
      (unless (plist-get (plist-get resp :body) :accepted)
        (error "no aceptado: %S" resp))))
  (sf-e2e--step "conflicto-desaparece"
    (let ((after (syncfiles--list-conflicts)))
      (when (seq-find (lambda (c)
                        (equal (plist-get c :conflict_id)
                               (plist-get conflict :conflict_id)))
                      (plist-get after :conflicts))
        (error "el conflicto sigue listado")))))

;; 9. Delete del binario y verificar que desaparece de files/list
(let* ((list-body (syncfiles--list-files))
       (bin-entry (seq-find (lambda (f)
                              (equal (plist-get f :relative_path)
                                     "emacs-e2e/binary.bin"))
                            (plist-get list-body :files))))
  (sf-e2e--step "delete-binario"
    (syncfiles-delete (plist-get bin-entry :file_id)
                      (plist-get bin-entry :path_hash)
                      "emacs-e2e/binary.bin"))
  (sf-e2e--step "list-sin-borrado"
    (setq syncfiles--files-cache nil)
    (let* ((fresh (syncfiles--list-files))
           (paths (mapcar (lambda (f) (plist-get f :relative_path))
                          (plist-get fresh :files))))
      (when (member "emacs-e2e/binary.bin" paths)
        (error "el binario sigue en el listado")))))

;; 10. Logout: endpoint OK y session.json borrado
(sf-e2e--step "logout"
  (syncfiles-logout)
  (when (file-exists-p (syncfiles--config-file))
    (error "session.json no se borró tras logout")))

;; Resumen
(message "=== RESULTADO: %s (%d fallos) ==="
         (if (zerop sf-e2e--failures) "PASS" "FAIL")
         sf-e2e--failures)
(kill-emacs (if (zerop sf-e2e--failures) 0 1))

;;; test-e2e.el ends here
