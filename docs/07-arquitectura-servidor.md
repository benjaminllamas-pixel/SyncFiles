# Arquitectura del Servidor

## Descripción
Diseño de la arquitectura backend para el MVP de SyncFiles, definiendo topología, módulos, contratos, persistencia, seguridad y operación del servidor.

## 1. Objetivo y Alcance del Servidor (MVP)

### 1.1 Objetivo
Definir un backend confiable para autenticación y sincronización bidireccional básica de archivos, priorizando simplicidad operativa y consistencia de datos.

### 1.2 Alcance incluido en v1
- Autenticación con cuentas precreadas y sesión server-side.
- Orquestación de sincronización bidireccional cliente-servidor.
- Operaciones de archivo: subir, bajar, borrar, renombrar, mover y copiar.
- Resolución de conflictos por **copia en conflicto** como regla universal.
- API versionada `/v1` sobre REST/JSON + TLS 1.3.

### 1.3 Fuera de alcance en v1
- Versionado funcional de archivos.
- Compartición de archivos.
- Push en tiempo real obligatorio (WebSocket/SSE).
- NAS obligatorio y alta disponibilidad distribuida.
- Cifrado E2E obligatorio.

## 2. Topología del Servidor

### 2.1 Topología operativa actual (v1)
- **Single-node** en operación.
- Backend en **monolito modular**.
- Persistencia de metadatos en SQLite.
- Almacenamiento de archivos mediante `StorageProvider` con `LocalDiskStorageProvider`.
- **Capacidad objetivo v1:** 10 usuarios activos, 20 dispositivos activos, 250 MB máximo por archivo, 20 GB cuota por usuario.

### 2.2 Arquitectura objetivo (evolución)
Se mantiene una arquitectura objetivo orientada a mayor desacoplamiento por dominios, pero sin activarla como requisito de operación en v1. La transición se ejecuta solo por señales operativas (capacidad, latencia, errores y cola).

### 2.3 Supuestos de despliegue
- SPOF aceptado en v1 con mitigación por backups y restauración verificada.
- Downtime planificado permitido.
- Sin clúster activo-activo en v1.

## 3. Módulos Internos y Responsabilidades

### 3.1 Módulos activos del backend

| Módulo | Responsabilidad principal | Estado v1 |
|---|---|---|
| `Identity` | Login, validación de sesión y lifecycle de token | Activo |
| `Sync Orchestrator` | Coordinación del flujo de sincronización | Activo |
| `Metadata Catalog` | Fuente de verdad de metadatos | Activo |
| `Conflict Resolution` | Aplicación de política de conflicto | Activo |
| `Storage Gateway` | Abstracción de acceso a archivos | Activo |
| `Background Jobs` | Reintentos, drenaje de cola y tareas operativas | Activo (mínimo) |
| `Audit` | Registro de eventos operativos y seguridad | Activo |
| `Versioning` | Extensión futura de historial | Stub (no funcional) |

### 3.2 Reglas de acoplamiento
- Los módulos se comunican por contratos internos explícitos.
- No se permite dependencia directa de módulos de dominio a implementaciones concretas de storage.
- La fuente de verdad de estado de sincronización es `Metadata Catalog`.

## 4. Flujo de Sincronización Server-Side

### 4.1 Contrato de interfaz
- Protocolo: REST/JSON sobre HTTP/1.1 + TLS 1.3.
- API versionada con prefijo `/v1`.
- Detección remota en cliente por polling fijo (30 segundos).
- Validación de cadena de confianza del sistema operativo en cliente; política fail-closed ante error TLS.  

### 4.2 Reglas de consistencia
- Atomicidad por archivo (no por lote completo).
- Idempotencia obligatoria: cada operación incluye `idempotency_key` persistida; deduplicación temporal de **24 horas** en servidor (si reintentos dentro de ventana, se descarta sin duplicar efecto).
- Orden canónico de aplicación por `server_seq` para garantizar consistencia reproducible.

### 4.3 Conflictos
- Condición de conflicto: ambos lados modifican desde `last_sync` y el hash del contenido final difiere.
- Política universal de resolución: generar **copia en conflicto** con convención de nombre legible (incluir dispositivo y timestamp); no hay sobrescritura silenciosa.
- Regla `delete` vs `modify` concurrente: `delete` define el estado canónico; `modify` se preserva como copia en conflicto.
- Normalización temporal: `modified_at` en UTC epoch ms con tolerancia de deriva ±2–5 segundos, validando siempre con hash.

## 5. Datos, Persistencia y Almacenamiento

