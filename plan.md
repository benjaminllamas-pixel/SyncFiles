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
- [ ] Añadir pantalla de login (email/contraseña/URL servidor) al arrancar si no hay sesión
      — reutilizar `login()` (`syncfiles-client/src/main.rs:264`)
- [ ] Añadir botón "Cerrar sesión" que invoque `logout()` (`main.rs:294`)
- [ ] Mostrar `login_error` y estado `connecting` en la UI

### 2.2 Pausar/Reanudar sincronización
- [ ] Botón "Pausar/Reanudar" conectado a `toggle_pause()` (`main.rs:317`)
- [ ] Reflejar el estado `paused` visualmente (header/badge)

### 2.3 Notificaciones/toasts
- [ ] Renderizar la pila de notificaciones (`notify()` ya existe, `main.rs:465`)
- [ ] Llamar `prune_notifications()` en cada frame (main.rs:479)
- [ ] Cerrar con botón X y auto-descarte tras N segundos

### 2.4 Modales de operaciones sobre archivos
- [ ] Implementar render de `Modal` (Rename/Move/Copy/Delete/ConflictResolve, `main.rs:146`)
- [ ] Menú contextual (clic derecho) sobre un archivo → abrir modal correspondiente
      usando `context_menu_file`/`context_menu_pos`
- [ ] Conectar a `network.rs`: `move_file`, `copy_file`, `resolve_conflict` ya existen

### 2.5 Vistas Dashboard y Devices
- [ ] Dashboard: estado conexión, resumen archivos, tamaño, última sincronización
      (consumir `storage/stats` de Fase 1.6)
- [ ] Devices: lista de dispositivos usando `self.devices` (`main.rs:425`, ya se descarga
      pero nunca se muestra) + `GET /devices` (Fase 1.5)

### 2.6 Correcciones de bugs UI
- [ ] Conflictos: mostrar ruta del archivo, no `file_id` (`main.rs:407`)
- [ ] Vista Conflictos accesible también en ventana estrecha (<680px, `main.rs:795`)
- [ ] Preferencias: persistir cambios con `config.save()`; el cambio de carpeta debe
      aplicar de verdad (reescribir `sync_root` en disco)
- [ ] "Sincronizar ahora" debe disparar un ciclo real del `SyncEngine`, no solo log
- [ ] Estado "Conectado": setear `connected = true` cuando el último ciclo de sync tuvo éxito
- [ ] Eliminar o implementar el comando de selección de carpeta con `rfd` (dependencia
      presente pero sin uso)

### 2.7 Sincronización subyacente (para pruebas reales)
- [ ] Implementar rename/move/copy remotos en `syncfiles-client/src/sync.rs:84`
      (hoy se descartan con "Operación remota no implementada")
- [ ] Persistir la cola entre reinicios (hoy es solo en memoria)

## 3. Fase 3 — Android: cablear lo ya escrito

> El app ya tiene login/home/subir/sincronizar manual; falta el fondo y las vistas.

- [ ] 3.1 Enqueuear `SyncWorker` (WorkManager) — periodic + constraints de red
- [ ] 3.2 Instanciar `FileWatcher` (FileObserver) para detectar cambios en la carpeta elegida
- [ ] 3.3 Conectar `SyncQueueStore` con el worker (ya ambos existen, sin uso)
- [ ] 3.4 Añadir pantalla de conflictos usando `resolveConflict` (ya en `SyncFilesApi.kt`,
      sin llamadas)
- [ ] 3.5 Añadir botón de descarga / vista de archivos usando `download` (ya en la API,
      sin llamadas)
- [ ] 3.6 Pantalla de ajustes: URL servidor, carpeta, intervalo, logout
- [ ] 3.7 Indicador de estado de sincronización (en progreso / al día / error)

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
