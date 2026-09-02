# Cuestionario de cierre de arquitectura SyncFiles

## Línea base definida

- V1: SQLite + almacenamiento local + REST/JSON + polling + criterio de archivo más nuevo con confirmación del usuario + posibilidad de conservar versiones alternativas por copia/renombrado.
- V2: NAS + PostgreSQL + Kafka + E2E + resolución de conflictos avanzada.
- WebSocket y gRPC quedan como alternativas de transporte a evaluar en fases posteriores; no son requisito obligatorio de v1 ni de v2.

## Sección 2 — Principios de diseño

### 2.1. ¿Cuál es la decisión exacta de conflicto en v1?  [RESPONDIDA]
- La versión más reciente gana solo por defecto.
- El usuario debe confirmar la resolución cuando hay conflicto real.
- Las opciones son: aceptar la versión más reciente, conservar la alternativa como copia en conflicto, renombrar la alternativa para revisión manual o dejar ambas versiones persistidas.

### 2.2. ¿Qué significa “archivo más nuevo” en la práctica? [RESPONDIDA]
- En v1, la regla base es `modified_at` con tolerancia de ±2–5 segundos.
- Si la diferencia temporal es ambigua, se puede revisar el `checksum` como diagnóstico, pero la decisión no depende del hash como criterio principal.
- La resolución sigue siendo por la versión más reciente por defecto, con confirmación del usuario antes de cerrar el conflicto.

### 2.3. ¿Qué debe ocurrir cuando ambos lados cambian el mismo archivo pero en rutas distintas? [RESPONDIDA]
- Si el contenido final difiere aunque la ruta cambie, se trata como conflicto de contenido y no como cambio inocuo.
- Si el contenido es idéntico y solo cambia la ruta, se considera renombrado o copia, no conflicto.
- En caso de conflicto, se conserva la alternativa como copia/renombrado y la resolución final requiere confirmación del usuario.

### 2.4. ¿Cómo se define el origen de verdad de metadatos en v1? [RESPONDIDA]
- El servidor siempre gana y es la fuente de verdad de metadatos.
- El cliente solo propone cambios; no reescribe ni valida autoridad de estado sin confirmar con el servidor.
- La metadata canónica es la del servidor: `checksum`, `modified_at`, `path_hash`, `device_id`, `last_sync` y estado final de sincronización.

### 2.5. ¿Qué reglas exactas debe seguir la cola local persistente? [RESPONDIDA]
- La cola se escribe antes de intentar la operación y cada item recibe un `idempotency_key`.
- Si la app reinicia durante un upload, la operación puede reintentarse sin duplicar efectos porque el servidor dedupea por la misma clave.
- El cliente mantiene estados `queued`/`in_flight`/`retry` para reintentar de forma segura.

### 2.6. ¿Qué tolera la arquitectura de v1 respecto a fallos parciales? [RESPONDIDA]
- La operación debe rechazarse y reintentarse; no se deja un archivo en estado intermedio no consistente.
- La definición de operación completa es aquella que tiene confirmación del servidor y metadatos actualizados.
- La operación queda incompleta si no existe confirmación final o si hubo error de escritura, red o timeout antes del commit.

### 2.7. ¿Qué se considera una “conflicto real” y qué un “cambio no conflictivo”? [RESPONDIDA]
- Un conflicto real ocurre cuando ambos lados cambiaron desde la última sincronización y el contenido final difiere por hash.
- Un cambio no conflictivo ocurre cuando solo un lado cambia el archivo, o cuando la ruta cambia y el contenido final sigue siendo idéntico.
- Un borrado local frente a modificacion remota se resuelve como borrado canónico si no hay una versión posterior que el usuario quiera conservar.

### 2.8. ¿Quién decide la resolución final del conflicto en v1? [RESPONDIDA]
- El usuario confirma la decisión final con opción de aceptar la versión más reciente o conservar la alternativa como copia/renombrado.
- El sistema no sobrescribe silenciosamente; el cliente y el servidor solo ejecutan la decisión confirmada.

### 2.9. ¿Qué tipo de decisiones quedan definidas como “regla por defecto” y cuáles deben ser decisiones manuales del usuario?
- Confirmación antes de sobrescribir.
- Confirmación antes de conservar ambas versiones.
- Decisión de renombrar, mover o “mantener ambas”.

### 2.10. ¿Qué pasa con la seguridad en v1 y en v2?
- ¿Los metadatos del servidor pueden estar visibles en v1?
- ¿Qué datos se consideran operativos y cuáles serán cifrados en v2?
- ¿Cuál es la separación exacta entre `display_name`, `full_path` y metadatos operativos?

## Sección 3 — Componentes principales

