# Sistema de Actualización

## 1. Versionamiento de Software (Semver)

SyncFiles sigue **Semantic Versioning 2.0.0** (`MAJOR.MINOR.PATCH`):

- **MAJOR**: breaking changes que requieren migración manual (V2+). En V1 no se
  previsten breaking changes en el formato de storage sin migración asistida.
- **MINOR**: nuevas funcionalidades compatibles con versiones anteriores.
- **PATCH**: correcciones de bugs, mejoras de rendimiento, hardening.

Los versiones se publican como **git tags** (`v1.0.0`) y como releases en GitHub
con los binarios firmados y checksums SHA256.

## 2. Versionamiento de API

| Versión API | Estado | Compatibilidad |
|-------------|--------|----------------|
| `/api/v1` | Activo (congelado en V1) | Inmutable: no se añaden breaking changes. Nuevos endpoints son aditivos. |
| `/api/v2` | Planeado para V2 (pendiente decisiones D-01) | Resolverá multi-usuario, MFA, versionado formal. |

**Política de compatibilidad:**
- Clientes V1 funcionan contra servidores V1 (garantizado).
- Servidores V2 mantendrán compatibilidad con clientes V1 mediante feature flag
  `v1_compat` (planeado).
- Los clientes reportan su versión en cada request vía header `X-SyncFiles-Client-Version`
  para server-side compatibility checks.
- Se recomienda validar compatibilidad de API antes de activar nuevas capacidades
  (ver §4).

## 3. Versionado de Android (APK)

| Campo | Descripción |
|-------|-------------|
| `versionCode` | Entero incremental. Android exige valor mayor al instalado. |
| `versionName` | `MAJOR.MINOR.PATCH` (ej. `1.0.0`). |

**Política:** en cada release, incrementar `versionCode` en `syncfiles-android/app/build.gradle.kts`.
El cliente verifica `versionCode` en cada arranque (manual en V1: notificación).

Ver: `docs/48-documentacion-usuario.md` § instalación APK.

## 4. Mecanismo de Verificación de Compatibilidad

1. **Cliente → Server:** envía `X-SyncFiles-Client-Version: <semver>` en headers.
2. **Server → Cliente:** si la versión es incompatible (mayor MAJOR distinto),
   responde `426 Upgrade Required` con `server_version` en el body.
3. **Desktop/Android:** muestra notificación de actualización requerida.
4. **API versioning:** el cliente usa siempre `/api/v1`. Si el server responde
   `X-API-Deprecated: true`, el cliente muestra aviso de actualización.

## 5. Estrategia de Rollback

### 5.1 Server (bare metal / Docker)

1. **Antes del deploy:** ejecutar backup manual (`POST /admin/backup` o
   `scripts/backup.sh`).
2. **Deploy:** `git tag vX.Y.Z && git push origin --tags`. El CI genera release.
3. **Rollback:** en caso de problema crítico:
   - Detener el server: `systemctl stop syncfiles-server` (o `docker-compose down`).
   - Restaurar DB desde backup (ver `docs/36-backup-y-restauracion.md`).
   - Reinstalar binario anterior: descargar `syncfiles-server` del release tag
     anterior en GitHub Releases.
   - Reiniciar: `systemctl start syncfiles-server`.
4. **Verificación:** `/health/ready` responde 200; tests E2E pasan.

### 5.2 Cliente Desktop

- El cliente descarga el binario del release tag en GitHub Releases.
- Si la versión nueva falla al arrancar o al sync, el cliente degrada automáticamente
  al binario anterior (si está en caché) o notifica al usuario para reinstalar manualmente.
- El cliente persiste `last_sync_version` por archivo; tras rollback, los cambios
  no aplicados se re-procesan en el siguiente ciclo.

### 5.3 Cliente Android

- **Manual (V1):** el usuario descarga el APK del release tag y lo instala.
  Android exige `versionCode` mayor para actualizar.
- **Auto-update (V2, planeado):** el cliente consulta `GET /api/v1/app/version`
  y notifica si hay una versión nueva disponible.

## 6. Calendario de Actualizaciones

| Tipo | Frecuencia | Responsable |
|------|-----------|-------------|
| Patch releases | Emergency / semanal | Equipo de desarrollo |
| Minor releases | Mensual | PM + CTO |
| Major releases (V2+) | Trimestral | CTO + Arquitectura |
| Release de seguridad | Immediate (coordinated disclosure) | Security Lead |

**Ventana de mantenimiento:** martes 02:00–04:00 UTC (horario de bajo tráfico).
Los deploys de patch fuera de ventana requieren aprobación del CTO.

## 7. Proceso de Release (CI/CD)

Ver: `docs/47-ci-cd.md`

1. Push a `main` → CI ejecuta: `cargo test` + `cargo clippy -D warnings`.
2. Si CI pasa, se crea git tag `vX.Y.Z` → GitHub Release con:
   - Binarios: `syncfiles-server`, `syncfiles-client`, `syncfiles-cli` (Linux/macOS/Windows).
   - APK debug (release firmada en V2).
   - Checksums SHA256.
   - Changelog (automático desde commits).
3. El release se notifica a canales de usuarios via Discord/GitHub Discussions.

## 8. Rollback por Versión (no por ad hoc fixes)

> "Un problema crítico debe revertirse por versionado, no por ad hoc fixes."

- Nunca se patcha en producción sin tag de versión.
- Si un release `v1.2.3` introduce un bug, se publica `v1.2.4` con el fix, o se
  revierte a `v1.2.2` (rollback documentado).
- Cada rollback se documenta: causa raíz, timeline, lecciones aprendidas en
  el post-mortem.
