# Arquitectura General del Sistema

<!-- ...existing code... -->

## 1. Objetivo y Alcance

Definir la arquitectura de alto nivel de SyncFiles para un **MVP** enfocado en autenticación y sincronización básica de archivos entre clientes y servidor, priorizando simplicidad operativa y evolución incremental.

### 1.1 Alcance incluido en MVP
- **Autenticación + login** de **cuentas precreadas**.
- **Sincronización bidireccional** entre cliente y servidor.
- Operaciones soportadas:
  - subir
  - bajar
  - borrar
  - renombrar
  - mover
  - copiar
- Resolución de conflictos por **"copia en conflicto"**.
- Plataformas objetivo iniciales: **desktop + Android**.
- Capa de almacenamiento mediante abstracción `StorageProvider` con implementación inicial: **`LocalDiskStorageProvider`**.

### 1.2 Fuera de alcance en MVP
- Registro público/autónomo de usuarios.
- **Versionado** de archivos.
- **Compartición** de archivos.
- Modo **offline** como requisito obligatorio.
- **NAS como requisito de v2/post-MVP**; no es obligatorio en v1.
- Metas de gran escala, SLA/SLO estrictos y optimizaciones avanzadas de rendimiento.

### 1.3 Decisiones de alcance para v1
- **V1** define una arquitectura single-node con **SQLite** y almacenamiento local mediante `LocalDiskStorageProvider`.
- **NAS queda post-MVP**; la arquitectura mantiene compatibilidad futura vía `StorageProvider`.
- **REST/JSON + polling** es el transporte operativo de v1; no se exige WebSocket ni gRPC como requisito obligatorio.
- La **encriptación E2E no es obligatoria en v1** (queda planificada para evolución posterior).
- Los objetivos de escalado/alta disponibilidad avanzados se tratan en fases posteriores.

### 1.4 Línea base funcional V1 y V2
- **V1 (base estable):** SQLite, almacenamiento local, REST/JSON con polling, sesión server-side, conflictos con criterio de archivo más nuevo y opción de conservar versiones alternativas (copiar/renombrar). 
- **V2 (evolución):** NAS, PostgreSQL, Kafka, E2E, resolución de conflictos avanzada y validación multi-dispositivo. 
- **WebSocket y gRPC** son alternativas de transporte a valorar en fases posteriores, no un requisito obligatorio de v2 ni de v1.

## 2. Principios de Diseño

### 2.1 Principios rectores

- **Funcionalidad estable antes que optimización:** la prioridad en v1 es que el ciclo subir/sincronizar/bajar/modificar funcione de forma confiable. La optimización de rendimiento y escala se aborda en fases posteriores.
- **Metadatos como fuente única de verdad:** el estado de sincronización de cualquier archivo se determina exclusivamente por los metadatos del servidor (`checksum`, `modified_at`, `device_id`). El cliente solo propone cambios; no reescribe ni valida autoridad de estado sin confirmación del servidor. No existen fuentes secundarias de verdad en v1.
- **Mínimo privilegio:** separación estricta de permisos entre cliente, API y almacenamiento.
- **Consistencia estricta ante fallos parciales:** ante una operación incompleta (corte de red, error de escritura), el sistema prefiere rechazar y reintentar antes que dejar un estado inconsistente.
- **Idempotencia universal en operaciones remotas:** toda operación de sync puede reejecutarse sin efectos duplicados. El servidor detecta y descarta reintentos de operaciones ya aplicadas.
- **Archivo más nuevo como criterio base de convergencia:** en v1, el archivo con `modified_at` más reciente define la versión canónica por defecto, usando una tolerancia de ±2–5 segundos para compensar deriva temporal entre dispositivos. La decisión puede ser aplicada por el sistema solo cuando el cambio es inequívoco; si hay conflicto real, el flujo ofrece al usuario conservar la alternativa mediante **copia/renombrado** antes de confirmar la resolución, sin sobrescritura silenciosa.
- **Regla operativa de conflicto en v1:** la resolución no se ejecuta automáticamente sin confirmación del usuario. El flujo debe ofrecer opciones como aceptar la versión más reciente, conservar la alternativa como copia en conflicto o renombrarla para revisión manual. El usuario es quien confirma la decisión final del conflicto.
- **Evolución incremental sin reescritura:** las abstracciones clave (`StorageProvider`, `TransportAdapter`, `ConflictResolutionPolicy`, `IdentityProvider`) se definen desde v1 aunque solo tengan una implementación concreta.
- **Portabilidad:** comportamiento consistente entre desktop (Windows/macOS/Linux) y Android.

### 2.2 Matriz de verdad (12 acuerdos cerrados)

| # | Acuerdo | Decisión final | Estado |
|---|---------|----------------|--------|
| 1 | MVP | Solo auth + sync básico | ✅ Aprobado |
| 2 | Login | Solo cuentas precreadas (sin registro público) | ✅ Aprobado |
| 3 | Operaciones | subir, bajar, borrar, renombrar, mover, copiar | ✅ Aprobado |
| 4 | Sincronización | Bidireccional cliente ↔ servidor | ✅ Aprobado |
| 5 | Offline | No obligatorio en MVP | ✅ Aprobado |
| 6 | Conflictos | Archivo más nuevo gana por defecto, con confirmación del usuario y opción de conservar copia/renombrado | ✅ Aprobado |
| 7 | Versionado | Fuera de MVP | ✅ Aprobado |
| 8 | Compartición | Fuera de MVP | ✅ Aprobado |
| 9 | Plataformas MVP | Desktop + Android | ✅ Aprobado |
| 10 | Encriptación v1 | No obligatoria; E2E diseñado para evolución post-MVP | ✅ Aprobado |
| 11 | Almacenamiento v1 | NAS no obligatorio; `StorageProvider` = `LocalDiskStorageProvider` | ✅ Aprobado |
| 12 | Escala/rendimiento altos | Post-MVP | ✅ Aprobado |

### 2.3 Seguridad mínima v1

Los siguientes controles son **obligatorios desde v1**:

- **TLS 1.3** en todo tráfico cliente–servidor.
- **Autenticación por sesión** con token y expiración controlada.
- **Control básico de acceso:** un usuario solo opera sobre sus propios archivos.
- **Integridad por checksum** en toda operación de subida/bajada.

El cifrado E2E **no es obligatorio en v1**, pero el esquema de metadatos separa desde el inicio los campos operacionales (visibles en servidor) de los campos candidatos a cifrado posterior (`display_name`, `full_path`). Esta separación evita una reescritura de base de datos en v2.

> Los metadatos pueden estar expuestos en v1. A partir de v2 esta condición no es aceptable.

### 2.4 Observabilidad mínima v1

**Eventos auditables obligatorios:**

| Evento | Datos capturados |
|--------|-----------------|
| `auth.login_success` | `user_id`, `device_id`, `ip`, `timestamp` |
| `auth.login_failed` | `user_id`, `ip`, `timestamp`, motivo |
| `sync.file_uploaded` | `user_id`, `device_id`, `path_hash`, `size_bytes`, `checksum`, `timestamp` |
| `sync.file_downloaded` | `user_id`, `device_id`, `path_hash`, `timestamp` |
| `sync.conflict_detected` | `user_id`, `path_hash`, `device_local`, `device_remote`, `strategy_applied`, `timestamp` |

**Métricas básicas:**

| Métrica | Tipo | Referencia informal |
|---------|------|---------------------|
| `sync_operation_duration_ms` | Histograma p50/p95 | p95 < 30 s para archivos < 10 MB |
| `sync_failed_operations_total` | Contador | Valor creciente sostenido > 5 min = investigar |
| `active_sessions_count` | Gauge | Valor anómalo = posible sesión colgada |

