# Backlog de Implementación V1

## 1. Objetivo

Este backlog convierte la arquitectura aprobada en un plan práctico de implementación para la versión V1 del producto.

La prioridad es entregar una versión estable, segura y testable con:
- SQLite como base local y operativa,
- almacenamiento local como capa inicial,
- REST/JSON más polling,
- sincronización bidireccional con conflictos guiados por usuario,
- estructura modular para crecer hacia V2 sin reescribir la base.

## 2. Principio de ejecución

Se implementará en orden de dependencia real:
1. base común y contratos,
2. persistencia y almacenamiento local,
3. autenticación y API server,
4. cliente + sincronización + conflictos,
5. pruebas y release.

## 3. Epicas y tareas

### Epic 1: Fundamentos y contrato de sistema

#### Tarea 1.1 — Base del proyecto
- Crear estructura de repositorio por dominio.
- Definir contratos de datos compartidos.
- Definir configuración por entorno.
- Definir convenciones de naming y errores.

Criterio de aceptación:
- el repo permite construir backend, cliente y shared models sin duplicación funcional.

#### Tarea 1.2 — Modelo de datos base V1
- crear migraciones de SQLite,
- modelar usuarios, dispositivos, sesiones, archivos, cola, conflictos y auditoría,
- definir índices y constraints de negocio.

Criterio de aceptación:
- la base puede soportar autenticación, sincronización y auditoría mínima.

### Epic 2: Backend y API

#### Tarea 2.1 — Autenticación y sesiones
- login, logout, token de sesión,
- validación de `device_id`,
- revocación y expiración,
- API de estado de sesión.

Criterio de aceptación:
- sesión válida no se puede reutilizar desde un dispositivo no autorizado.

#### Tarea 2.2 — Storage provider local
- implementar `LocalDiskStorageProvider`,
- manejar rutas normalizadas,
- validar checksums,
- guardar/leer archivos y metadatos de forma segura.

Criterio de aceptación:
- cada archivo se puede guardar y recuperar sin corrupción ni pérdida de integridad.

#### Tarea 2.3 — REST API de sincronización
- `GET /files/changes`
- `POST /files/upload`
- `POST /files/download`
- `POST /files/delete`
- `POST /files/rename`
- `POST /files/move`
- `POST /files/copy`
- `POST /conflicts/resolve`
- manejo de `idempotency_key`, 401/403/409/429/5xx.

Criterio de aceptación:
- el cliente y el servidor pueden sincronizar estado y contenido sin duplicados ni confirmación falsa.

### Epic 3: Cliente

#### Tarea 3.1 — App shell y configuración
- inicialización de app,
- manejo de configuración,
- almacenamiento local seguro,
- lifecycle del cliente,
- sesión persistida.

Criterio de aceptación:
- el cliente arranca, guarda sesión y puede continuar tras reinicio.

#### Tarea 3.2 — Watcher y cola local
- detector de cambios del sistema de archivos,
- debounce por archivo,
- normalización de rutas,
- persistencia de `sync_queue`,
- política de reintentos con backoff.

Criterio de aceptación:
- un cambio local queda en cola y puede recuperarse tras un corte abrupto.

#### Tarea 3.3 — Sincronización funcional
- polling de cambios,
- pull remoto y push local,
- control de estado `queued`, `in_flight`, `retry`, `synced`, `conflict`, `failed`.

Criterio de aceptación:
- la app puede sincronizar archivos entre cliente y servidor sin pérdida de intención.

#### Tarea 3.4 — Resolución de conflicto
- detectar conflictos reales,
- preservar alternativa local/remota,
- mostrar confirmación de resolución,
- soportar copiar, renombrar o conservar versión por decisión del usuario.

Criterio de aceptación:
- no hay sobrescritura silenciosa; el usuario decide explícitamente.

### Epic 4: Validación y release

#### Tarea 4.1 — Pruebas unitarias
- hashing, rutas, conflict policy,
- backoff, idempotency, session logic,
- serialización de payloads y validación de estado.

#### Tarea 4.2 — Pruebas de integración
- flujo completo cliente-servidor,
- upload/download,
- conflictos y recuperación,
- red inestable y reintentos.

#### Tarea 4.3 — Pruebas de seguridad
- autenticación, autorización, sesiones,
- checksum y compatibilidad,
- manejo de ataques básicos y rutas no autorizadas.

#### Tarea 4.4 — Release V1
- empaquetado por plataforma,
- firma y validación de artefactos,
- CI/CD básico,
- rollout y rollback controlado.

Criterio de aceptación:
- se puede entregar una versión estable de cliente y backend con trazabilidad y rollback.

## 4. Orden recomendado de implementación

1. Base del proyecto y SQLite
2. Storage local y autenticación
3. API REST de sincronización
4. Cliente shell y sesión
5. Watcher, cola y sincronización
6. Conflictos y UX
7. Pruebas automáticas
8. Release

## 5. Dependencias clave

- Autenticación depende de la base y del modelo de sesión.
- API depende de almacenamiento y autenticación.
- Cliente shell depende de la API y de la sesión.
- Watcher depende de la API y de la cola local.
- Conflictos depende de la sincronización funcional.
- Release depende de pruebas y hardening final.

## 6. Criterios de cierre de la fase V1

La V1 queda cerrada cuando:
- el cliente y el servidor sincronizan archivos sin corrupción,
- los conflictos se resuelven con confirmación del usuario,
- la cola recupera estados tras reinicios o caídas,
- las pruebas críticas pasan y el release puede desplegarse sin riesgo crítico.

## 7. Siguiente paso concreto

El próximo paso práctico es convertir este backlog en tareas de sprint con estimación y owner por módulo, y luego empezar por:
- esquema SQLite,
- storage local,
- auth y session,
- endpoints REST iniciales.
