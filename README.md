# SyncFiles - Sistema de Sincronización de Archivos Encriptados

## Descripción
Sistema multiplataforma de sincronización de archivos con encriptación end-to-end prevista para fases posteriores y respaldo con soporte futuro en servidor NAS.

## Fase Actual
**V1 — Prototipo funcional** (sincronización operativa) + hardening completado: tests automatizados (server, client, models), CI/CD (GitHub Actions), health checks, rate limiting, migraciones versionadas y validación E2E.

## Estructura de Documentación

### Análisis y Requerimientos
- `01-requerimientos-funcionales.md` - Casos de uso y funcionalidades del sistema
- `02-requerimientos-no-funcionales.md` - Requisitos de rendimiento, seguridad y calidad
- `03-casos-de-uso.md` - Escenarios detallados de uso del sistema
- `04-analisis-de-riesgos.md` - Identificación y mitigación de riesgos

### Arquitectura del Sistema
- `05-arquitectura-general.md` - Vista de alto nivel del sistema
- `06-arquitectura-cliente.md` - Diseño del componente cliente multiplataforma
- `07-arquitectura-servidor.md` - Diseño del servidor/NAS y servicios backend
- `08-arquitectura-red.md` - Protocolos de comunicación y topología de red
 
### Seguridad y Encriptación
- `09-modelo-encriptacion.md` - Algoritmos y estrategias de encriptación
- `10-gestion-de-claves.md` - Manejo, almacenamiento y distribución de claves
- `11-seguridad-comunicaciones.md` - TLS, autenticación y seguridad en tránsito
- `12-seguridad-almacenamiento.md` - Protección de datos en reposo

### Sincronización y Datos
- `13-algoritmo-sincronizacion.md` - Lógica de detección y propagación de cambios
- `14-deteccion-de-cambios.md` - File watchers y mecanismos de monitoreo
- `15-resolucion-conflictos.md` - Estrategias para manejar conflictos de sincronización
- `16-versionado-de-archivos.md` - Sistema de versionado y recuperación

### Modelo de Datos
- `17-modelo-de-datos.md` - Estructura de metadatos y esquemas
- `18-formato-archivos-encriptados.md` - Especificación del formato de almacenamiento
- `19-estructura-directorios.md` - Organización de archivos en NAS y clientes
- `20-base-de-datos-metadatos.md` - Almacenamiento de información de sincronización

### Tecnologías y Stack
- `21-seleccion-lenguaje.md` - Evaluación y elección del lenguaje de programación
- `22-seleccion-framework.md` - Frameworks para cliente y servidor
- `23-librerias-encriptacion.md` - Evaluación de bibliotecas criptográficas
- `24-protocolo-comunicacion.md` - Selección de protocolo (rsync, SFTP, WebDAV, custom)

### Plataformas y Compatibilidad
- `25-cliente-windows.md` - Especificaciones para Windows
- `26-cliente-macos.md` - Especificaciones para macOS
- `27-cliente-linux.md` - Especificaciones para Linux
- `28-cliente-movil.md` - Especificaciones para iOS y Android

### Performance y Escalabilidad
- `29-optimizacion-rendimiento.md` - Estrategias de optimización
- `30-chunking-archivos-grandes.md` - Manejo de archivos de gran tamaño
- `31-compresion.md` - Algoritmos de compresión previo a encriptación
- `32-escalabilidad.md` - Diseño para crecimiento de usuarios y datos

### Operaciones y Mantenimiento
- `33-logging-y-monitoreo.md` - Sistema de logs y telemetría
- `34-manejo-de-errores.md` - Estrategias de recuperación ante fallos
- `35-actualizaciones.md` - Sistema de actualización de clientes
- `36-backup-y-restauracion.md` - Procedimientos de respaldo y recuperación

### UI/UX y Usabilidad
- `37-interfaz-usuario.md` - Diseño de interfaces gráficas
- `38-configuracion-inicial.md` - Proceso de onboarding y setup
- `39-notificaciones.md` - Sistema de notificaciones al usuario
- `40-experiencia-usuario.md` - Flujos de usuario y usabilidad

### Pruebas y Calidad
- `41-estrategia-pruebas.md` - Plan de testing (unitarias, integración, E2E)
- `42-pruebas-seguridad.md` - Auditorías y pentesting
- `43-pruebas-rendimiento.md` - Benchmarking y stress testing
- `44-calidad-codigo.md` - Estándares y revisiones de código

### Despliegue e Infraestructura
- `45-infraestructura-nas.md` - Configuración del servidor NAS
- `46-deployment-clientes.md` - Distribución de aplicaciones cliente
- `47-ci-cd.md` - Pipelines de integración y despliegue continuo
- `48-documentacion-usuario.md` - Manuales y guías de usuario

### Roadmap y Futuro
- `49-roadmap.md` - Plan de desarrollo por fases
- `50-funcionalidades-futuras.md` - Features planificadas para versiones posteriores

## Estado del Proyecto
🟢 V1 funcional — server + cliente desktop + CLI + Android + web + Emacs, con CI y suite de tests

## Tests y CI

```bash
cargo test --manifest-path syncfiles-models/Cargo.toml
cargo test --manifest-path syncfiles-server/Cargo.toml
cargo test --manifest-path syncfiles-client/Cargo.toml -- --test-threads=1
bash scripts/e2e-test.sh              # regresión E2E completa
bash scripts/e2e-phase5.sh             # escenarios fase 5
bash scripts/e2e-rename-move-copy.sh   # rename/move/copy (27 checks)
```

CI en `.github/workflows/ci.yml`: check + test (server/client/cli/models) + clippy en cada push/PR a `main`, más `assembleDebug` Android con artefacto APK.

## Próximos Pasos
1. V2: encriptación end-to-end, soporte NAS, PostgreSQL, Kafka (ver `docs/49-roadmap.md`)
2. Limpiar deuda de clippy (hoy informativo en CI)
3. Load testing y containerización (Docker)

## Instalación

Manual completo en [`docs/48-documentacion-usuario.md`](docs/48-documentacion-usuario.md): servidor, dashboard web, cliente desktop, CLI, APK Android (debug y release firmado) y cliente Emacs.

Comandos de compilación por componente:

```bash
cargo build --release --manifest-path syncfiles-server/Cargo.toml   # servidor + dashboard web (:8080)
cargo build --release --manifest-path syncfiles-client/Cargo.toml   # cliente desktop
cargo build --release --manifest-path syncfiles-cli/Cargo.toml      # cliente CLI
cd syncfiles-android && ./gradlew assembleDebug                      # APK Android (debug)
```

El cliente Emacs no requiere compilación: carga `syncfiles-emacs/syncfiles.el`
desde Emacs 29+ (ver `syncfiles-emacs/README.md`).

```elisp
(add-to-list 'load-path "syncfiles-emacs")
(require 'syncfiles)   ; M-x syncfiles-login, syncfiles-upload, ...
```

## Dashboard UI

El cliente incluye un prototipo nativo del dashboard con estado de conexión,
carpeta sincronizada, archivos recientes y acción de sincronización manual.

```bash
cd syncfiles-client
cargo run --release --manifest-path Cargo.toml
```

La URL del servidor y la carpeta local se pueden personalizar con `SF_SERVER_URL`
y `SF_SYNC_ROOT`.

En pantallas pequeñas el dashboard cambia a navegación compacta y tarjetas
adaptadas para Android. En Android la carpeta se configura escribiendo la ruta
en Preferencias; el selector gráfico de carpetas queda disponible en escritorio.