**Formato de log:** JSON estructurado (NDJSON), una línea por evento. Campos mínimos: `ts`, `level`, `event`, `user_id`, `device_id`, `message`. Rotación diaria, retención 30 días. Niveles en producción: `INFO`, `WARN`, `ERROR`.

**SLO formal:** no aplica en v1. Meta operativa informal: el 95% de las operaciones de sync completan en menos de 30 s para archivos menores a 10 MB en red doméstica.

### 2.5 Puntos de variación y evolución post-MVP

Ningún módulo del dominio de sync llama directamente a implementaciones concretas; siempre a través de la abstracción:

| Abstracción | Implementación v1 | Implementación futura |
|-------------|-------------------|-----------------------|
| `StorageProvider` | `LocalDiskStorageProvider` | `NasStorageProvider`, `S3StorageProvider` |
| `TransportAdapter` | REST sobre TLS 1.3 | WebSocket (sync real-time), gRPC |
| `ConflictResolutionPolicy` | `LastWriteWinsWithConflictCopy` | `ThreeWayMerge`, merge asistido |
| `IdentityProvider` | Usuario/contraseña + token de sesión | OAuth2/OIDC, biometría Android |

**Señales que disparan el paso a Fase 2:**

1. **E2E activo:** el usuario almacena contenido sensible y pregunta si está cifrado en el servidor.
2. **Versionado activo:** ocurre el primer conflicto que una copia de conflicto no resuelve y se necesita restaurar una versión anterior.
3. **NAS + escala activos:** `LocalDiskStorageProvider` supera el 75% de capacidad, o se incorpora un tercer dispositivo activo.

## 3. Componentes Principales

### 3.1 Vista general y límites del MVP
- La arquitectura del MVP se organiza en componentes cliente/servidor con responsabilidades explícitas.
- Alcance funcional de componentes en v1: autenticación, sincronización básica y resolución de conflictos por **copia en conflicto**.
- Quedan fuera del núcleo v1: versionado completo, compartición de archivos y modo offline obligatorio.
- La arquitectura se diseña para evolución incremental sin reescritura.

### 3.2 Componente Cliente (Desktop + Android)
- Los clientes se implementan por separado para **desktop** y **Android**, compartiendo contrato funcional de sincronización.
- Estructura interna del cliente:
  - **Núcleo modular + adaptadores** (sistema de archivos, red, persistencia local).
  - Motor de sincronización bidireccional.
  - Manejador de conflictos (regla universal: copia en conflicto).
  - Módulo de sesión/autenticación para cuentas precreadas.
- El cliente mantiene **metadatos locales mínimos** para continuidad de sincronización y reintentos.

### 3.3 Componente Servidor (Monolito modular)
- El backend se implementa como **monolito modular** con 8 módulos activos:
  - `Identity`
  - `Sync Orchestrator`
  - `Metadata Catalog`
  - `Conflict Resolution`
  - `Versioning` (stub/interfaz)
  - `Storage Gateway`
  - `Background Jobs` (modo mínimo)
  - `Audit`
- El servidor expone endpoints con topología **híbrida**:
  - endpoints de dominio (auth, metadatos, operaciones),
  - fachada de sincronización para flujo cliente.
- `Versioning` no está activo funcionalmente en MVP; queda preparado para fase posterior.

### 3.4 Datos y metadatos
- Persistencia de metadatos en **SQLite v1**, con esquema compatible para migración a PostgreSQL.
- Identificación de ruta por `path_hash` de **ruta relativa normalizada** usando **SHA-256**.
- Modelo híbrido para estado de sincronización:
  - registro canónico por archivo,
  - estado mínimo por `device_id` para control de convergencia y trazabilidad.
- El servidor mantiene la fuente de verdad de metadatos; el cliente conserva copia mínima operativa.

### 3.5 Almacenamiento de archivos
- La capa de almacenamiento se abstrae mediante `StorageProvider`.
- Implementación inicial: `LocalDiskStorageProvider` (NAS no obligatorio en v1).
- Layout de almacenamiento **híbrido**, equilibrando simplicidad de operación y evolución futura.
- La abstracción permite migrar a NAS/post-MVP sin romper contratos del núcleo de sincronización.

### 3.6 Comunicación e interfaces entre componentes
- Protocolo cliente-servidor en v1: **REST/JSON sobre HTTP/1.1 + TLS**.
- Versionado de API: prefijo **`/v1`**.
- Detección de cambios remotos del cliente: **polling** (sin push en v1).
- Todas las operaciones remotas se diseñan como idempotentes para reintentos seguros.
- Kafka no se incluye en v1; se considera post-MVP bajo criterios de carga y desacoplamiento.

### 3.7 Ejecución en segundo plano y evolución
- `Background Jobs` en modo mínimo interno para:
  - drenaje de cola persistente,
  - reintentos con backoff,
  - limpieza operativa básica.
- En cliente, la cola de operaciones es **persistente en disco** para recuperación ante reinicios/fallos.
- Señales de evolución post-MVP:
  - activación de versionado real,
  - incorporación de NAS obligatorio,
  - introducción de mensajería distribuida (Kafka),
  - refuerzo de seguridad de metadatos y cifrado obligatorio.

## 4. Flujo General de Sincronización

### 4.1 Alcance, actores y precondiciones del flujo
- El flujo de esta sección aplica al MVP: autenticación y sincronización básica bidireccional.
- Actores: cliente desktop, cliente Android y servidor de sincronización.
- Precondición obligatoria: sesión válida de cuenta precreada.
- Quedan fuera en v1: versionado funcional, compartición, modo offline obligatorio, NAS obligatorio y cifrado E2E obligatorio.

### 4.2 Detección de cambios y encolado local persistente
- Disparadores de sincronización:
  - cambios locales de archivos,
  - polling remoto periódico,
  - acción manual “Sincronizar ahora”.
- El cliente aplica debounce por archivo (~1 segundo) para reducir ráfagas de eventos.
- Cada cambio se transforma en una operación de sync y se persiste en cola local en disco antes de enviarse.
- Cada item de la cola incluye `idempotency_key` y se registra con estado de `queued`/`in_flight`/`retry` para permitir reintentos seguros y evitar duplicados.
- La cola persistente permite recuperación tras cierre inesperado o reinicio de la app/dispositivo.

### 4.3 Canal saliente (cliente → servidor): orden, atomicidad e idempotencia
- Transporte oficial v1: REST/JSON sobre HTTP/1.1 con TLS, bajo endpoints `/v1`.
- Orden canónico de aplicación: secuencia de servidor (`server_seq`) para garantizar consistencia reproducible.
- Atomicidad en v1: por archivo (no por lote completo).
- Idempotencia: cada operación incluye `idempotency_key` persistida y deduplicación de 24 horas en servidor.
- Prioridad operativa: primero pull remoto para actualizar base local; luego push de cola local con rebase cuando aplique.

### 4.4 Canal entrante (servidor → cliente): polling y aplicación de deltas
- El cliente consulta cambios remotos por polling fijo cada 30 segundos.
- El servidor responde deltas pendientes por usuario/dispositivo.
- El cliente aplica deltas en orden consistente con metadatos canónicos.
- En v1 no se usa push en tiempo real (WebSocket/SSE).

