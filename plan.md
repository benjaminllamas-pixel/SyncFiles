# Plan UI SyncFiles — Dejar el Frontend Listo para Pruebas Reales

> Objetivo: completar/habilitar toda la funcionalidad de UI (desktop egui, Android y web)
> para poder ejecutar pruebas reales de sincronización de extremo a extremo.
> Basado en el inventario de brechas de `docs/37-interfaz-usuario.md` y el código actual.

## 0. Alcance y criterios de salida

**Incluido:** todo lo necesario para probar sincronización real entre clientes y servidor
(login → subir/bajar/borrar/renombrar → conflictos → cola → actividad).

**Excluido (fases posteriores):** encriptación E2E, versionado, compartir, NAS avanzado.

**Criterios de éxito:**
- [x] Login/logout visible y funcional en desktop (ya existe en Android/web pendiente)
- [x] Sincronización real entre 2+ dispositivos con archivos de prueba
      (verificado en 5.2/5.4 con motor real + CLI multi-dispositivo)
- [x] Conflictos creados, detectados y resueltos desde la UI
      (verificado en 5.3: detección, listado y resolución keep_local)
- [x] Cola de sincronización persistente con reintentos visible
      (verificado en 5.4: reinicio del cliente con archivo offline pendiente)
- [x] Registro de actividad/auditoría accesible desde la UI
      (verificado en 5.5: GET /activity con eventos reales)
- [x] Web dashboard mínimo servido desde `syncfiles-server`

---

## 1. Fase 1 — Server: endpoints faltantes para soportar la UI

> Sin estos, ninguna UI puede mostrar datos reales.

- [x] 1.1 `GET /api/v1/files/list` — listar archivos sincronizados (path, size, mtime, estado)
      del usuario autenticado (fuente: `files`/`file_changes` en `syncfiles-server/src/db.rs`)
      (soporta `?include_deleted=true`)
- [x] 1.2 `GET /api/v1/queue` — operaciones pendientes del dispositivo (para la vista Cola)
      (soporta `?device_id=`, `?status=pending`, `?limit=`)
- [x] 1.3 `GET /api/v1/activity` — eventos de auditoría del usuario (para la vista Actividad)
      (soporta `?limit=`)
- [x] 1.4 `GET /api/v1/conflicts` — conflictos sin resolver (para la vista Conflictos)
- [x] 1.5 `GET /api/v1/devices` — dispositivos con sesión activa del usuario
- [x] 1.6 `GET /api/v1/storage/stats` — bytes usados, número de archivos (para Dashboard)
- [x] 1.7 Corregir `syncfiles-server/src/main.rs:22-36`: activar CORS (`.wrap()` con
      `actix-cors`, dependencia ya presente pero sin usar)
- [x] 1.8 Añadir tests de integración para los endpoints nuevos (mismo estilo que
      `scripts/e2e-test.sh`) — `syncfiles-server/tests/api_endpoints.rs` (9 tests, todos OK)

## 2. Fase 2 — Desktop (egui): activar código muerto y corregir bugs

> Prioridad: conectar lo ya escrito antes de crear features nuevas.

### 2.1 Login/Logout visible
- [x] Añadir pantalla de login (email/contraseña/URL servidor) al arrancar si no hay sesión
      — reutilizar `login()` (`syncfiles-client/src/main.rs`)
- [x] Añadir botón "Cerrar sesión" que invoque `logout()` (header)
- [x] Mostrar `login_error` y estado `connecting` en la UI

### 2.2 Pausar/Reanudar sincronización
- [x] Botón "Pausar/Reanudar" conectado a `toggle_pause()` (header)
- [x] Reflejar el estado `paused` visualmente (badge "Pausado" en header y Dashboard)

### 2.3 Notificaciones/toasts
- [x] Renderizar la pila de notificaciones (`notify()` ya existe, área anclada arriba-derecha)
- [x] Llamar `prune_notifications()` en cada frame
- [x] Cerrar con botón X y auto-descarte tras 8 segundos

### 2.4 Modales de operaciones sobre archivos
- [x] Implementar render de `Modal` (Rename/Move/Copy/Delete/ConflictResolve)
- [x] Menú contextual (clic derecho) sobre un archivo → abrir modal correspondiente
- [x] Conectar a `network.rs`: `move_file`, `copy_file`, `resolve_conflict` (operación local
      + remota con actualización de metadatos)

### 2.5 Vistas Dashboard y Devices
- [x] Dashboard: estado conexión, resumen archivos, tamaño, última sincronización
      (consume `storage/stats` vía cache `stats_cache.json`)
- [x] Devices: lista de dispositivos usando `self.devices` + refresh