### 5.1 Persistencia de metadatos
- SQLite como base operativa en v1.
- Esquema preparado para migración futura a PostgreSQL.
- Identificación de ruta por `path_hash` (SHA-256 de ruta relativa normalizada).

### 5.2 Almacenamiento de archivos
- Contrato `StorageProvider` como punto único de acceso.
- Implementación v1: `LocalDiskStorageProvider`.
- Evolución a NAS o almacenamiento externo solo por trigger post-MVP (capacidad > 75 % por 2 semanas, O tercer dispositivo con demanda compartida).

### 5.3 Integridad
- Verificación por `SHA-256` + `size_bytes` en subida y descarga.
- Discrepancias detectadas: **1–2 reintentos automáticos**; si persiste, archivo u operación pasa a **cuarentena**.
- Incidente aislado de integridad no bloquea sesión completa del usuario.

## 6. Seguridad del Servidor

### 6.1 Autenticación y sesión
- Token opaco con estado en servidor.
- Sesión vinculada a `device_id`; validación de correspondencia sesión/usuario/dispositivo en cada request.
- **Política de sesión completa:**
  - TTL base: **24 horas** con renovación deslizante (se extiende con actividad válida).
  - Expiración absoluta: **7 días** (incluso si activa).
  - Máximo de sesiones concurrentes por usuario: **3**.
  - Rebase al arranque post-cierre/reinicio para recuperar consistencia.

### 6.2 Autorización
- Control por acción y recurso (ownership estricto).
- El cliente no es fuente de verdad para permisos; servidor valida siempre identidad, permisos y estado crítico antes de ejecutar operaciones.

### 6.3 Canal seguro
- **TLS 1.3 obligatorio** en todo tráfico cliente-servidor.
- Validación de cadena de confianza del sistema operativo sin cambios de certificado.
- Sin certificate pinning obligatorio en v1.
- Política **fail-closed** ante error TLS: rechazo de operación, no fallback degradado.

### 6.4 Límite de seguridad v1
- E2E no obligatorio en v1; metadatos operativos visibles en servidor (dentro del modelo MVP).
- Secretos locales almacenados en almacén seguro nativo de plataforma (Keychain macOS, Credential Manager Windows, Secret Storage Linux).

## 7. Operación, Observabilidad y Evolución

### 7.1 Observabilidad mínima v1
- Logs estructurados NDJSON con campos mínimos: `ts`, `level`, `event`, `user_id`, `device_id`, `message`.
- Eventos obligatorios: auth (login success/failed), sync (upload/download/delete/rename/move/copy), conflictos detectados.
- Métricas básicas: latencia de sync (p50/p95), errores, sesiones activas, profundidad de cola, tasa de reintentos, tasa de conflictos.
- Rotación diaria de logs; retención mínima 30 días.
- Correlación por operación: `request_id`, `op_id`, `device_id` propagados en logs y métricas.

### 7.2 Resiliencia operativa
- Reintentos con backoff exponencial + jitter (base 2s, máximo 5 intentos por operación).
- Control de presión: respuesta **429 Too Many Requests** con cabecera `Retry-After` cuando aplique.
- Recuperación basada en cola persistente y reconciliación al arranque.
- Clasificación de errores: reintentables (red, timeout, 408, 429, 5xx); terminales (400, 401, 403, 404, 422).

### 7.3 Backup y restauración
- **Backups automatizados en v1** para metadatos (dump SQLite diario) y archivos (incremental diario).
- Almacenamiento fuera del nodo principal.
- Verificación periódica de restauración: restore test semanal con control de integridad por checksum.
- Bitácora de incidentes y recuperación con trazabilidad completa.
- **Objetivos operativos:** RPO ≤ 24 h, RTO < 4 h.

### 7.4 Evolución post-MVP (resumen)
- Kafka, Redis, workers distribuidos, WebSocket/gRPC y escalado multi-nodo real se activan **solo por triggers operativos** y decisión de fase, no forman parte del baseline operativo de v1.
- Versionado funcional, compartición de archivos, cifrado E2E obligatorio, NAS obligatorio quedan **post-MVP** con criterios de activación documentados.

## Checklist de validación de coherencia

- [x] Mantiene alcance MVP (auth + sync básico) sin ampliar funcionalidades no aprobadas.
- [x] Respeta contrato REST `/v1` + TLS 1.3 + polling 30s en v1.
- [x] Mantiene regla de conflicto por copia en conflicto universal.
- [x] Conserva SQLite + `LocalDiskStorageProvider` como estado operativo v1.
- [x] No convierte Kafka/Redis/NAS/E2E en requisitos obligatorios de v1.
- [x] Conserva separación explícita entre estado v1 y evolución post-MVP con triggers claros.