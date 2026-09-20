# Plan Unificado Atómico — SyncFiles (revisado)

> Revisión del estado real al 2026-09-20. Este plan cubre el trabajo restante de V1 y la preparación de V2/V3; no incluye cambios de código en esta fase de planificación.

## Objetivo

Cerrar la distribución y operación de V1, eliminar las deudas de validación y gobernanza, y dejar una ruta explícita hacia V2 multiusuario y V3 en Kubernetes sin perder compatibilidad con `/api/v1`.

## Estado verificado

- Worktree con cambios sin commit; HEAD base: `1c7730e`.
- **Fase A completada en código:** A-01..A-15. WAL/FK, rutas absolutas, integridad, health checks, backups, jobs, RateLimiter y documentación operativa están presentes.
- **A-08:** E2E backup/restore ejecutado y repetible; `scripts/e2e-backup-restore.sh` pasó con checksums verificados.
- **A-10:** `RedisRateLimiter` sigue siendo un stub allow-all; no se presenta como backend Redis distribuido.
- **Completados además:** B-01 firma release de APK, B-02/B-03/B-04/B-05/B-06/B-08 empaquetado/firma/notarización/CLI, B-07 versión, C-01/C-02 Docker/Compose, C-07 helper LAN, C-03/C-04 scripts mínimos cloud, D-02 archivos de gobernanza, D-03 validación de entrada y benchmarks reproducibles.
- **Pendientes de validación ambiental V1:** protección de rama GitHub, secretos/certificados reales de firma, ejecución de Docker en runner, prueba física LAN entre dos dispositivos, despliegue real GCP/AWS con credenciales/costes y OWASP ZAP.
- **V2/V3:** C-05 Google Drive, C-06 NAS y C-08 dual-write quedan fuera de V1; el trabajo multiusuario/E2E/HA se describe más abajo.

## Cierre de V1 al 2026-09-20

- **Código y validación local completados:** hardening, backups/restores, empaquetado, workflows, Docker/Compose, helper LAN, cloud dry-runs, benchmark y manuales.
- **Cierre operativo condicionado:** los artifacts multiplataforma solo se consideran firmados cuando los secretos de GitHub están configurados; Docker, LAN física y cloud real requieren entornos/credenciales externos.
- **No bloquea V2:** el stub Redis y la ausencia de HA/PostgreSQL son limitaciones conocidas de V1 documentadas; no se anuncian como capacidades distribuidas.


## Decisiones de diseño cerradas

| Decisión | Resultado |
|---|---|
| Multiusuario en V2 | **Sí**, con carpetas compartidas, invitaciones y permisos/RBAC. |
| Alcance E2E | **Por archivo**, según decisión del usuario; cada archivo llevará su propia envoltura de clave. |
| MFA en V2 | **TOTP solamente**. WebAuthn queda fuera de V2. |
| Versionado API | **URL path**: `/api/v1` congelado y `/api/v2` para cambios nuevos. |
| Target enterprise | **V3**. V2 puede preparar auditoría, pero GDPR/SOC2 y controles enterprise se reservan para V3. |
| HA V3 | **Kubernetes**, con PostgreSQL externo y almacenamiento persistente/replicado; no se usará SQLite compartido como writers múltiples. |

## Trabajo restante y orden recomendado

### 1. Gobernanza y validación defensiva (paralelo)

1. **D-02 — Gobernanza**
   - Crear `CODEOWNERS` y `.github/dependabot.yml`.
   - Activar en GitHub: protección de `main`, una revisión requerida, CI y clippy obligatorios.
   - Criterio: un PR sin revisión o con CI fallido no puede mergearse.

2. **D-03 — Validación de entrada**
   - No está bloqueada por D-01; puede ejecutarse ahora.
   - Añadir límites de cuerpo, validación de Content-Type, normalización y rechazo de path traversal, y validación estructural de payloads.
   - Mantener consultas SQL parametrizadas; no confiar en una detección regex de “SQL injection”.
   - Criterio: tests de payloads inválidos, cuerpos grandes y traversal;OWASP ZAP sin hallazgos críticos.

### 2. Release y distribución V1

3. **B-05 — Job release en CI**
   - Crear un job separado del job actual de pruebas.
   - Construir server, client y CLI en release; ejecutar tests/clippy; generar APK release cuando existan propiedades de firma; publicar artefactos y SHA256.
   - Criterio: push/tag en `main` genera artefactos verificables; la firma se trata en B-06.

4. **B-06 — Firma de binarios**
   - Integrar `codesign`/notarización macOS, `signtool` Windows y firma GPG de paquetes Linux con secretos externos.
   - Criterio: firma verificable en cada plataforma sin guardar certificados ni contraseñas en el repo.

5. **B-02/B-03/B-04/B-08 — Paquetes**
   - Windows: `.msi` o `.exe` con instalación limpia y acceso al binario.
   - macOS: `.dmg`/`.app` firmado y notarizado.
   - Linux: tarball portable y, si procede, `.deb`/`.rpm` con unidad systemd opcional.
   - CLI: tarball versionado con binario y guía rápida.
   - Criterio: instalación/ejecución probada en un entorno limpio por plataforma.

6. **B-07 — Versión y actualización**
   - Incrementar `versionCode` en cada release Android y definir el flujo manual V1; dejar el check de versión preparado para V2.
   - Criterio: una APK nueva no puede tener un `versionCode` menor y la política de rollback/update está documentada.