### 4.5 Resolución de conflictos y convergencia
- Condición principal de conflicto: ambos lados cambiaron desde `last_sync` y el contenido final difiere por hash.
- Un cambio no conflictivo es aquel en el que solo un lado modificó el archivo, o la ruta cambió y el contenido final sigue siendo idéntico. En esos casos no se bloquea la sincronización ni se crea una copia de conflicto.
- Regla de convergencia en v1: por defecto, el archivo con `modified_at` más reciente gana, con tolerancia de ±2–5 s para compensar deriva temporal entre dispositivos. Si el usuario confirma conservar la alternativa, se genera una **copia en conflicto** o se renombra la versión alternativa para revisión manual; nunca hay sobrescritura silenciosa.
- Política `delete` vs `modify` concurrente:
  - `delete` define el estado canónico cuando el cambio de borrado es inequívoco,
  - `modify` se preserva como copia en conflicto si el usuario decide conservar la versión alternativa.
- Reglas de identificación por ruta y contenido:
  - si el mismo archivo cambia en dos rutas equivalentes o una ruta se renombra mientras el contenido difiere, se trata como conflicto de contenido, no como “cambio inocuo”;
  - si la ruta cambia y el contenido es idéntico, se considera un caso de renombrado o copia, no un conflicto.
- Normalización temporal: `modified_at` en UTC epoch ms con tolerancia de deriva (±2–5 s). Cuando la diferencia temporal no es concluyente, se puede revisar el hash como diagnóstico, pero la regla base de resolución sigue siendo `modified_at`.
- Convención de nombre de conflicto: sufijo legible en el mismo directorio (incluyendo dispositivo y timestamp).

### 4.6 Errores, reintentos y recuperación
- Clasificación de errores:
  - reintentables: red, timeout, 408, 429, 5xx;
  - terminales: 400, 401, 403, 404, 422.
- Reintentos: backoff exponencial con jitter y límite de intentos.
- Recuperación tras reinicio: estrategia híbrida por antigüedad de cola:
  - cola reciente: drenado directo;
  - cola antigua: reconciliación previa y luego drenado.
- Estado de sincronización degradada: umbral por tamaño de cola + antigüedad del ítem más antiguo.

### 4.7 Observabilidad del flujo y visibilidad al usuario
- Correlación mínima por operación (nivel balanceado):
  - `op_id`, `request_id`, `sync_session_id`, `queue_job_id`, `attempt`, `user_id`, `device_id`, `server_trace_id`, `result`.
- Eventos obligatorios:
  - hitos del flujo (inicio/fin, enqueue, upload/download, conflicto, fallo),
  - transiciones de estado de cola y respuestas API.
- Métricas mínimas del pipeline:
  - latencia de sync (p50/p95),
  - tasa de éxito/error,
  - profundidad de cola,
  - tasa de reintentos,
  - tasa de conflictos.
- Estado visible en UI (global + detalle breve):
  - Sincronizado, Sincronizando, Con conflictos, Error, Pausado;
  - última sincronización, pendientes, botón reintentar y resumen de conflictos/errores.

## 5. Seguridad y Modelo de Confianza (MVP)

### 5.1 Alcance, supuestos y límites de confianza
- Esta sección define la seguridad del MVP de SyncFiles para autenticación y sincronización básica en **desktop** y **Android**.
- El modelo de confianza es cliente-servidor, con el servidor como punto principal de control para identidad, autorización y estado crítico de sincronización.
- En v1, la **encriptación end-to-end no es obligatoria**, por lo que el servidor puede acceder a metadatos operativos necesarios para gestionar el proceso de sync.
- El modelo de confianza no depende de un backend NAS específico; el almacenamiento puede evolucionar sin alterar estas reglas.
- El canal oficial de comunicación es **REST/JSON sobre HTTP/1.1 + TLS**, con consulta remota por polling.

### 5.2 Autenticación y gestión de sesión
- La autenticación del MVP se basa en **cuentas precreadas** y emisión de **token opaco** con sesión mantenida en servidor.
- La sesión tiene una validez base de **24 horas**, con **renovación deslizante** mientras exista actividad válida.
- Toda sesión expira de forma absoluta a los **7 días**, incluso si sigue activa.
- Se permite un máximo de **3 sesiones concurrentes por usuario**.
- El token no contiene estado autoritativo; la fuente real de verdad de la sesión vive exclusivamente en servidor.

### 5.3 Autorización y servidor como fuente de verdad
- La autorización se define con **permisos granulares por acción y recurso** para las operaciones del MVP:
  - subir,
  - bajar,
  - borrar,
  - renombrar,
  - mover,
  - copiar.
- El servidor valida siempre identidad, permisos y estado crítico antes de ejecutar cualquier operación.
- El cliente no es fuente de verdad para autorización, ownership, estado de sincronización ni resolución de conflictos.
- Cualquier caché local de permisos o estado se considera una optimización, nunca una autoridad.
- Este enfoque sigue el principio de **mínimo privilegio** y evita confiar en decisiones críticas tomadas por el cliente.

### 5.4 Vinculación de sesión a dispositivo y validación por request
- Cada sesión queda asociada a un único **`device_id`** desde el momento del login.
- En cada request, el servidor valida la correspondencia entre:
  - sesión,
  - usuario autenticado,
  - `device_id`.
- Si un token válido se presenta desde un dispositivo distinto al asociado, la operación se rechaza.
- Esta validación aplica a endpoints de autenticación, metadatos y sincronización.
- El objetivo es reforzar trazabilidad y limitar abuso o reutilización indebida de sesión.

### 5.5 Seguridad del canal y autenticidad del servidor
- Todo el tráfico entre cliente y servidor utiliza **TLS** de forma obligatoria.
- La autenticidad del servidor se valida usando la **cadena de confianza del sistema operativo**.
- En v1 no se exige **certificate pinning**.
- Ante errores de validación TLS, cadena inválida o conexión degradada, el cliente debe **fallar en cerrado**.
- La seguridad del canal protege transporte y autenticidad del endpoint, pero no sustituye la futura incorporación de cifrado E2E.

### 5.6 Integridad de archivos y manejo de discrepancias
- La verificación de integridad usa **`SHA-256` + `size_bytes`** como base mínima.
- En operaciones de subida, el servidor valida integridad antes de aceptar y confirmar el archivo.
- En operaciones de descarga, el cliente valida integridad antes de hacer commit local.
- Ante una discrepancia de integridad:
  - se realizan **1–2 reintentos automáticos**,
  - si el problema persiste, el archivo u operación pasa a **cuarentena**.
- Un incidente aislado de integridad no bloquea la sesión completa del usuario.

### 5.7 Protección local de secretos y persistencia segura
- Los tokens y secretos locales deben almacenarse en el **almacén seguro nativo** de cada plataforma.
- En desktop y Android se priorizan mecanismos nativos de protección, evitando cofres propios implementados por la aplicación.
- Los secretos no deben persistirse en texto plano en logs, colas, archivos temporales ni almacenamiento general.
- Esta protección cubre credenciales y material de sesión, no implica cifrado E2E del contenido de archivos en v1.
- La estrategia queda preparada para endurecimiento futuro sin rediseñar la arquitectura base.

### 5.8 Auditoría, retención y alertas operativas de seguridad
- La información operativa y de seguridad se clasifica por niveles con retención diferenciada:
  - **logs operativos:** 30 días,
  - **auditoría de seguridad:** 90 días,
  - **métricas agregadas:** 180 días.
- La auditoría mínima debe cubrir autenticación, validación de sesión y eventos relevantes de seguridad.
- Se generan alertas automáticas con reglas simples ante:
  - múltiples logins fallidos,
  - uso de token expirado o revocado,
  - login desde dispositivo nuevo,
  - borrados o renombres masivos en poco tiempo.
