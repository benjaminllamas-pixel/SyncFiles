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

| Pantalla | Contenido |
|---|---|
| Login | email/contraseña/URL servidor |
| Home | estado del motor (en progreso/al día/error con timestamp), subida, "Sincronizar ahora" |
| Archivos | lista (`/files/list`), descarga a carpeta SAF |
| Conflictos | lista (`/conflicts`), resolver mantener local/remoto (guarda copia `.conflict_*`) |
| Ajustes | URL servidor (validada), carpeta SAF persistente, intervalo (15 min–6 h), logout |
| Sincronización de fondo | WorkManager periódico + expedido, FileObserver de la carpeta, cola `SyncQueueStore` |

### Web dashboard — `syncfiles-server/static/` (servido en `/`)

| Pantalla | Contenido |
|---|---|
| Login | contra `/auth/login`, sesión en localStorage (Bearer), restauración automática |
| Dashboard | tarjetas Servidor/Almacenamiento/Última modificación, tabla dispositivos, auto-refresh 15 s |
| Archivos | tabla (`/files/list`), descarga (base64→Blob), borrado con confirmación, "Mostrar borrados" |
| Conflictos | tarjetas con checksums/dispositivos/fecha, mantener local/remoto + copia alternativa, badge contador |
| Actividad | últimos 100 eventos (`/activity`) |

Responsive móvil (< 720px nav deslizable; < 460px tarjetas apiladas) y modo oscuro.

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