### 3. Contenedores, LAN y nube

7. **C-01 — Dockerfile del servidor**
   - Imagen multi-stage con build Rust y runtime mínimo; volumen persistente para SQLite/storage; health endpoints configurados.
   - Criterio: `docker build` y `docker run` arrancan un servidor funcional sin depender del CWD del host.

8. **C-02 — Docker Compose**
   - Servicios server y backup; persistencia explícita; backup programado; Adminer solo para desarrollo.
   - Criterio: `docker compose up` levanta el stack y el backup se puede restaurar; no se permiten múltiples writers sobre una SQLite compartida.

9. **C-07 — LAN**
   - Crear `scripts/setup-lan.sh` o equivalente documentado: IP del servidor, firewall, cleartext permitido solo para LAN en Android y configuración desktop.
   - Criterio: dos dispositivos en la misma red sincronizan correctamente.

10. **C-03/C-04 — GCP/AWS**
    - Preparar scripts mínimos y validar costes/disponibilidad real del free tier antes de prometerlo.
    - GCP: Cloud Run + persistencia/backups compatibles; AWS: ECS/Fargate + persistencia/backups compatibles.
    - Criterio: health checks y backup/restauración funcionan en cada entorno; los secretos se inyectan por secret manager.

11. **Benchmarks (`docs/43-pruebas-rendimiento.md`)**
    - Ejecutar benchmarks reales de upload/download, polling, archivos pequeños/medianos/grandes, red inestable y servidor concurrente.
    - Criterio: resultados reproducibles con comando, entorno, métricas p50/p95 y regresiones documentadas.

### 4. V2: multiusuario, E2E por archivo y API v2

12. **Modelo multiusuario y RBAC**
    - Añadir usuarios, carpetas compartidas, invitaciones y roles owner/editor/viewer; revisar cada endpoint para evitar acceso cruzado.
    - Mantener `/api/v1` congelado y compatible; `/api/v2` expone el nuevo contrato.

13. **E2E por archivo**
    - Cifrar cada archivo en el cliente con una clave/envoltura independiente; definir rotación, recuperación, eliminación y comportamiento ante pérdida de clave.
    - Criterio: el servidor nunca recibe plaintext; los tests verifican round-trip, integridad y permisos por archivo.

14. **MFA TOTP y autenticación V2**
    - Implementar TOTP, códigos de recuperación y flujo de activación/desactivación; WebAuthn queda fuera de V2.
    - Criterio: login, recuperación y revocación cubiertos por tests E2E.

15. **Migración de datos**
    - Mantener SQLite como V1; para V2 definir migración a PostgreSQL con exportación/importación, corte controlado, checksums y rollback desde backup.
    - Criterio: migración de prueba sin pérdida ni acceso cruzado; rollback documentado.

### 5. V3: enterprise y HA en Kubernetes

16. **Enterprise**
    - Añadir auditoría inmutable/exportable, retención, exportación/borrado GDPR y controles SOC2.
    - Criterio: pruebas de auditoría, retención y borrado con trazabilidad.

17. **Kubernetes HA**
    - Desplegar servidores stateless, PostgreSQL HA, storage externo/replicado, probes, secretos y políticas de recursos.
    - Criterio: failover de pod/node sin pérdida de datos y sin writers múltiples sobre SQLite.

18. **C-08 — Dual-write/reconciliación**
    - Implementar solo cuando exista el storage externo de V2/V3; hacerlo opt-in y con reconciliación verificable.
    - Criterio: copias idénticas y detección de discrepancias; no se presenta como reemplazo de backups.

## Ruta de datos y migración

- **V1 actual:** cliente → `/api/v1` → Actix server → SQLite de metadatos + storage local; backup mediante `VACUUM INTO` y tar.
- **Transición V2:** `/api/v1` sigue sirviendo clientes antiguos; `/api/v2` añade usuarios, shares, RBAC, TOTP y E2E por archivo; PostgreSQL reemplaza SQLite tras migración controlada.
- **Transición V3:** la aplicación se vuelve stateless en Kubernetes; PostgreSQL HA y storage externo/replicado eliminan la dependencia de SQLite local.
- **Rollback:** backup verificable antes de cada migración, versión anterior disponible, restauración probada y compatibilidad de clientes V1 durante el periodo de deprecación.

## Riesgos y fallos que el plan debe cubrir

- E2E por archivo complica rotación, recuperación y sharing; definir formato de claves y flujo de recuperación antes de implementar.
- Kubernetes no es una solución HA para la SQLite actual; requiere PostgreSQL/storage externo.
- La firma requiere secretos y certificados externos; nunca commitir keystores ni tokens.
- Los supuestos de “free tier” de GCP/AWS deben verificarse contra la oferta vigente antes del despliegue.
- El stub Redis de A-10 no debe presentarse como rate limiting distribuido.
- El script de backup depende de herramientas del host (`sqlite3`, `tar`, `sha256sum`/equivalentes); validar en los sistemas objetivo.

## Criterio de cierre

- V1 tiene release firmado (cuando se configuran secretos), instaladores verificables, Docker/LAN operativos, backup/restore probado, gobernanza y benchmarks reales.
- V2 mantiene compatibilidad V1 y cumple multiusuario/RBAC, E2E por archivo, TOTP y `/api/v2`.
- V3 tiene auditoría enterprise y HA Kubernetes con PostgreSQL externo, failover y restauración validados.