- Las alertas del MVP son operativas y orientadas a diagnóstico, no un sistema avanzado de detección de amenazas.
- Los eventos de seguridad deben incluir trazabilidad consistente (`user_id`, `device_id`, `ip`, timestamp, resultado y contexto).

## 6. Escalabilidad y Disponibilidad

### 6.1 Modelo operativo de v1 (single-node explícito)
La arquitectura de v1 es **single-node** (decisión A1-A): una sola instancia de backend, una base SQLite y almacenamiento local mediante `StorageProvider` actual.
Se aceptan **SPOF** (decisión A2-B) para priorizar simplicidad de implementación y operación en MVP.

**Mitigación obligatoria de pérdida de datos en v1:**
- backups programados de metadatos y archivos,
- verificación periódica de restauración,
- bitácora operativa de incidentes y recuperación.

No se introduce HA avanzada, clúster activo-activo ni componentes distribuidos en v1.

### 6.2 Capacidad objetivo del MVP
Capacidad de diseño cerrada para v1:
- **10 usuarios activos** y **20 dispositivos activos** (decisión B1-B).
- Hasta **250 MB por archivo** y **20 GB por usuario** (decisión B2-B).
- Nota de evolución: **chunking** de archivos grandes se mantiene como mejora futura (post-v1).

### 6.3 Disponibilidad y ventana de mantenimiento
Se permite **downtime planificado** (decisión A3-A), con aviso previo y ventana de mantenimiento definida.
El objetivo operativo de v1 es continuidad razonable, no alta disponibilidad formal con failover automático.

### 6.4 Control de carga, umbrales y concurrencia
Para protección del servicio se adoptan umbrales **balanceados** para transición a Fase 2 (decisión B3-B):
- CPU > 75% por 15 min, RAM > 80%, disco > 75%, p95 sync > 30 s para archivos < 10 MB, cola > 300 ops o ítem > 15 min, errores > 5%.

La concurrencia es **flexible** (decisión B4-C):
- límite por operación según recurso disponible,
- degradación controlada bajo carga,
- sin bloqueo rígido global.

### 6.5 Manejo de saturación y resiliencia de sincronización
Control de presión híbrido (decisión C1 A+B):
- respuesta **429** cuando aplica,
- cabecera `Retry-After`,
- cliente con **polling adaptativo** para reducir presión en picos.

Resiliencia de estado híbrida (decisión C2 A+B):
- persistencia local en **SQLite**,
- **reconciliación al arranque** para recuperar consistencia tras reinicio o caída.

### 6.6 Recuperación y prioridad de trabajo
Estrategia de recuperación (decisión C3-B):
- restauración **semi-manual** mediante scripts + checklist operativa,
- sin orquestación automática compleja en v1.

Priorización de ejecución (decisión C4-B):
- política balanceada por cuotas entre lecturas y subidas,
- evita hambruna de un tipo de operación.

### 6.7 Ruta de evolución (Fase 1 → Fase 2)
Evolución definida en secuencia **A → B → C** (decisión D1):
1. **A:** estabilización y endurecimiento operativo de v1 single-node.
2. **B:** optimizaciones de capacidad y confiabilidad dentro del modelo simple.
3. **C:** escalado estructural post-MVP (solo tras evidencia de carga real).

La **v2** se activa únicamente tras testing aprobado de v1 (decisión D2).

### 6.8 Compatibilidad y criterio de cambio
La compatibilidad estricta hacia atrás **no es prioritaria** en esta etapa (decisión D3), pero todo cambio debe:
- documentarse,
- incluir plan de migración razonable,
- minimizar impacto operativo en clientes activos.

## 7. Operación y Observabilidad

### 7.1 Alcance operativo de v1
La observabilidad del MVP cubre instrumentación suficiente para operar, diagnosticar y mantener el sistema de sincronización en un entorno **single-node** con operación asistida. No se asume monitoreo 24/7 ni NOC formal.

Quedan fuera de v1:
- dashboards avanzados externos (Grafana, etc.),
- alertado por score compuesto o correlación compleja,
- tracing distribuido con exportador externo (OpenTelemetry exportado),
- SLO/SLA formal.

### 7.2 Señales mínimas obligatorias

**Eventos auditables** (heredados de §2.4 + hitos de flujo de §4.7):

| Evento | Origen | Datos mínimos |
|--------|--------|---------------|
| `auth.login_success` | servidor | `user_id`, `device_id`, `ip`, `timestamp` |
| `auth.login_failed` | servidor | `user_id`, `ip`, `timestamp`, motivo |
| `sync.file_uploaded` | servidor | `user_id`, `device_id`, `path_hash`, `size_bytes`, `checksum`, `timestamp` |
| `sync.file_downloaded` | servidor | `user_id`, `device_id`, `path_hash`, `timestamp` |
| `sync.conflict_detected` | servidor | `user_id`, `path_hash`, `device_local`, `device_remote`, `strategy_applied`, `timestamp` |
| `sync.enqueue` | cliente | `op_id`, `device_id`, `operation_type`, `timestamp` |
| `sync.retry` | cliente/servidor | `op_id`, `attempt`, `error_code`, `timestamp` |
| `sync.start` / `sync.end` | cliente | `sync_session_id`, `device_id`, `timestamp`, `result` |
| `queue.state_change` | cliente | `queue_depth`, `oldest_item_age_ms`, `timestamp` |

**Métricas operativas:**

| Métrica | Tipo | Referencia informal |
|---------|------|---------------------|
| `sync_operation_duration_ms` | Histograma p50/p95 | p95 < 30 s para archivos < 10 MB |
| `sync_failed_operations_total` | Contador | Valor creciente > 5 min → investigar |
| `active_sessions_count` | Gauge | Valor anómalo → posible sesión colgada |
| `queue_depth` | Gauge | > 300 ops → señal de saturación |
| `retry_rate` | Contador | Tasa alta sostenida → degradación |
| `conflict_rate` | Contador | Picos → revisar cambios concurrentes |

**Formato de log:** NDJSON estructurado, una línea por evento.

Esquema mínimo obligatorio por línea:


Niveles en producción: `INFO`, `WARN`, `ERROR`. Rotación diaria.

**Correlación por operación:** toda operación de sync propaga `request_id` + `op_id` + `device_id` en logs y métricas.

**SLO formal:** no aplica en v1. Meta operativa informal: 95 % de operaciones completan en menos de 30 s para archivos < 10 MB en red doméstica.

### 7.3 Alertas operativas de v1

Alertado automático básico disparado por umbrales definidos en §6.4:

| Señal | Umbral | Acción sugerida |
|-------|--------|-----------------|
| CPU | > 75 % por 15 min | Revisar colas y operaciones activas |
| RAM | > 80 % | Reiniciar proceso si necesario |
| Disco | > 75 % de capacidad | Liberar espacio / activar alerta de Fase 2 |
| p95 sync | > 30 s para < 10 MB | Revisar colas, red y reintentos |
| Cola | > 300 ops o ítem > 15 min | Revisar estado degradado |
| Tasa de error | > 5 % | Investigar logs de error |

Adicionalmente, las alertas de seguridad mínimas de §5.8 se mantienen obligatorias:
- múltiples logins fallidos,
- uso de token expirado o revocado,
- login desde dispositivo nuevo,
- borrados o renombres masivos en poco tiempo.

### 7.4 Taxonomía de errores, reintentos y soporte

**Taxonomía de errores por dominio + resultado:**

| Dominio | Reintentable | Terminal |
|---------|-------------|---------|
| red | timeout, desconexión, 408, 429, 5xx | — |
| auth | — | 401, 403 |
| sync | 5xx transitorio | 404, 422 |
| conflicto | — | requiere intervención manual |
| storage | error I/O transitorio | corrupción de datos |

