# Roadmap del Proyecto

## 1. Visión general

El roadmap de SyncFiles está organizado por fases funcionales y por evolución natural del producto.

La regla de decisión es clara:
- V1 entrega la sincronización segura y operable con almacenamiento local y SQLite,
- V2 incorpora NAS, PostgreSQL, E2E, Kafka y resolución avanzada de conflictos,
- la transición no depende de métricas reactivas, sino de la estabilidad operativa y la completitud funcional de V1.

## 2. Fase 0: validación de arquitectura y definición de contrato

### Objetivo
Preparar la base de diseño y dejar cerrados los acuerdos básicos del producto.

### Entregables
- definición de alcance V1/V2,
- decisión de conflicto de V1,
- reglas de estado de sincronización,
- esquema de metadatos mínimo,
- contratos REST y política de API,
- seguridad y crypto baseline.

### Backlog crítico
- [ ] cerrar V1 vs V2,
- [ ] definir `path_hash`, `checksum`, `modified_at`, `device_id`, `session_id`,
- [ ] confirmar conflictos y UX de resolución,
- [ ] definir almacenamiento local y fallback,
- [ ] cerrar diseño del cliente y backend iniciales.

## 3. Fase 1: MVP de sincronización (V1)

### Objetivo
Entregar una solución funcional de sincronización para un único usuario y varios dispositivos con almacenamiento local y SQLite.

### Alcance
- sincronización bidireccional básica,
- autenticación y sesión,
- polling REST/JSON,
- persistencia local con SQLite,
- almacenamiento local con `LocalDiskStorageProvider`,
- detección de cambios y cola local,
- conflicto con elección del usuario.

### Backlog principal
- [ ] corrección y validación de rutas normalizadas,
- [ ] implementación de `sync_queue` con persistencia y reintentos,
- [ ] upload/download/delete/rename/move/copy,
- [ ] lista de cambios por polling,
- [ ] validación de checksum,
- [ ] lógica de conflicto y alternativa conservada,
- [ ] auditoría mínima,
- [ ] manejo de sesión y expiración,
- [ ] primer release de escritorio y Android.

### Criterios de salida
- la app sincroniza archivos de forma fiable en dos dispositivos,
- los conflictos no borran datos sin confirmación del usuario,
- la cola soporta reinicio y reintentos,
- no hay pérdida de datos bajo fallos transitorios esperados.

## 4. Fase 2: estabilización y hardening de V1 — COMPLETADA

### Objetivo
Hacer que V1 sea operable y robusta para uso real con un número moderado de archivos y dispositivos.

### Entregables
- pruebas de integración (39 tests server: 22 API + 8 dashboard + 9 lib),
- pruebas de seguridad (sesión expirada 401, rate limit 429, Content-Type,
  path traversal/absoluto — con fix real de rutas absolutas),
- métricas y observabilidad (logging JSON con request_id/correlation ID,
  health checks `/health/live` + `/health/ready`),
- pipeline de CI/CD (`.github/workflows/ci.yml`: Rust check/test/clippy +
  Android assembleDebug con artefacto),
- distribución de clientes de producción (binarios release, APK firmado,
  cliente Emacs),
- migraciones versionadas idempotentes (`syncfiles-server/migrations/`).

### Backlog principal
- [x] pruebas unitarias de sincronización (31 en `syncfiles-client`:
  network con mock HTTP, sync engine, metadata, config; 15 en models),
- [x] pruebas de conflicto y recuperación (409 → resolución, cola
  persistida reanudada tras reinicio, E2E fase 5),
- [x] pruebas de seguridad de sesión y acceso (expiración, revocación de
  dispositivos, rate limiting, validación de paths),
- [x] pruebas de rendimiento sobre archivos pequeños y medianos (E2E con
  binario 64 KiB, checksum verificado),
- [x] testing de redes inestables (E2E fase 5.5: server caído → fallo
  limpio; recuperación → reintento),
- [x] configuración de actualización de clientes (documentado en
  `docs/48-documentacion-usuario.md`),
- [x] release management y rollback (CI por PR a main; rollback orquestado
  queda para despliegue multi-instancia — V2).

## 5. Fase 3: V2 preparación y evolución natural

### Objetivo
Cuando V1 esté implementada y validada, evolucionar a la arquitectura de producción con capacidades avanzadas.

### Alcance esperado
- NAS como almacenamiento centralizado,
- PostgreSQL para metadatos y escalabilidad,
- Kafka para eventos y flujo asíncrono,
- E2E para contenido y metadatos sensibles,
- resolución avanzada de conflictos,
- observabilidad más profunda y operación multi-dispositivo.

### Backlog principal
- [ ] diseño de `StorageProvider` NAS,
- [ ] migración de SQLite a PostgreSQL con plan de compatibilidad,
- [ ] preparación de `Kafka` para eventos de sincronización,
- [ ] diseño de E2E y gestión de claves,
- [ ] separación de metadatos operativos y metadatos sensibles,
- [ ] soporte para conflictos con resolución automática o por flujo guiado,
- [ ] preparación de WebSocket/gRPC como opciones posteriores.

## 6. Fase 4: madurez y escalado

### Objetivo
Convertir SyncFiles en una plataforma de sincronización robusta y configurable para más usuarios, más volumen y más dispositivos.

### Alcance
- capacidad con volumen elevado,
- más dispositivos activos,
- mejor soporte para archivos grandes,
- diagnósticos de salud y fallos,
- políticas de retención y observabilidad más avanzadas.

## 7. Recomendación de priorización

La prioridad inmediata debe ser:
1. cerrar V1 y dejarlo operativo,
2. validar el flujo de sincronización con conflictos y reintentos,
3. estabilizar seguridad y pruebas,
4. preparar la transición a V2 por evolución natural, no por presión operativa.

## 8. Criterios de cierre del roadmap

El proyecto puede pasar a la siguiente fase cuando:
- V1 funciona sin errores funcionales críticos,
- los clientes se pueden instalar y actualizar con seguridad,
- el conflicto se resuelve sin pérdida de datos,
- la arquitectura tiene observabilidad y pruebas suficientes,
- el equipo tiene claridad de qué se introduce en V2 y por qué.

## 9. Conclusión

El roadmap recomendado para SyncFiles es un camino de especialización progresiva:
- V1 = fiable, unificado y simple,
- V2 = escalable, enriquecido y más seguro,
- siempre con una base funcional estable y validada antes de añadir complejidad.
