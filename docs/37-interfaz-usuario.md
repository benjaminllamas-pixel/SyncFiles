# Interfaz de Usuario

## Diseño Visual

Las tres UIs comparten los mismos conceptos: navegación por vistas (Dashboard,
Archivos, Cola, Conflictos, Dispositivos, Actividad, Ajustes), estado de
conexión siempre visible y toasts/notificaciones para el resultado de las
operaciones. Sin build step ni frameworks JS: el web dashboard es vanilla
HTML/CSS/JS.

---

## Estado tras la Fase 5 (2026-09-10)

Todas las pantallas listadas abajo están implementadas y verificadas con la
batería E2E real (`scripts/e2e-phase5.sh`, 17 PASS / 0 FAIL; detalles en
`docs/41-estrategia-pruebas.md` §9).

### Desktop (egui) — `syncfiles-client`

| Pantalla | Contenido | Verificación E2E |
|---|---|---|
| Login | email/contraseña/URL servidor, error y estado "conectando" | login headless + ciclo OK |
| Dashboard | estado conexión, badge "Pausado", resumen archivos/tamaño (`storage/stats`), última sincronización (`engine_status.json`) | `storage/stats` con datos reales |
| Archivos | tabla con estado por archivo, menú contextual (renombrar/mover/copiar/borrar), modales de confirmación | subida/bajada/borrado vía motor |
| Cola | operaciones pendientes/retry con intentos y último error | cola persistente probada en 5.4 |
| Conflictos | ruta del archivo (no file_id), resolver mantener local/remoto con copia alternativa | 5.3 end-to-end |
| Dispositivos | dispositivos de la cuenta (`/devices`) | multi-dispositivo 5.2 |
| Actividad | log local de eventos | `activity` con eventos reales |
| Ajustes | URL servidor, carpeta (selector `rfd`), intervalo, pausa, logout | cambio de carpeta recrea watcher |

### Android — `syncfiles-android`

Rediseño **"Midnight"** (2026-09-10): paleta oscura elegante (fondo casi
negro azulado `#0B0E13`, superficies `#131820`, acento índigo→cian) con
dynamic color (Material You) en Android 12+, edge-to-edge con status bar
oscura (sin flash blanco al abrir), cards con borde 1dp alpha 10% y esquinas
20dp, icono por tipo de archivo, tamaños humanos ("1.2 MB"), badges de estado
con punto de color, FAB "Subir" y login con logo en círculo de gradiente.

| Pantalla | Contenido |
|---|---|
| Login | email/contraseña/URL servidor, campos tonal, botón primario 52dp con gradiente |
| Home | hero card de estado con gradiente + icono de sync, sesión compacta (email con avatar de iniciales), subida (FAB), "Sincronizar ahora" |
| Archivos | lista (`/files/list`) con icono por tipo de archivo y tamaño humano, descarga a carpeta SAF, badges de estado |
| Conflictos | lista (`/conflicts`), resolver mantener local/remoto (guarda copia `.conflict_*`) |
| Ajustes | URL servidor (validada), carpeta SAF persistente, intervalo (15 min–6 h), logout |
| Sincronización de fondo | WorkManager periódico + expedido, FileObserver de la carpeta, cola `SyncQueueStore` |

Componentes compartidos del tema en `ui/theme/Components.kt`
(`SfCard`, `SfStatusBadge`, `formatBytes`, iconos por tipo de archivo).

### Web dashboard — `syncfiles-server/static/` (servido en `/`)

| Pantalla | Contenido |
|---|---|
| Login | contra `/auth/login`, sesión en localStorage (Bearer), restauración automática |
| Dashboard | tarjetas Servidor/Almacenamiento/Última modificación, tabla dispositivos con botón "Revocar" (revoca sesiones activas vía `POST /devices/revoke`), auto-refresh 15 s |
| Archivos | tabla (`/files/list`), dropzone de subida con drag&drop (`/sync/upload`, SHA-256 + base64), descarga (base64→Blob), renombrar/mover/copiar con modal (`/sync/rename`, `/sync/move`, `/sync/copy`), borrado con confirmación, badge "Desactualizado" vía `/sync/diff`, "Mostrar borrados" |
| Cola | operaciones de sincronización (`GET /queue`) con operación/dispositivo/estado/intentos/error, filtro "Solo pendientes" |
| Conflictos | tarjetas con checksums/dispositivos/fecha, mantener local/remoto + copia alternativa, badge contador |
| Actividad | últimos 100 eventos (`/activity`) |

Responsive móvil (< 720px nav deslizable, acciones en segunda línea; < 460px tarjetas apiladas) y modo oscuro.

### Cómo lanzar cada cliente

```bash
# Servidor (necesario para los tres)
(cd syncfiles-server && cargo run)

# Desktop (GUI)
(cd syncfiles-client && cargo run)

# Web: abrir http://127.0.0.1:8080/ (dashboard servido por el propio servidor)

# Android
(cd syncfiles-android && ./gradlew installDebug)
```

Credenciales por defecto: `admin@syncfiles.local` / `syncfiles`.

### Pendiente (fases posteriores)

- Encriptación E2E, versionado de archivos, compartir, NAS avanzado (V2).
- Notificaciones push/system en Android para conflictos.
- Vista previa de archivos (miniaturas) en web/desktop.
