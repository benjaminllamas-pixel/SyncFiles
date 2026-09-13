# Distribución de Clientes

## 1. Objetivo

La distribución de clientes define cómo SyncFiles llega a cada plataforma soportada y cómo se mantiene la versión instalada sincronizada con el ciclo de despliegue y de seguridad.

En V1, la distribución debe ser simple, verificable y consistente:
- Windows, macOS, Linux y Android,
- instalación con firma digital,
- actualización controlada,
- rollback posible si aparece un problema significativo.

## 2. Canales de distribución

### 2.1 Windows

- instalador `.msi` o `.exe` firmado por certificado válido,
- distribución por canal interno o por paquete de empresa,
- validación de firma digital antes de aceptar actualizaciones,
- soporte para instalación silenciosa en entornos administrados.

### 2.2 macOS

- `.dmg` o paquete `.app` firmado,
- soporte para entornos de desktop con validación de identidad,
- actualizaciones por auto-update con verificación de firma,
- compatibilidad con el sistema operativo soportado por la versión.

### 2.3 Linux

- paquetes `.deb` / `.rpm` o tarball portable,
- versiones por distribución y perfil de soporte,
- instalación con permisos del usuario o del sistema según contexto.

### 2.4 Android

- distribución por Google Play o vía internal testing,
- validación de versión y compatibilidad con API mínima,
- actualización por store o canal interno según política de despliegue.

### 2.5 Emacs

- no hay binario que distribuir: el cliente es un único paquete Elisp
  (`syncfiles-emacs/syncfiles.el`) que se carga con `add-to-list` +
  `require` desde Emacs 29+,
- sin dependencias externas (solo `url-retrieve` y `json` incluidos en
  Emacs); instalación documentada en `syncfiles-emacs/README.md`,
- sesión persistente en `~/.config/syncfiles/session.json` (0600) y
  credenciales vía auth-source o prompt único,
- limitación V1: es un cliente de operaciones ad-hoc (subir, descargar,
  listar, borrar, resolver conflictos), no un motor de sincronización de
  carpetas completas ni reemplazo del cliente desktop.

## 3. Requisitos mínimos

- firma digital obligatoria para instalar o actualizar,
- comprobación de hash o versión antes de aplicar actualización,
- registro de versión instalada por dispositivo,
- rollback documentado en caso de regresión,
- mantenimiento de compatibilidad con la API del backend.

## 4. Política de actualización

- no se actualiza automáticamente sin validación de firma,
- la versión cliente debe comprobar compatibilidad del backend antes de activar nuevas capacidades,
- si el backend no soporta una funcionalidad, el cliente debe degradar al comportamiento compatible,
- los cambios mayores deben aplicarse por etapas y con observabilidad.

## 5. Distribución y soporte

La estrategia recomendada es:
- versión estable + canal beta para pruebas internas,
- QA y validación antes de release general,
- seguimiento por dispositivo y versión,
- alertas cuando una versión queda por debajo del nivel de soporte mínimo.

## 6. Seguridad de distribución

- certificados y firmas verificadas en todos los canales,
- validación de origen del binario,
- bloqueo de instalación desde fuentes no autorizadas,
- no aceptar “actualización” sin integridad y autenticación.

## 7. Criterios de aceptación

La distribución del cliente es adecuada si:
- cada cliente se instala y actualiza de forma segura,
- la versión de cliente y backend son compatibles,
- las actualizaciones pueden revertirse si aparecen regresiones,
- la arquitectura deja paso a nuevas plataformas sin reescribir el cliente base.

## 8. Cliente Emacs: instalación y uso

Instalación (desde el raíz del repo, con el servidor corriendo):

```elisp
(add-to-list 'load-path "syncfiles-emacs")
(require 'syncfiles)
(setq syncfiles-server-url "http://127.0.0.1:8080")
```

Comandos principales:

| Comando | Acción |
|---|---|
| `M-x syncfiles-login` / `M-x syncfiles-logout` | gestión de sesión (persistente, auto-revalidada) |
| `M-x syncfiles-upload` | subir archivo del buffer / dired; `C-u` para archivo arbitrario |
| `M-x syncfiles-upload-region` | subir la región marcada |
| `M-x syncfiles-list-files` | listado tabulado (`RET` descarga, `d` borra, `g` refresca) |
| `M-x syncfiles-download` | descargar con verificación de checksum |
| `M-x syncfiles-delete` | borrar remoto |
| `M-x syncfiles-conflicts` | resolver conflictos (`l` local, `r` remoto, `p` local preservando) |

Limitaciones documentadas: operaciones síncronas (bloquean Emacs durante la
transferencia), sin motor de sincronización de carpetas, sin rename/move/copy
remotos en V1. Validación: `syncfiles-emacs/test-e2e.el` (E2E batch contra un
servidor real, mismo esquema que `scripts/e2e-test.sh`).
