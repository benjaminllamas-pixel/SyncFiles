# Pruebas de Rendimiento

## Objetivo

V1 debe ser predecible en una red doméstica. El benchmark local mide la API real sobre HTTP, SQLite y storage local, sin depender de librerías externas.

## Comando reproducible

```bash
scripts/benchmark.sh
```

El script compila el servidor si es necesario, arranca una instancia aislada en un puerto libre, crea una cuenta de benchmark, ejecuta upload/download/diff y genera:

- `results.csv`: una fila por operación, tamaño, ronda y concurrencia.
- `summary.json`: muestras, p50/p95/p99, mínimo, máximo y resultado del SLO.

Por defecto prueba 1 KiB, 64 KiB, 1 MiB, 5 MiB y 10 MiB, tres rondas y cuatro solicitudes concurrentes. Para una prueba más corta:

```bash
SF_BENCH_SIZES=1024,65536 SF_BENCH_ROUNDS=2 SF_BENCH_CONCURRENCY=2 scripts/benchmark.sh
```

Para apuntar a un servidor ya desplegado:

```bash
SF_BENCH_SERVER_URL=https://syncfiles.example.com \
SF_BENCH_EMAIL=admin@example.com \
SF_BENCH_PASSWORD='...' \
SF_BENCH_DEVICE_ID=benchmark \
scripts/benchmark.sh
```

## Métricas

| Métrica | Cálculo |
|---|---|
| Duración upload/download/diff | `perf_counter` alrededor de cada request |
| Throughput | bytes transferidos / duración |
| p50/p95/p99 | percentil de muestra por escenario y operación |
| Concurrencia | ThreadPoolExecutor con `SF_BENCH_CONCURRENCY` |
| Integridad | SHA-256 y tamaño verificados en cada descarga |

## SLO V1

- p95 de upload para archivos menores a 10 MiB: `< 30 s`.
- Todas las descargas deben conservar SHA-256 y tamaño.
- El servidor debe permanecer `ready` durante la prueba.

El script termina con código distinto de cero si no hay muestras o si el SLO falla.

## Factores que cambian los resultados

- CPU y disco del host.
- Tamaño del archivo y codificación base64 del contrato V1.
- Límite `SF_MAX_UPLOAD_BYTES`.
- Rate limiter; el servidor de benchmark usa límites altos.
- Latencia de red cuando `SF_BENCH_SERVER_URL` apunta a un equipo remoto.
- `SF_BENCH_LATENCY_MS` permite añadir latencia artificial del cliente.

## Integración

CI ejecuta una versión corta del benchmark en cada cambio y publica `results.csv` como artefacto. Para una comparación de regresión, conserve también `summary.json` y compare p95 por tamaño/operación antes de fusionar cambios de red, storage o serialización.