**Política de reintentos por clase de operación** (backoff exponencial + jitter, §4.6):

| Clase | Intentos máx. | Backoff base |
|-------|--------------|--------------|
| upload | 5 | 2 s |
| download | 5 | 2 s |
| metadatos | 3 | 1 s |
| auth | 2 | 5 s |

**Runbook de incidente por severidad:**

- **SEV1** (servicio caído, datos en riesgo): contener → comunicar → recuperar → post-mortem en ≤ 24 h.
- **SEV2** (degradación perceptible, colas largas): diagnosticar → mitigar → documentar en ≤ 4 h.
- **SEV3** (errores aislados, sin impacto general): registrar → priorizar → resolver en próximo ciclo operativo.

**Visibilidad en UI:** estado base (§4.7) + motivo corto del error, último intento, próxima acción sugerida.

### 7.5 Rutina operativa y retención

**Checklist diario/semanal mínimo:**
- Verificar alertas activas.
- Revisar métricas de cola, errores y conflictos.
- Confirmar estado de backups y restauración.
- Revisar capacidad de disco y CPU.
- Documentar incidentes en bitácora operativa.

**Retención diferenciada (híbrida: caliente + archivo comprimido):**

| Tipo | Retención activa | Archivo comprimido |
|------|-----------------|-------------------|
| Logs operativos | 30 días | — |
| Auditoría de seguridad | 90 días | configurable |
| Métricas agregadas | 180 días | configurable |

### 7.6 Validación continua y control de cambios

**Validación de telemetría en runtime:** los eventos se validan contra el esquema §2.4 en el momento de emisión; ningún evento malformado llega a los logs.

**Pruebas de resiliencia operativa:** fallos inyectados en tests automatizados (red, disco, auth) para validar reintentos, backoff y recuperación de cola sin afectar producción.

**Pipeline de cambios:** sin gate automático específico de observabilidad en CI. Todo cambio operativo (umbrales, retención, runbooks) se introduce mediante **redeploy controlado con rollback documentado** versionado en git.

**Post-MVP:** activación de tracing distribuido con exportador externo, gates automáticos de observabilidad en CI, y dashboards formales.

## 8. Decisiones Gobernadas y Evolución Post-MVP

### 8.1 Introducción: Señales Definidas, Decisiones Estructuradas

La Sección 8 documenta las **decisiones gobernadas que rigen la evolución del sistema post-MVP**, basadas en **señales operativas reales** derivadas de secciones 1–7. Esta gobernanza **no es el diseño final de Fase 2**, sino el **mecanismo de transición controlada** desde v1 hacia mejoras incrementales.

Ninguna de estas decisiones es especulativa: cada una se vincula a una señal observable del sistema que, cuando se manifieste, **activa automáticamente el análisis y la decisión correlativa**. El objetivo es **evitar arquitectura prematura**, permitir aprendizaje empírico en v1 y validar hipótesis con datos reales antes de comprometerse con cambios estructurales.

**Alcance de esta sección:**
- Decisiones cerradas para MVP (confirmadas en Sección 2) que impactan la evolución.
- Criterios de transición de Fase 1 → Fase 2 basados en señales observables.
- Bloque de gobierno con propietarios de decisión, cronograma y criterios de aceptación.
- Matriz de decisiones con relaciones entre dominios y condiciones de disparo.

**Fuera de alcance:**
- Especificación detallada de Fase 2 (se define tras aprobación de Go/No-Go).
- Cambios reactivos sin señal o evidencia operativa.
- Rediseño arquitectónico sin validación previa en v1.

---

### 8.2 Bloque A: Seguridad y Confianza (Decisiones Confirmadas)

#### 8.2.1 Cifrado End-to-End en v1: Diseño Post-MVP (A1-A)

**Decisión confirmada:** E2E **no es obligatorio en v1**. El servidor puede acceder a metadatos operacionales necesarios para gestionar sincronización básica.

**Justificación:** Implementación de E2E introduce complejidad no lineal: cambios en generación de claves, reencriptación de transporte, gestión de identidad de clientes, revocación segura y manejo de recuperación ante pérdida de claves. Todo esto es validable únicamente tras operación estable de v1 sin cifrado.

**Señal de disparo:** Usuario accede al servidor y pregunta explícitamente si sus archivos están cifrados en reposo. Esta conversación indica madurez operacional y demanda creciente de privacidad.

**Alcance de diseño E2E (a realizar post-MVP):**
- Separación de campos operacionales (hash de ruta, timestamps, checksums) de campos cifrados (display_name, contenido).
- Protocolo de negociación de claves: cliente genera par asimétrico, servidor mantiene clave pública certificada.
- Material de sesión: derivación de clave de simetría por usuario/dispositivo usando KDF.
- Revalidación de integridad: checksum calculado en cliente antes de cifrado; servidor valida checksum cifrado mediante firma.
- Migración de datos: reencriptación on-demand por archivo al primer acceso o batch nightly (configurables).
- Recuperación ante pérdida de clave: protocolo de "recuperación de cuenta" basado en código de recuperación multi-dispositivo.

**Propietario:** Arquitecto de Seguridad. **Revisor:** CTO. **Evaluación:** Semana 2 de Fase 2 (si es activada).

---

#### 8.2.2 Gestión de Claves y Material Criptográfico (A2-B)

**Decisión confirmada:** Gestión de claves queda **fuera de v1**; se diseña en transición a Fase 2 con **aceptación explícita de riesgo operacional**.

**Riesgos mitigados en v1:**
- Secretos en texto plano en almacenamiento local: mitigación mediante almacén seguro del sistema operativo.
- Exposición de material en tránsito: TLS 1.3 obligatorio con validación de cadena de confianza.
- Material heredado de sesión: expiración absoluta a 7 días, máximo 3 sesiones concurrentes, vinculación estricta a dispositivo.

**Alcance post-MVP de gestión de claves:**
- Repositorio de claves centralizado (servidor) con permiso granular por claves de usuario.
- Rotación de claves de sesión: cada 24 horas con renovación automática durante actividad.
- Rotación de claves de cifrado de archivo: no rotación automática; solo bajo demanda del usuario (complejidad alta).
- Protocolo de generación de claves: **PBKDF2** para derivación de clave primaria a partir de contraseña + salt; luego **HKDF** para derivar subclaves por contexto.
- Almacenamiento seguro: servidor en HSM (Hardware Security Module) o caja fuerte criptográfica nativa; cliente en Keychain/Keystore.
- Revocación: lista de revocación en tiempo real, validada en cada operación; TTL de caché local = 5 minutos.

**Propietario:** Ingeniero de Seguridad. **Revisor:** CTO, Auditor Externo. **Evaluación:** Semana 1 de Fase 2 (pre-arquitectura E2E).

---

#### 8.2.3 Migración de Datos Operacional (A3-B)

**Decisión confirmada:** Migración de archivos entre proveedores de almacenamiento (**LocalDiskStorageProvider** → **NasStorageProvider**) ocurre **fuera de v1**, con protocolo de doble escritura (dual-write) y backfill por lotes.

**Estrategia de migración post-MVP:**
- **Fase 0 (Preparación):** ambos proveedores están implementados; servidor activo solo usa LocalDisk.
- **Fase 1 (Dual-write):** nuevas escrituras van a ambos proveedores en paralelo; fallos en NAS no bloquean respuesta (fire-and-forget con reintentos en background).
- **Fase 2 (Backfill):** job batch copia datos históricos desde LocalDisk a NAS; sincronización de metadatos.
- **Fase 3 (Switchover):** tráfico de lectura se redirige a NAS con fallback a LocalDisk durante 24 horas.
- **Fase 4 (Sunsetting):** comprobación de integridad final; archivado de LocalDisk; purga después de 30 días de retención.

