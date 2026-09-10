# Plan UI SyncFiles — Dejar el Frontend Listo para Pruebas Reales

> Objetivo: completar/habilitar toda la funcionalidad de UI (desktop egui, Android y web)
> para poder ejecutar pruebas reales de sincronización de extremo a extremo.
> Basado en el inventario de brechas de `docs/37-interfaz-usuario.md` y el código actual.

## 0. Alcance y criterios de salida

**Incluido:** todo lo necesario para probar sincronización real entre clientes y servidor
(login → subir/bajar/borrar/renombrar → conflictos → cola → actividad).

**Excluido (fases posteriores):** encriptación E2E, versionado, compartir, NAS avanzado.

**Criterios de éxito:**
- [ ] Login/logout visible y funcional en desktop (ya existe en Android/web pendiente)
- [ ] Sincronización real entre 2+ dispositivos con archivos de prueba
- [ ] Conflictos creados, detectados y resueltos desde la UI
- [ ] Cola de sincronización persistente con reintentos visible
- [ ] Registro de actividad/auditoría accesible desde la UI
- [ ] Web dashboard mínimo servido desde `syncfiles-server`

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

- [ ] 4.1 Servir `static/` con `actix-files` (`Files::new("/ui", "./static")`)
- [ ] 4.2 Login (contra `/auth/login`) con manejo de sesión (localStorage + Bearer)
- [ ] 4.3 Vista Dashboard: estado del servidor, storage stats, dispositivos
- [ ] 4.4 Vista Archivos: tabla con lista (`/files/list`), descarga (`/sync/download`),
      borrado (`/sync/delete`)
- [ ] 4.5 Vista Conflictos: listar y resolver (`/conflicts` + `/conflicts/resolve`)
- [ ] 4.6 Vista Actividad: log de eventos (`/activity`)
- [ ] 4.7 Responsive mínimo ( móvil ) para poder probar desde el navegador del teléfono
- [ ] 4.8 Activar CORS para permitir pruebas desde otros orígenes (Fase 1.7)

## 5. Fase 5 — Pruebas E2E reales

- [ ] 5.1 Levantar servidor local (`scripts/e2e-test.sh` como base)
- [ ] 5.2 Escenario multi-dispositivo: desktop + Android + web simultáneos
      con la misma cuenta, verificando propagación de cambios
- [ ] 5.3 Escenario de conflicto: editar el mismo archivo en 2 dispositivos offline,
      reconectar, resolver desde la UI
- [ ] 5.4 Escenario de reinicio: cerrar/abrir cliente con cola pendiente (verificar
      persistencia)
- [ ] 5.5 Escenario de red interrumpida: desconectar servidor a mitad de subida,
      verificar reintentos y notificaciones
- [ ] 5.6 Documentar resultados en `docs/41-estrategia-pruebas.md` y actualizar
      `docs/37-interfaz-usuario.md` con pantallas finales

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
| Smoke test E2E actual | `scripts/e2e-test.sh` |