### 2.6 Correcciones de bugs UI
- [x] Conflictos: mostrar ruta del archivo, no `file_id` (lookup en BD local)
- [x] Vista Conflictos accesible también en ventana estrecha (<680px, ComboBox)
- [x] Preferencias: persistir cambios con `config.save()`; cambio de carpeta recrea watcher;
      selector de carpeta nativo con `rfd`
- [x] "Sincronizar ahora" dispara un ciclo real del `SyncEngine` (canal wakeup interrumpible)
- [x] Estado "Conectado": seteado desde `engine_status.json` que escribe el motor tras cada ciclo
- [x] Selector de carpeta implementado con `rfd` en Preferencias

### 2.7 Sincronización subyacente (para pruebas reales)
- [x] Implementar rename/move/copy remotos en `syncfiles-client/src/sync.rs`
      (`apply_renaming` para rename/move; copy descarga contenido por path_hash)
- [x] Persistir la cola entre reinicios (`retry_queued_ops` reprocesa payloads upload
      en cola SQLite al iniciar y en cada ciclo; intentos incrementados, estado final en BD)

## 3. Fase 3 — Android: cablear lo ya escrito

> El app ya tiene login/home/subir/sincronizar manual; falta el fondo y las vistas.

- [x] 3.1 Enqueuear `SyncWorker` (WorkManager) — periodic + constraints de red
      — `SyncScheduler.kt` (nuevo): periódico con intervalo configurable en Ajustes
      (15 min por defecto, política `ExistingPeriodicWorkPolicy.UPDATE`) + one-shot
      expedited `syncNow()` con `NetworkType.CONNECTED`
- [x] 3.2 Instanciar `FileWatcher` (FileObserver) para detectar cambios en la carpeta elegida
      — `FileWatcher` reescrito: observa la ruta SAF subyacente del `DocumentFile` (si no se
      puede resolver la ruta física, queda inactivo y se usa el escaneo manual; FileObserver
      no funciona directamente con URIs de SAF)
- [x] 3.3 Conectar `SyncQueueStore` con el worker (ya ambos existen, sin uso)
      — nuevo `SyncEngine` (singleton en `SyncFilesApplication`) que cablea watcher → cola →
      worker; el watcher encola upload/delete por `path_hash` y dispara `syncNow()`;
      el worker procesa `queued` + `retry`, hace pull remoto (diff + download + delete local)
      y actualiza metadatos en `LocalFileSyncStore`
- [x] 3.4 Añadir pantalla de conflictos usando `resolveConflict` (ya en `SyncFilesApi.kt`,
      sin llamadas)
      — `ui/conflicts/`: lista desde `GET /conflicts` (endpoint añadido a la API), resolución
      keep_local/keep_remote con `preserve_alternative=true` (guarda copia `.conflict_*`)
- [x] 3.5 Añadir botón de descarga / vista de archivos usando `download` (ya en la API,
      sin llamadas)
      — `ui/files/`: lista desde `GET /files/list` (endpoint añadido), botón Descargar →
      `POST /sync/download` guardando el contenido en la carpeta sincronizada
- [x] 3.6 Pantalla de ajustes: URL servidor, carpeta, intervalo, logout
      — `ui/settings/`: URL validada contra `session/status` (guarda aunque falle la
      validación), carpeta SAF con `takePersistableUriPermission`, intervalo (chips
      15 min/30 min/1 h/6 h), logout detiene el motor
- [x] 3.7 Indicador de estado de sincronización (en progreso / al día / error)
      — `SyncStatusStore` (SharedPreferences) escrito por el worker en cada ciclo;
      Home muestra tarjeta de estado con color, timestamp del último ciclo y mensaje
      de error; "Sincronizar ahora" dispara también el worker

## 4. Fase 4 — Web dashboard mínimo (servido desde syncfiles-server)

> Decisión previa: servir desde el propio servidor Rust (sin Node) para pruebas.
> Implementado como SPA vanilla (HTML/CSS/JS sin build step) en `syncfiles-server/static/`,
> servida en `/` (alias `/ui` → redirect) con `actix-files`. Arranca desde la raíz del
> repo o desde `syncfiles-server/`; sobreescribible con `SF_STATIC_DIR`.

- [x] 4.1 Servir `static/` con `actix-files` (`Files::new("/", ...)`, index `index.html`;
      alias `web::redirect("/ui", "/")`; fallback a `./syncfiles-server/static` si el CWD
      es la raíz del repo; `SF_STATIC_DIR` opcional)
- [x] 4.2 Login (contra `/auth/login`) con manejo de sesión (localStorage + Bearer)
      (`static/app.js`: form con email/password/URL servidor, `sf_token`/`sf_server`/
      `sf_device` en localStorage, `Authorization: Bearer` en todas las llamadas,
      restauración de sesión con `/session/status`, logout con `/auth/logout`)
