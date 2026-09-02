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