**Validación de integridad:** checksum SHA-256 se recalcula en cada copia; inconsistencias se registran como evento de auditoría sin bloquear migración.

**Rollback:** si se detecta corrupción en NAS, switchback a LocalDisk es manual pero asistido por script; cliente revalida checksum automáticamente.

**Propietario:** Ingeniero de Infraestructura. **Revisor:** CTO, Operaciones. **Evaluación:** Semana 3 de Fase 2 (post-pruebas de carga).

---

#### 8.2.4 Estrategia de Revalidación de Confianza Post-Migración (A4-B)

**Decisión confirmada:** Tras cualquier migración de datos o cambio de almacenamiento, **revalidación obligatoria de checksums** para todos los archivos antes de marcar como "seguro" el nuevo estado.

**Protocolo de revalidación:**
1. **Trigger:** cambio de proveedor de almacenamiento activado.
2. **Auditoría inicial:** servidor compara checksum de archivo en almacenamiento antiguo vs. nuevo.
3. **Reparación selectiva:** archivos con discrepancia son recopiados; si persisten, pasan a cuarentena.
4. **Confirmación al cliente:** cliente recibe notificación de éxito o necesidad de re-sincronización.
5. **Retención:** evento de auditoría permanente del resultado final.

**Criterio de aceptación:** 100 % de integridad validada o no se acepta el nuevo almacenamiento.

**Propietario:** Ingeniero de Base de Datos. **Revisor:** CTO, Auditor. **Evaluación:** Antes de Switchover (Fase 3 de migración).

---

### 8.3 Bloque B: Plataforma y Almacenamiento (Decisiones Confirmadas)

#### 8.3.1 Arquitectura Single-Node Obligatoria en v1 (B1-A)

**Decisión confirmada:** El backend de v1 es **single-node**: una sola instancia de proceso, una base SQLite y almacenamiento local mediante `LocalDiskStorageProvider`.

**SPOF (Single Point of Failure) aceptado:** fallos de servidor, disco o proceso causan downtime. No se incluye redundancia activa-activa ni failover automático.

**Mitigación de pérdida de datos obligatoria:**
- Backups diarios de metadatos (tablas SQLite) a almacenamiento externo comprimido en gzip.
- Backups semanales de archivos completos (o incremental si volumen > 100 GB).
- Verificación de restauración: un backup aleatorio semanal se restaura en sandbox y se valida integridad.
- Bitácora operativa de todos los backups, restauraciones y verificaciones.
- Retención: mínimo 4 semanas de backups; máximo 12 semanas.

**Implicación operativa:** downtime planificado es aceptable; downtime no planificado requiere intervención manual y restauración controlada.

**Propietario:** Lead de Operaciones. **Revisor:** CTO. **Evaluación:** Antes de producción v1 (Sprint 0).

---

#### 8.3.2 Transición de Persistencia: SQLite → PostgreSQL Post-MVP (B2-A)

**Decisión confirmada:** v1 usa **SQLite** (simpleza, cero administración). Transición a **PostgreSQL** ocurre **post-MVP** cuando carga o requisitos de concurrencia lo justifiquen.

**Esquema de migración:**
1. **Compatibilidad desde v1:** esquema SQLite se diseña con compatibilidad futura a PostgreSQL; tipos de datos genéricos, sin SQLite-isms.
2. **Dual-write opcional en Fase 2:** si se activa, servidor puede escribir a PostgreSQL en paralelo mientras sigue leyendo de SQLite.
3. **Switchover:** una vez que PostgreSQL contiene datos históricos y pasa pruebas de carga, se redirige tráfico de lectura con rollback manual disponible.
4. **Validación:** todos los schémas migreados se validan con script de auditoría; diferencias de conteo o checksum se resuelven antes de sunsetting de SQLite.

**Criterio de disparo:** carga de v1 supera 10 GB de metadatos O 5+ escrituras simultáneas sostenidas O latencia de query > 1 s en p95.

**Propietario:** Ingeniero de Base de Datos. **Revisor:** CTO. **Evaluación:** Semana 2 de Fase 2 (post-validación de carga).

---

#### 8.3.3 Evolución de Almacenamiento: StorageProvider → NAS Post-MVP (B3-A)

**Decisión confirmada:** v1 usa `LocalDiskStorageProvider` (disco local). La evolución a `NasStorageProvider` ocurre post-MVP por señales de capacidad y uso compartido.

**Regla clave:** el trigger de NAS está **desacoplado** de la política de backup.

**Abstracción mantenida:** interfaces `StorageProvider` no cambian; el nuevo proveedor se incorpora sin romper contratos del núcleo de sincronización.

**Criterios de disparo para activación de NAS:**
- Capacidad de disco local > 75 % durante 2 semanas, O
- **tercer dispositivo con demanda compartida**, O
- degradación operativa sostenida vinculada a almacenamiento.

**Implementación de `NasStorageProvider`:**
- Protocolo primario: SFTP (sobre SSH); alternativa LAN: SMB3.
- Autenticación: credenciales de NAS solo en servidor (cofre seguro); cliente no accede directo al NAS.
- Aislamiento: mapeo por `user_id` en rutas separadas.
- Integridad: verificación de checksum en operaciones críticas.
- Fallback: si NAS no responde, escritura temporal en `LocalDiskStorageProvider` con reconciliación posterior controlada.

**Criterio de aceptación:**
- `NasStorageProvider` operativo en lectura/escritura.
- Integridad validada en migración (sin discrepancias sin resolver).
- Fallback y reconciliación probados.

**Propietario:** Ingeniero de Infraestructura. **Revisor:** CTO, Operaciones. **Evaluación:** Semana 3 de Fase 2.

---

#### 8.3.4 Backup y Restauración Post-MVP (B4-B)

**Decisión confirmada:** en v1 el backup es **automatizado** para metadatos y archivos. Fase 2 incorpora capacidades avanzadas de restauración y optimización operativa.

**Estrategia de backup v1 (obligatoria):**
- **Metadatos:** dump diario de SQLite, comprimido y almacenado fuera del nodo principal.
- **Archivos:** backup automatizado programado (incremental diario + verificación periódica).
- **Validación:** restore test semanal (metadatos + archivos) con control de integridad por checksum.

**Estrategia de mejora en Fase 2:**
- Restauración self-service para recuperación puntual desde UI.
- Políticas de retención y recuperación más granulares.
- Endurecimiento de operación de respaldo (auditoría ampliada y reportes).

**Objetivos operativos:**
- `RPO` <= 24 h
- `RTO` < 4 h para restauraciones de usuario

**Auditoría:** toda restauración queda registrada con trazabilidad completa.

**Propietario:** Ingeniero de Operaciones. **Revisor:** CTO, Seguridad. **Evaluación:** v1 + mejoras en Semana 4 de Fase 2.

---

### 8.4 Bloque C: Capacidades Fase 2 (Decisiones de Evolución)

#### 8.4.1 Versionado de Archivos: Alcance y Coexistencia (C1-A)

**Decisión confirmada:** **Versionado queda fuera de v1**. Se activa únicamente cuando usuario genera conflicto que copia de conflicto no resuelve.

**Señal de disparo:** Evento registrado en auditoría: usuario ha intentado restaurar versión anterior, o conflicto de 3+ partes simultáneamente no se resolvió con copia de conflicto.

