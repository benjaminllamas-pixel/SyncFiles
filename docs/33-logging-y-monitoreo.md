# Logging y Monitoreo

## 1. Visión General

SyncFiles V1 utiliza **logging estructurado NDJSON** (una línea JSON por evento) con
correlación por `request_id` y auditoría mínima. El subsistema de logging está
implementado con `tracing` + `tracing-subscriber` en el servidor, y con logs
structurados en los clientes (desktop, Android).

## 2. Niveles de Log

| Nivel | Cuándo se usa |
|-------|--------------|
| `INFO` | Operaciones normales: login exitoso, upload/download completado, sync ciclo OK |
| `WARN` | Condiciones recuperables: rate limit, retry programado, path traversal bloqueado |
| `ERROR` | Fallos que requieren intervención: integrity_check falló, DB corrupta, storage no disponible |

En producción se recomienda `SF_LOG_LEVEL=info` (default). Durante debugging
operativo: `SF_LOG_LEVEL=debug` (no recomendado en producción por volumen).

## 3. Formato NDJSON

Cada línea es un JSON independiente con los siguientes campos mínimos:

| Campo | Descripción | Ejemplo |
|-------|-------------|---------|
| `ts` | Timestamp ISO 8601 UTC | `2026-09-16T12:34:56.789Z` |
| `level` | Nivel de log | `INFO` |
| `event` | Nombre del evento | `auth.login_success` |
| `user_id` | ID del usuario (o `system`) | `user-001` |
| `device_id` | ID del dispositivo (o `server`) | `device-abc` |
| `request_id` | Correlation ID único por request | `a1b2c3d4` |
| `message` | Mensaje legible | `Login exitoso` |
| `result` | Resultado (`ok` / `error`) | `ok` |

Ejemplo de salida:

```json
{"ts":"2026-09-16T12:34:56.789Z","level":"INFO","event":"auth.login_success","user_id":"user-001","device_id":"device-abc","request_id":"a1b2c3d4","result":"ok","message":"Login exitoso"}
```

## 4. Eventos de Auditoría

Los eventos que deben registrarse en `audit_log` (tabla de la BD) incluyen:

- `auth.login_success` — login exitoso
- `auth.login_failed` — login fallido (email/IP, motivo)
- `sync.file_uploaded` — upload completado
- `sync.file_downloaded` — download completado
- `sync.file_deleted` — borrado
- `sync.file_renamed` — rename
- `sync.file_moved` — move
- `sync.file_copied` — copy
- `sync.conflict_detected` — conflicto detectado (strategy aplicada)
- `sync.retry_scheduled` — reintento programado
- `sync.quarantined` — operación en quarantesa

## 5. Métricas

| Métrica | Tipo | Descripción | Umbral operativo |
|---------|------|-------------|-----------------|
| `sync_operation_duration_ms` | Histograma p50/p95 | Latencia de operaciones sync | p95 < 30s para archivos < 10MB |
| `sync_failed_operations_total` | Contador | Operaciones fallidas acumuladas | Crecimiento sostenido > 5 min = investigar |
| `active_sessions_count` | Gauge | Sesiones activas | Valor anómalo = sesión colgada |
| `queue_depth` | Gauge | Operaciones pendientes en sync_queue | > 100 = backpressure |
| `retry_rate` | Ratio | Tasa de reintentos / operaciones totales | > 10% = problema de red o storage |
| `conflict_rate` | Ratio | Conflictos / operaciones de sync | > 5% = política de resolución revisar |
| `storage_usage_per_user` | Gauge | Bytes usados por usuario | Para cuotas y alerting |

## 6. Health Check Endpoints

| Endpoint | Method | Respuesta | Comentario |
|----------|--------|-----------|------------|
| `/health/live` | GET | 200 `{"status":"alive"}` | Liveness: el proceso está vivo |
| `/health/ready` | GET | 200/503 | Readiness: verifica DB + storage. 503 si falla alguno |

El readiness probe escribe y borra un archivo de test (`__healthcheck__/ping.txt`)
en storage. En entornos CI sin permisos de escritura, usar:

```bash
SF_HEALTH_PROBE_WRITE=0
```

## 7. Configuración

| Variable | Default | Descripción |
|----------|---------|-------------|
| `SF_LOG_LEVEL` | `info` | Nivel mínimo de log (`trace`, `debug`, `info`, `warn`, `error`) |
| `SF_LOG_FORMAT` | `json` | `json` (NDJSON) o `text` (legible) |
| `SF_HEALTH_PROBE_WRITE` | `1` | `0` para deshabilitar el write probe en readiness (CI) |

Ejemplo de configuración para producción:

```bash
SF_LOG_LEVEL=info SF_LOG_FORMAT=json SF_HEALTH_PROBE_WRITE=1 ./syncfiles-server
```

## 8. Rotación y Retención

- **Rotación:** diaria (midnight).
- **Retención:** 30 días de logs operativos.
- **Auditoría:** los eventos de `audit_log` persiten en la BD y no se eliminan
  (retención indefinida hasta que se compacten con `archive_audit` en V2).
- **Externalización:** en despliegue Docker/Cloud Run, los logs van a stdout
  (capturados por el runtime). Para bare-metal, usar `journald` o redireccionar
  a un archivo con rotación vía `logrotate`.

## 9. Integración con Observabilidad

- **Tracing:** cada request lleva un `request_id` generado por el middleware
  `trace_context`. Se propaga a través de los handlers.
- **Correlation:** los logs de servidor y cliente comparten `request_id` cuando
  el cliente lo envía vía header `X-Request-Id` (opcional).
- **Alerting:** los operadores deben alertar en:
  - `/health/ready` responde 503 > 30s (SEV1)
  - `sync_failed_operations_total` sube > 200% en 5 min (SEV2)
  - `rate_limited` respuestas > 10% del tráfico (SEV3)
