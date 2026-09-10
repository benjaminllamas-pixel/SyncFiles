# Estrategia de Pruebas

## 1. Objetivo

La estrategia de pruebas de SyncFiles debe garantizar que la sincronización funcione de forma fiable en V1 y que la arquitectura pueda evolucionar a V2 sin introducir regresiones funcionales ni de seguridad.

La prioridad no es maximizar la cantidad de pruebas, sino cubrir los riesgos reales del sistema:
- pérdida o corrupción de archivos,
- conflicto de sincronización,
- reintentos y estados inconsistentes,
- sesiones inválidas,
- fallo de red o de disco,
- regresiones de compatibilidad entre clientes.

## 2. Principios

- Probar primero la lógica de negocio crítica: sincronización, conflictos y recuperación.
- Hacer pruebas de fallo antes que optimización de rendimiento.
- Mantener pruebas deterministas y reproducibles.
- Diferenciar claramente pruebas unitarias, de integración, de sistema y de seguridad.
- Validar siempre la compatibilidad de V1 con la evolución a V2.

## 3. Pirámide de testing

### 3.1 Unit tests

Cobertura mínima recomendada:
- hashing y checksum,
- normalización de rutas,
- política de conflicto,
- cálculo de `modified_at`,
- serialización JSON y validación de payloads,
- lógica de backoff y idempotencia.

### 3.2 Integration tests

Validan la interacción entre:
- cliente local y SQLite,
- cliente y servidor REST,
- servidor y almacenamiento local,
- notificaciones y cola de reintentos.

### 3.3 System tests

Se ejecutan sobre escenarios completos con varios dispositivos simulados:
- upload desde un cliente,
- sync de dos clientes con cambios diferentes,
- conflicto de archivo más reciente,
- reconexión tras caída de red,
- restauración después de reinicio.

### 3.4 Exploratory / resilience tests

Se usan para validar comportamientos no cubiertos por pruebas automatizadas:
- corte de red durante upload,
- disco lleno,
- ruta con caracteres especiales,
- nombres duplicados y renombrados,
- conflicto con archivos temporales, carpetas o sistemas de archivos distintos.

## 4. Matriz de pruebas por fase

### 4.1 V1

Debe cubrir:
- autenticación y sesión,
- upload/download/delete/rename/move/copy,
- polling con cambios remotos,
- reintentos de red,
- conflicto y resolución por confirmación,
- estado de cola y recuperación tras reinicio,
- validación de checksum,
- expiración y recuperación de la sesión.

### 4.2 V2

Debe añadirse además:
- NAS failover y resiliencia,
- PostgreSQL migration tests,
- Kafka event consistency,
- E2E key management y rotación,
- pruebas multi-dispositivo y escalabilidad.

## 5. Reglas de calidad

- Cualquier cambio en la lógica de sincronización debe tener pruebas de regresión.
- Cualquier cambio en conflictos debe validarse con escenario “dos clientes, un archivo, mismo tiempo”.
- Cualquier modificación en la API debe verificar compatibilidad con clientes existentes.
- La prueba no puede depender de un entorno manual no reproducible.
- Los fallos de sincronización deben dejar evidencia de estado para diagnóstico.

## 6. Pruebas automatizadas recomendadas

- unit: en cada pull request,
- integration: en cada merge a main,
- system: en CI con entorno desacoplado,
- security: en cada release candidate,
- performance: en cada cambio de arquitectura o de endpoint crítico.

## 7. Criterios de salida

La versión V1 se considera lista para validación operativa cuando:
- el 100% de las pruebas funcionales críticas pasa,
- los escenarios de conflicto y restablecimiento se ejecutan exitosamente,
- no existen regresiones en sesiones, autenticación ni cola,
- el patrón de fallos es consistente y documentado.

## 8. Recomendación final

La estrategia correcta para SyncFiles es una combinación de:
- pruebas unitarias exhaustivas para la lógica de sincronización,
- pruebas de integración para cliente-servidor,
- pruebas de sistema para escenarios reales de conflicto y recuperación,
- pruebas de seguridad y rendimiento como capa de validación transversal.

Esto es suficiente para cerrar V1 y preparar una evolución ordenada a V2 sin aferrarse a una estrategia de pruebas demasiado compleja para el MVP.

## 9. Resultados de la batería E2E real (Fase 5 del plan UI)