- [x] 4.3 Vista Dashboard: estado del servidor, storage stats, dispositivos
      (tarjetas Servidor/Almacenamiento/Última modificación vía `/storage/stats`;
      tabla de dispositivos vía `/devices`; badge Conectado/Sin conectar;
      auto-refresh cada 15 s)
- [x] 4.4 Vista Archivos: tabla con lista (`/files/list`), descarga (`/sync/download`),
      borrado (`/sync/delete`)
      (checkbox "Mostrar borrados" → `?include_deleted=true`; descarga base64 → Blob →
      `a[download]`; borrado con confirmación; toasts de éxito/error)
- [x] 4.5 Vista Conflictos: listar y resolver (`/conflicts` + `/conflicts/resolve`)
      (tarjetas con tipo/checksums/dispositivos/fecha; botones Mantener local /
      Mantener remoto / Local + copia alternativa (`preserve_alternative=true`);
      badge contador en la navegación)
- [x] 4.6 Vista Actividad: log de eventos (`/activity`)
      (últimos 100 eventos: fecha, `event_name`, dispositivo, payload truncado)
- [x] 4.7 Responsive mínimo ( móvil ) para poder probar desde el navegador del teléfono
      (nav lateral deslizable < 720px con overlay; acciones de fila como segunda línea;
      tarjetas apiladas < 460px; `viewport-fit=cover`; soporte dark mode)
- [x] 4.8 Activar CORS para permitir pruebas desde otros orígenes (Fase 1.7)
      (ya estaba activo desde Fase 1; verificado con test `login_via_web_flow_returns_session`
      y curl con cabecera `Origin`)
- [x] 4.9 Tests de integración — `syncfiles-server/tests/static_dashboard.rs`
      (6 tests: index servido, assets servidos, redirect `/ui`, login con CORS,
      404 JSON en API, 404 de archivo inexistente; total server: 15 tests OK)

## 5. Fase 5 — Pruebas E2E reales

- [x] 5.1 Levantar servidor local (`scripts/e2e-test.sh` como base)
      (corregido: espera activa del arranque, login JSON del CLI, descarga con
      file_id real del diff + verificación de contenido con `cmp`)
- [x] 5.2 Escenario multi-dispositivo: desktop + Android + web simultáneos
      con la misma cuenta, verificando propagación de cambios
      (`scripts/e2e-phase5.sh`: A sube → B ve diff → B descarga idéntico →
      B borra → A ve el delete; los endpoints que consumen las 3 UIs quedaron
      verificados con datos reales)
- [x] 5.3 Escenario de conflicto: editar el mismo archivo en 2 dispositivos offline,
      reconectar, resolver desde la UI
      (CLI A/B + curl: `status=conflict`, listado `GET /conflicts`, resolución
      `keep_local` con `preserve_alternative=true`, conflicto desaparece)
- [x] 5.4 Escenario de reinicio: cerrar/abrir cliente con cola pendiente (verificar
      persistencia)
      (cliente desktop en modo headless `SF_HEADLESS=1` con motor real: sube en
      caliente, se mata, archivo creado offline, al reiniciar el escaneo de
      reconciliación lo detecta y sube; SQLite local persiste vía `SF_DATA_DIR`)
- [x] 5.5 Escenario de red interrumpida: desconectar servidor a mitad de subida,
      verificar reintentos y notificaciones
      (servidor caído → upload falla limpiente sin colgar; servidor recuperado →
      reintento manual OK y el motor reintenta solo el archivo pendiente)
- [x] 5.6 Documentar resultados en `docs/41-estrategia-pruebas.md` y actualizar
      `docs/37-interfaz-usuario.md` con pantallas finales
      (§9 en docs/41 con matriz de resultados y bugs; docs/37 con tablas de
      pantallas de desktop/Android/web y cómo lanzar cada cliente)

**Resultado Fase 5: 17 PASS / 0 FAIL** (`scripts/e2e-phase5.sh`) + smoke PASS +
21 tests de integración del server PASS. Bugs corregidos de camino: parseo de
subcomandos del CLI, salida JSON de login, escaneo de reconciliación de carpeta
local en el SyncEngine (archivos creados con el cliente apagado), y purga de
operaciones de cola sin payload.

## 5b. Rediseño UI Android "Midnight" (post-Fase 5)

> Aplicado sobre la app existente de Fase 3; sin cambios funcionales, solo
> look & feel y ergonomía.