### 3.1. ¿Cuál es la topología exacta de v1?
- Un solo proceso backend / monolito modular.
- Una base SQLite.
- Un storage local.
- ¿Hay más de una instancia o no?

### 3.2. ¿Qué módulos internos deben existir con responsabilidad clara?
- Identity
- Sync Orchestrator
- Metadata Catalog
- Conflict Resolution
- Storage Gateway
- Background Jobs
- Audit
- Versioning
- ¿Cuáles están activos en v1 y cuáles son stubs?

### 3.3. ¿Qué capa de almacenamiento es la real en v1?
- `LocalDiskStorageProvider` como implementación concreta.
- ¿Dónde se guardan los archivos?
- ¿Dónde se guardan los conflictos?
- ¿Qué estructura de carpetas usa por usuario/dispositivo?

### 3.4. ¿Qué layout de metadatos debe existir en SQLite?
- ¿Tabla `files`?
- ¿Tabla `sync_queue`?
- ¿Tabla `conflicts`?
- ¿Tabla `sessions`?
- ¿Tabla `audit_log`?
- ¿Qué columnas mínimas son obligatorias?

### 3.5. ¿Qué define un archivo “sincronizado” vs “pendiente” vs “con conflicto”?
- ¿Se basa en un único campo de estado?
- ¿O hay una combinación de `checksum`, `modified_at`, `synced_at`, `device_id` y `status`?

### 3.6. ¿Qué contrato de API debe existir en v1?
- ¿Cuáles son los endpoints y métodos?
- ¿Qué payload envía el cliente?
- ¿Qué respuesta devuelve el servidor en upload, download, delete, rename, move, copy?
- ¿Qué estructura llevan los errores?

### 3.7. ¿Qué debe incluir el `idempotency_key`?
- ¿Se genera por operación o por archivo?
- ¿Cuánto dura la deduplicación?
- ¿Qué pasa si el mismo archivo se intenta dos veces con misma clave y un contenido distinto?

### 3.8. ¿Cuál es la política de polling exacta en v1?
- ¿Frecuencia fija de 30s?
- ¿Puede adaptarse por carga?
- ¿El cliente solicita deltas por usuario/dispositivo o por archivo?

### 3.9. ¿Qué hace el cliente en segundo plano?
- ¿Reintenta automáticamente?
- ¿Drena a cola persistente?
- ¿Qué se almacena localmente para recuperación tras cierre?

### 3.10. ¿Qué es exactamente un “conflicto” desde la vista del cliente y del servidor?
- Diferencias de `modified_at`.
- Diferencias de `checksum`.
- Ruta distinta pero mismo archivo.
- Archivo borrado en un dispositivo y contenido actualizado en otro.

### 3.11. ¿Qué debemos mostrar en la UI en cada estado?
- “Sincronizado”, “Sincronizando”, “Con conflictos”, “Error”, “Pausado”.
- ¿Qué datos se muestran por archivo y por dispositivo?
- ¿Qué botón de acción tiene el usuario ante un conflicto?

### 3.12. ¿Cuáles son los mensajes mínimos de auditoría y observabilidad en v1?
- `auth.login_success`
- `auth.login_failed`
- `sync.file_uploaded`
- `sync.file_downloaded`
- `sync.conflict_detected`
- ¿Qué más hace falta? `sync.delete`, `sync.rename`, `sync.move`, `sync.copy`, `retry`, `quarantine`.

### 3.13. ¿Qué debe estar definido para cerrar la parte operativa de v1?
- Política de reintentos.
- Contraseña de backoff y jitter.
- Umbrales de cola.
- Qué hace el sistema cuando se supera un umbral.
- Qué hace el usuario cuando se llega a un estado degradado.

### 3.14. ¿Qué debe estar definido para cerrar la parte de v2?
- NAS obligatorio.
- PostgreSQL como persistencia.
- Kafka para event streaming.
- E2E.
- Conflicto avanzado.
- ¿WebSocket y gRPC son requisitos de v2 o solo opciones de transporte?

## Cierre recomendado

Antes de avanzar a diseño detallado, estas 8 decisiones deben quedar cerradas en texto formal:

1. Reglas de conflicto v1.
2. Decisión de quien gana entre servidor y cliente.
3. Política de copia/renombrado en conflicto.
4. Modelo de metadatos y estados del archivo.
5. Estructura de SQLite y layout de almacenamiento local.
6. Contrato de API y payloads mínimos.
7. Política de reintentos y recuperación.
8. Línea base V1 vs V2 y la evolución de transporte.

Estas preguntas deben responderse de forma explícita, una por una, antes de iniciar el diseño de detalle del cliente y del servidor.
