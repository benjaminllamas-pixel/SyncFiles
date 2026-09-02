# Cuestionario de cierre de arquitectura SyncFiles

## Línea base definida

- V1: SQLite + almacenamiento local + REST/JSON + polling + criterio de archivo más nuevo con confirmación del usuario + posibilidad de conservar versiones alternativas por copia/renombrado.
- V2: NAS + PostgreSQL + Kafka + E2E + resolución de conflictos avanzada.
- WebSocket y gRPC quedan como alternativas de transporte a evaluar en fases posteriores; no son requisito obligatorio de v1 ni de v2.

## Sección 2 — Principios de diseño

### 2.1. ¿Cuál es la decisión exacta de conflicto en v1?
- ¿La versión más reciente gana siempre o solo por defecto?
- ¿El usuario debe confirmar la resolución o el sistema puede decidir solo?
- ¿Qué opciones se ofrecen exactamente: aceptar versión nueva, conservar versión antigua, copiar, renombrar, dejar ambas versiones?

### 2.2. ¿Qué significa “archivo más nuevo” en la práctica?
- ¿Se basa solo en `modified_at`?
- ¿Qué pasa si `modified_at` es igual o no es fiable?
- ¿Se valida además con `checksum` antes de aceptar una resolución?

### 2.3. ¿Qué debe ocurrir cuando ambos lados cambian el mismo archivo pero en rutas distintas?
- ¿Se trata como conflicto de contenido o como renombrado + modificación?
- ¿Qué se guarda en el historial local y remoto?

### 2.4. ¿Cómo se define el origen de verdad de metadatos en v1?
- ¿El servidor siempre gana?
- ¿El cliente puede confirmar o reescribir metadatos sin validación?
- ¿Qué metadata es canónica: `checksum`, `modified_at`, `path_hash`, `device_id`, `last_sync`?

### 2.5. ¿Qué reglas exactas debe seguir la cola local persistente?
- ¿Se escribe antes de la operación o después del primer intento?
- ¿Qué ocurre si se reinicia la app durante un upload?
- ¿Cómo se evita duplicar la misma operación?

### 2.6. ¿Qué tolera la arquitectura de v1 respecto a fallos parciales?
- ¿Debe rechazar la operación con reintento?
- ¿Puede dejar un archivo en estado intermedio?
- ¿Qué define una operación “completa” y “incompleta”?

### 2.7. ¿Qué se considera una “conflicto real” y qué un “cambio no conflictivo”?
- ¿Un borrado local vs un cambio remoto es conflicto?
- ¿Un renombrado local + creación remota del mismo archivo es conflicto?
- ¿Un cambio de content hash igual pero `modified_at` distinto es conflicto o no?

### 2.8. ¿Quién decide la resolución final del conflicto en v1?
- ¿El sistema?
- ¿El usuario?
- ¿El servidor?
- ¿El cliente?

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