**Alcance de versionado en Fase 2:**
- **Coexistencia:** copias de conflicto v1 se mantienen como primer mecanismo; versionado formal es segunda línea.
- **Retención:** máximo 10 versiones por archivo, retención por 90 días, después purga automática.
- **Deduplicación:** versiones con identical checksum se colapsan a una entrada con timestamp de referencia.
- **Modelo:** cambios por archivo (no a nivel de sector), con metadata de versión (author_device_id, timestamp, cambio resumido).
- **Acceso:** UI muestra "Historial" de versión; usuario puede pre-visualizar y restaurar sin confirmación adicional.
- **Cuota:** versiones no consumen cuota de almacenamiento del usuario (a cargo del operador) hasta que alcancen 10 % de uso total.

**Propietario:** Product Manager. **Revisor:** CTO, UX. **Evaluación:** Semana 1 de Fase 2 (post-análisis de incidentes v1).

---

#### 8.4.2 Compartición de Archivos: Alcance y Modelo de Permisos (C2-A)

**Decisión confirmada:** **Compartición queda fuera de v1**. Se activa cuando múltiples usuarios solicitan explícitamente capacidad de compartir.

**Señal de disparo:** 3+ usuarios en período de 2 semanas pregunta por "compartir archivo con otro usuario" o "directorio compartido".

**Alcance de compartición en Fase 2:**
- **Modelo:** compartición granular por archivo o directorio; permisos explícitos (lectura, escritura, lectura+escritura).
- **Scope:** inicialmente entre usuarios de la misma instancia (no federación multi-servidor).
- **Control de acceso:** servidor valida permisos; cliente respeta sin lógica adicional.
- **Sincronización:** archivos compartidos se comportan como si fueran propios del usuario, pero con conflictos resueltos por propietario original.
- **Trazabilidad:** auditoría de quién compartió qué, cuándo, con quién y cambios subsecuentes.
- **Revocación:** propietario puede revocar acceso; archivos descargados persisten en dispositivo del usuario con estado de "offline read-only".

**Propietario:** Product Manager. **Revisor:** CTO, Seguridad. **Evaluación:** Semana 2 de Fase 2.

---

#### 8.4.3 Evolución de Transporte: REST → WebSocket → gRPC (C3-A→B→C)

**Decisión confirmada:** Transporte en v1 es **REST/JSON sobre HTTP/1.1 + TLS** con **polling de 30 segundos**. Evolución ocurre en etapas basadas en carga real.

**Fase A (v1): REST + Polling**
- Protocolo: GET/POST/PUT/DELETE con idempotencia vía `idempotency_key`.
- Latencia de delta: hasta 30 segundos (polling sync). Aceptable para MVP.
- Escalabilidad: limitada por número de conexiones simultáneas en servidor; 10 usuarios = ~10 conexiones en pico.

**Trigger → Fase B (Fase 2): WebSocket + Push**
- Señal: p95 de latencia de sincronización > 15 segundos sostenido por 2 semanas, O 50+ dispositivos activos simultáneamente.
- Protocolo: WebSocket (RFC 6455) con fallback a polling si conexión no es estable.
- Implementación: servidor mantiene mapa de dispositivos activos; deltas se envían en tiempo real (< 500 ms).
- Ventaja: latencia mínima; desventaja: estado en servidor más complejo, requiere manager de conexión con heartbeat.
- Compatibilidad: clientes antiguos REST siguen funcionando via polling; sin cambio de API.

**Trigger → Fase C (Fase 3): gRPC + Streaming**
- Señal: 100+ dispositivos activos simultáneamente, O latencia de WebSocket inestable en redes móviles.
- Protocolo: gRPC con streaming bidireccional; serialización Protobuf en lugar de JSON.
- Implementación: cliente abre una conexión streaming que vive toda la sesión; servidor envía deltas de forma asíncrona.
- Ventaja: eficiencia de red, multiplexing nativo, compresión implícita.
- Desventaja: cambio de cliente (biblioteca gRPC), cambio de servidor (stack gRPC).
- Compatibilidad: REST sigue disponible para clientes legacy o mode offline.

**Propietario:** Ingeniero de Frontend + Backend. **Revisor:** CTO. **Evaluación:** Fase 2 (WebSocket si trigger) o Fase 3 (gRPC si trigger).

---

#### 8.4.4 Modo Offline Obligatorio en Fase 2 (C4-A)

**Decisión confirmada:** **Offline no es obligatorio en MVP**. Se activará en Fase 2 con alcance limitado si usuario solicita explícitamente acceso offline sostenido.

**Señal de disparo:** Usuario intenta sincronizar con red inestable por 3+ sesiones en 1 semana, O usuario relata "trabajo offline" como caso de uso crítico.

**Alcance de offline en Fase 2:**
- **Lectura:** usuario puede descargar archivos específicos para acceso offline; actualización manual o pre-carga configurada.
- **Escritura:** cambios locales se persisten en cola; sincronización ocurre cuando conectividad se recupera.
- **Conflictos:** si múltiples usuarios editan offline, conflictos se resuelven al sincronizar usando lógica de "último en ganar" + copia de conflicto.
- **Almacenamiento:** caché local limitada (configurable, default 1 GB); archivos antiguos se purguen con LRU.
- **Seguridad:** archivos offline permanecen cifrados localmente con clave derivada de sesión; se borran al logout.

**Propietario:** Ingeniero de UX + Backend. **Revisor:** CTO. **Evaluación:** Semana 3 de Fase 2.

---

### 8.5 Bloque D: Gobierno de Transición Fase 1 → Fase 2

#### 8.5.1 Checklist de Cierre de MVP (Sprint 0 hasta ~Sprint 6)

Cierre de MVP requiere validación por dominio:

| Dominio | Criterio | Responsable | Target Sprint |
|---------|----------|-------------|---------------|
| **Autenticación** | Login exitoso para 5+ usuarios, 3+ sesiones concurrentes sin sesión cruzada, token expira correctamente | Eng. Backend | Sprint 2 |
| **Sincronización** | Upload/download/delete/rename/move/copy funciona para archivos < 250 MB, sin corrupción de datos, checksum validado en ambos lados | Eng. Backend + Frontend | Sprint 3 |
| **Conflictos** | Conflicto de escritura concurrente genera copia con nombrado legible, ambos archivos accesibles, sin sobrescritura silenciosa | Eng. Backend + QA | Sprint 4 |
| **Metadatos** | SQLite schema v1 se carga/consulta sin error, index básico en path_hash + user_id optimiza queries de lista de archivos | Eng. DB | Sprint 2 |
| **Storage** | LocalDiskStorageProvider escribe/lee/borra en disco local sin error de I/O, espacio de disco monitorizado | Eng. Infra | Sprint 1 |
| **Seguridad** | TLS 1.3 fuerza tráfico, token no se expone en logs, sesión vinculada a device_id, integridad por SHA-256 validada | Eng. Seguridad + Backend | Sprint 3 |
| **Observabilidad** | NDJSON logs escritos, métrica de latencia de sync en p50/p95 disponible, alertas de CPU/RAM/disco no silenciosas | Eng. Ops + Backend | Sprint 4 |
| **Desktop (Windows/macOS/Linux)** | Cliente se compila sin warnings, UI muestra estado de sync, retry manual disponible, icono de bandeja del sistema | Eng. Frontend | Sprint 5 |
| **Android** | APK compila, login/sync funciona en emulador + device real, UI responsive, no crash ante network switch | Eng. Frontend Mobile | Sprint 5 |
| **Integridad de datos** | Backup automatizado en v1 (metadatos + archivos) operativo, restore test semanal exitoso y checksum válido antes/después de restauración | Eng. Ops | Sprint 6 |