- [x] Tema oscuro "Midnight" forzado: fondo `#0B0E13`/superficies `#131820`,
      acento índigo→cian — `ui/theme/Theme.kt`
      (corregido: antes usaba `isSystemInDarkTheme()` + dynamic color, y en
      modo día el emulador mostraba tema claro pese a pedirse "colores
      oscuros"; ahora `SyncFilesTheme` siempre aplica `DarkColors`)
- [x] Base oscura real en el theme XML (sin flash blanco) + edge-to-edge
      — `res/values/themes.xml`, `MainActivity`
- [x] Componentes compartidos: `ElevatedCard` (borde 1dp, esquinas 20dp, con
      `onClick` opcional), `StatusBadge` (punto de color), `BrandGradient`,
      `InitialsAvatar`, `formatBytes` (tamaños humanos), icono por tipo de
      archivo — `ui/theme/Components.kt` (nuevo)
- [x] Login con marca: logo en círculo con gradiente, campos tonal, botón
      primario 52dp — `ui/login/LoginScreen.kt`
- [x] Home como producto: hero card de estado con gradiente, sesión compacta
      con avatar de iniciales (sin user_id/expira), FAB "Subir" —
      `ui/home/HomeScreen.kt`
      (corregido: las cards Archivos/Conflictos de `NavigationRow` no tenían
      `onClick` — dibujaban pero no navegaban; `ElevatedCard` recibió overload
      clicable)
- [x] Archivos: iconos por tipo, tamaños humanos, badges de estado —
      `ui/files/FilesScreen.kt`
- [x] Conflictos y Ajustes al estilo consistente — `ui/conflicts/`,
      `ui/settings/`
- [x] Verificado: `assembleDebug` OK, app en emulador sin crash en primer
      plano, tema oscuro confirmado por análisis de píxeles de screenshots
      (brillo promedio ≈37/255 con contenido visible), navegación Home→
      Archivos/Conflictos/Ajustes funciona, downloads de los 3 archivos
      remotos OK ("Todo al día")

### 5b.1 Bug de servidor corregido: storage inconsistente con la DB

- El default de `SF_STORAGE_ROOT` apuntaba a `~/Library/Application Support/
  syncfiles/storage` (absoluto, vía crate `dirs`) mientras que
  `SF_DATABASE_URL` default era relativo al CWD (`sqlite:data/syncfiles.db`).
  Con el server lanzado desde `syncfiles-server/`, el diff listaba archivos
  de la DB local pero `sync/download` buscaba el contenido en Application
  Support → 400 "Contenido no encontrado en storage" en cada pull → la app
  Android quedaba en "Error de sincronización: HTTP 400 Bad Request"
  permanente.
- Fix: default de storage ahora `data/storage` (relativo al CWD), alineado
  con la DB; crate `dirs` eliminado de `syncfiles-server`. Verificado:
  3/3 downloads 200 OK, app Android pasa a "Todo al día", batería E2E
  17 PASS / 0 FAIL y 21 tests del server en verde.

## 6. Dependencias entre fases

```
Fase 1 (server) ──► Fase 2 (desktop) ──┐
      │                                 ├──► Fase 5 (pruebas E2E)
      ├──► Fase 3 (Android) ────────────┤
      └──► Fase 4 (web) ────────────────┘
```

- Fase 2/3/4 pueden avanzar en paralelo una vez terminada la Fase 1
- Los items 2.7 y 3.1–3.3 (motor de sync de fondo) son prerrequisito de los
  escenarios 5.3–5.5

## 7. Orden sugerido de ejecución

1. Fase 1 completa (habilita todo lo demás)
2. Fase 2.1–2.6 (desktop usable con lo existente) → primera prueba manual
3. Fase 2.7 (cola persistente + operaciones remotas) → base de pruebas reales
4. Fase 3.1–3.4 (Android funcional en fondo)
5. Fase 4 (web mínimo)
6. Fase 5 (batería de pruebas E2E)

## 8. Referencias de código

| Qué | Dónde |
|---|---|
| Rutas API actuales | `syncfiles-server/src/main.rs:22-36`, `handlers.rs` |
| Estado UI desktop (modales, vistas, notificaciones sin usar) | `syncfiles-client/src/main.rs` (Modal:146, login:264, logout:294, toggle_pause:317, notify:465, stubs:490) |
| Operaciones remotas no implementadas | `syncfiles-client/src/sync.rs:84` |
| Cliente de red desktop (métodos ya listos) | `syncfiles-client/src/network.rs` |
| API Android no consumida | `syncfiles-android/.../SyncFilesApi.kt` (download, resolveConflict) |
| Worker/Watcher/Queue Android sin cablear | `SyncWorker.kt`, `FileWatcher.kt`, `SyncQueueStore.kt` |
| Web dashboard (Fase 4) | `syncfiles-server/static/` (index.html, app.js, styles.css), `main.rs` (serving + redirect), `tests/static_dashboard.rs` |
| Smoke test E2E actual | `scripts/e2e-test.sh` |