> Ejecutada el 2026-09-10 con `scripts/e2e-phase5.sh` (17 PASS / 0 FAIL),
> `scripts/e2e-test.sh` (smoke, PASS) y `cargo test` del server (21 tests PASS:
> 6 unit + 9 api_endpoints + 6 static_dashboard).

### 9.1 Escenarios ejecutados

| # | Escenario | Herramienta | Resultado |
|---|---|---|---|
| 5.1 | Servidor local levantado | `scripts/e2e-test.sh` (base) | PASS |
| 5.2 | Multi-dispositivo: A sube → B ve el diff → B descarga → B borra → A ve el delete | CLI + curl | PASS (4 asserts) |
| 5.3 | Conflicto mismo path/checksum distinto desde 2 dispositivos; detección, listado (`GET /conflicts`) y resolución `keep_local` con `preserve_alternative` | CLI + curl | PASS (5 asserts) |
| 5.4 | Reinicio de cliente con cambios pendientes: cliente desktop headless (motor real) sube archivo en caliente, se mata, se crea archivo offline, se reinicia y el escaneo de reconciliación lo sube | `syncfiles-client` con `SF_HEADLESS=1` | PASS (4 asserts) |
| 5.5 | Red interrumpida: servidor caído → subida falla limpiamente; servidor recuperado → reintento manual OK; motor con archivo pendiente reintenta solo tras recuperar el servidor | CLI + cliente headless | PASS (4 asserts) |

### 9.2 Cobertura de la UI en los escenarios

Los endpoints que consumen las tres UIs quedaron verificados con datos reales:
- `GET /files/list` (Archivos desktop/web/Android): listó archivos subidos por el motor.
- `GET /conflicts` + `POST /conflicts/resolve` (vista Conflictos): conflicto creado, listado y resuelto con copia alternativa.
- `GET /activity` (vista Actividad): eventos `conflict.resolved` y auditoría registrados.
- `GET /queue` (vista Cola): respondió correctamente (vacía tras procesar; la cola
  con contenido se validó en `tests/api_endpoints.rs`).
- `GET /devices`, `GET /storage/stats` (Dashboard): cubiertos por tests de integración.

### 9.3 Bugs encontrados y corregidos durante la Fase 5

1. **CLI (`syncfiles-cli`)**: `<bin> login` interpretaba "login" como URL del
   servidor (parseo posicional desalineado) → añadido modo subcomando
   (`<bin> <comando> [args]` + env vars) manteniendo compatibilidad posicional.
2. **CLI login**: imprimía solo el session_id; el smoke test esperaba JSON con
   `session_id` → ahora `login` imprime JSON completo `{"status":"ok",...}`.
3. **Cliente desktop (motor)**: no existía escaneo inicial de la carpeta; los
   archivos creados/editados con el cliente apagado jamás se sincronizaban →
   añadido `reconcile_local_folder()` al inicio de cada ciclo (escaneo → diff de
   checksums contra BD → encola 'pending').
4. **Cliente desktop (cola)**: operaciones en cola sin payload quedaban
   `queued` eternamente → ahora se marcan `done` con nota "sin payload".
5. **Script e2e-test.sh**: `sleep 2` no esperaba el arranque del servidor →
   espera activa con curl; descarga usaba `dummy-id` → usa el `file_id` del diff
   y verifica el contenido con `cmp`.

### 9.4 Reproducibilidad

```bash
# Prerrequisitos (una vez)
(cd syncfiles-server && cargo build)
(cd syncfiles-cli && cargo build)
(cd syncfiles-client && cargo build)   # incluye modo headless SF_HEADLESS=1

# Batería completa de Fase 5 (~2 min, usa puerto 8082 y /tmp/syncfiles-e2e-phase5)
bash scripts/e2e-phase5.sh

# Smoke original (puerto 8081)
bash scripts/e2e-test.sh

# Tests de integración del servidor
(cd syncfiles-server && cargo test)
```

El cliente headless (`SF_HEADLESS=1`) aísla sus datos con `SF_DATA_DIR` y su
carpeta con `SF_SYNC_ROOT`, por lo que la batería no toca los datos reales del
usuario. Nota: usa la misma cuenta `admin@syncfiles.local` contra el servidor
de prueba en el puerto 8082; no conectar contra producción.
