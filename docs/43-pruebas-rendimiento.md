# Pruebas de Rendimiento

## 1. Objetivo

Las pruebas de rendimiento de SyncFiles deben validar que la sincronización es viable en una red doméstica y en clientes móviles sin introducir latencias excesivas ni saturación del servidor.

Como V1 prioriza confiabilidad por encima de rendimiento extremo, el objetivo no es hacer un sistema ultra optimizado, sino un sistema predecible, estable y medible.

## 2. Métricas clave

- tiempo de sincronización por archivo,
- tiempo de polling medio y máximo,
- throughput de upload/download,
- tasa de reintentos por error de red,
- uso de CPU, memoria y disco,
- latencia de API del servidor,
- tiempo de recuperación tras fallo.

## 3. Benchmarks recomendados

### 3.1 Archivos pequeños

- 1 KB a 1 MB,
- 100 a 500 archivos simultáneos,
- red doméstica con latencia normal,
- objetivo: sincronización total sin bloqueos perceptibles.

### 3.2 Archivos medianos

- 5 MB a 50 MB,
- validación de checksum y reintentos,
- objetivo: mantener latencia aceptable para uso cotidiano.

### 3.3 Archivos grandes

- 100 MB a varios GB,
- pruebas de chunking, streaming y reanudación,
- objetivo: detectar cuello de botella de red y de disco.

## 4. SLO operativos recomendados (V1)

- 95% de operaciones de sync terminan en menos de 30 segundos para archivos menores a 10 MB en red doméstica.
- polling con backoff adaptativo para evitar picos de carga.
- reintentos limitados para reducir presión sobre el backend.
- tiempo de recuperación tras caída de red menor a 2 minutos en la mayoría de escenarios.

## 5. Áreas a medir

### 5.1 Cliente

- consumo de CPU durante escaneo de árbol,
- memoria al manejar cola y conflictos,
- duración de eventos de watch y debounce,
- latencia de ejecución de la cola local.

### 5.2 Servidor

- tiempo de respuesta por endpoint,
- latencia de SQLite bajo carga moderada,
- atención de uploads concurrentes,
- duración de validación de checksum y persistencia.

### 5.3 Red

- tasa de transferencia por cliente,
- falla de conexiones o timeouts,
- número de fallos con reintento,
- latencia del polling en red móvil.

## 6. Casos de prueba clave

- sincronización de 100 archivos pequeños en una sola operación,
- descarga repetida del mismo archivo tras modificado,
- conflicto de archivo más reciente entre dos dispositivos,
- reinicio del cliente con cola persistida,
- fallo de red a mitad del upload,
- servidor bajo carga durante polling simultáneo,
- operación con disco casi lleno.

## 7. Plan de benchmarking

Se recomienda un ciclo de benchmarking con:
- pruebas locales en entorno de desarrollo,
- pruebas de integración en CI,
- pruebas periódicas de regresión,
- comparación de benchmark en cada cambio arquitectónico importante.

## 8. Criterios de aceptación

La arquitectura de V1 es aceptable en rendimiento si:
- el sistema opera sin bloquear la interfaz del usuario al sincronizar archivos de uso cotidiano,
- la cola no sufre pérdidas ni corrupción en fallos temporales,
- el cliente continúa operativo ante redes lentas o caídas puntuales,
- el backend no muestra degradación severa con varios clientes concurrentes,
- las métricas se mantienen dentro de los objetivos definidos por el producto.

## 9. Recomendación final

Para V1, la estrategia correcta es medir y estabilizar. No conviene sobre-optimizar desde el inicio. Debe validarse la velocidad real de sincronización, la resiliencia y la escalabilidad moderada, dejando la optimización avanzada para V2, cuando NAS, PostgreSQL, Kafka y flujo multi-dispositivo requieran un diseño más exigente.