**Go/No-Go formal:** Reunión al final de Sprint 6 con CTO, Product, Ops y leads de ingeniería. Se valida cada criterio. Si alguno está "En rojo" sin fecha de cierre clara, se blocka la Fase 2 hasta resolución.

**Propietario:** Scrum Master + CTO. **Revisor:** Product Manager, Ops Lead.

---

#### 8.5.2 Criterios Go/No-Go para Fase 2

**Condición de Go:** 95 % de criterios en VERDE + < 5 incidentes críticos sin causa raíz documentada en Sprint 6.

**Condición de No-Go:** > 3 criterios en ROJO sin roadmap claro O incidente crítico que requiere rediseño arquitectónico.

**Acciones en No-Go:**
1. Identificar raíz de bloqueos.
2. Estimar el costo de remediación (sprints adicionales).
3. Decidir: continuar v1 refinement O pivote de roadmap.

---

#### 8.5.3 Métricas de Go/No-Go Semanales (Sprint 1–6)

**Formato:** tabla semanal publicada a todas las partes interesadas cada viernes.

| Métrica | Sprint 1 | Sprint 2 | Sprint 3 | Sprint 4 | Sprint 5 | Sprint 6 | Target | Estado |
|---------|----------|----------|----------|----------|----------|----------|--------|--------|
| % Criterios (v. Verde) | 30 % | 50 % | 65 % | 75 % | 85 % | 95 % | 95 % | — |
| Incidentes Críticos | < 2 | < 2 | < 2 | < 1 | 0 | 0 | 0 | — |
| P95 Latencia Sync (s) | — | — | 20 | 18 | 15 | 12 | 12 | — |
| Tasa de Error (%) | — | — | 3 | 2 | 1 | 1 | < 1 | — |
| Cobertura de Test (%) | 60 | 70 | 80 | 85 | 90 | 95 | > 90 % | — |

---

#### 8.5.4 Matriz de Decisiones: Propietarios, Calendarios y Criterios

| Decisión | Código | Dominio | Disparo | Propietario | Revisor | Evaluación | Criterio de Aceptación | Estado |
|----------|--------|---------|---------|------------|---------|-----------|----------------------|--------|
| E2E no obligatorio v1 | A1 | Seguridad | Pregunta de usuario sobre cifrado | Arq. Seguridad | CTO | Semana 2 F2 | Diseño E2E aprobado, no implementado | ✅ v1 |
| Gestión de claves post-MVP | A2 | Seguridad | Activación de E2E | Ing. Seguridad | CTO, Auditor | Semana 1 F2 | KMS definido, protocolo escrito | ⏳ F2 |
| Migración LocalDisk → NAS | A3 | Infraestructura | Capacidad > 75 % por 2 semanas | Ing. Infra | CTO, Ops | Semana 3 F2 | Migración completa, validada 100 % | ⏳ F2 |
| Revalidación post-migración | A4 | Infraestructura | Switchover a NAS | Ing. DB | CTO, Auditor | Antes de switchover | Checksum 100 % validado | ⏳ F2 |
| Single-node v1 forzado | B1 | Plataforma | Inicio de Sprint 1 | Lead Ops | CTO | Sprint 1 | SPOF documentado, backups operacionales | ✅ v1 |
| SQLite → PostgreSQL opcional | B2 | Base de Datos | Carga > 10 GB O escrituras > 5 concurrentes | Ing. DB | CTO | Semana 2 F2 | Schéma migraciones validadas | ⏳ F2 |
| LocalDiskStorageProvider → NAS | B3 | Almacenamiento | Capacidad > 75 % por 2 semanas O tercer dispositivo con demanda compartida | Ing. Infra | CTO, Ops | Semana 3 F2 | NasStorageProvider operativo, integridad validada y fallback probado | ⏳ F2 |
| Backup automatizado v1 + restauración avanzada F2 | B4 | Ops | Backup automatizado base obligatorio en v1; mejoras avanzadas tras Go de Fase 2 | Ing. Ops | CTO, Seguridad | Sprint 6 (v1) + Semana 4 F2 | Backup metadatos+archivos automatizado en v1, restore test periódico exitoso, RTO < 4 h y RPO <= 24 h en F2 | ✅ v1 / ⏳ F2 |
| Versionado en Fase 2 | C1 | Capacidades | Usuario intenta restaurar versión anterior | PM | CTO, UX | Semana 1 F2 | 10 versiones / archivo, 90 días retención | ⏳ F2 |
| Compartición en Fase 2 | C2 | Capacidades | 3+ solicitudes en 2 semanas | PM | CTO, Seguridad | Semana 2 F2 | Permisos granulares, auditoría funciona | ⏳ F2 |
| REST → WebSocket → gRPC | C3 | Transporte | P95 sync > 15 s O 50+ deviceos | Ing. Frontend + Backend | CTO | Fase 2/3 | WebSocket falla < 1 %, gRPC latencia < 200 ms | ⏳ F2/3 |
| Offline obligatorio Fase 2 | C4 | Capacidades | Usuario trabajar offline 3+ sesiones/semana | Ing. UX + Backend | CTO | Semana 3 F2 | Caché 1 GB, conflicto resuelto, seguro | ⏳ F2 |
| Cierre de MVP | D1 | Gobierno | Sprint 6 finalizado | Scrum Master + CTO | PM, Ops | Sprint 6 | 95 % criterios GREEN | ⏳ F1 |
| Go/No-Go para Fase 2 | D2 | Gobierno | Fin Sprint 6 | CTO | Execs | Sprint 6 semana 4 | Decisión formal documentada | ⏳ F1 |
| KPI semanal + re-evaluación | D3 | Gobierno | Cada viernes | PM + Ops Lead | CTO | Sprint 1–6 semanal | Tabla actualizada, cambios resaltados | ✅ v1 |
| Matriz de decisiones | D4 | Gobierno | Inicio de proyecto | PM + CTO | Execs | Sprint 0 | Matriz completa, propietarios confirmados | ✅ Sprint 0 |

---

### 8.6 Hitos y Transición: Resumen Visual

**Fase MVP (v1):**
- Single-node estable con REST + polling.
- Backup automatizado activo (metadatos + archivos).
- Cierre MVP con criterios de 8.5.1 y decisión Go/No-Go.

**Ruta de evolución desacoplada:**
- **Carril A (Almacenamiento/NAS):** se activa por capacidad o **tercer dispositivo con demanda compartida**.
- **Carril B (Backup/Restauración):** backup base ya activo en v1; Fase 2 agrega restauración avanzada y optimización operativa.

**Transición a Fase 2:**
- Se habilita por Go/No-Go aprobado.
- Cada decisión se activa por su trigger específico (sin acoplar NAS con backup).

---

### 8.7 Nota de Cierre

Esta Sección 8 **no es especulativa**: cada decisión está vinculada a una condición observable del sistema definida en secciones 1–7. **No hay evolución propuesta sin señal de disparo**.

El objetivo es permitir que el MVP se ejecute sin arquitectura prematura, recopilar datos operativos reales en Fase 1, y tomar decisiones **basadas en evidencia** de qué características y cambios estructurales habilitar en Fase 2.

Los propietarios de decisión son responsables de monitorear sus señales de disparo en tiempo real; cuando se manifieste una señal, el propietario escala a CTO + PM para discusión y scheduling. La matriz semanal (§8.5.3) asegura que no haya sorpresas y que todas las partes interesadas alineadas.

**Aprobación requerida:** CTO, PM, Ops Lead. **Fecha de revisión:** Semana 4 de Sprint 1 + fin de cada Sprint post-MVP.

