# Plan: Manual de Instalación de SyncFiles + Empaquetado del APK Android

## Contexto

El proyecto SyncFiles (V1 prototipo funcional) tiene 4 componentes entregables:
- `syncfiles-server` (Rust/actix-web) — servidor; sirve además el dashboard web en `/` (`static/`).
- `syncfiles-client` (Rust/egui) — cliente desktop con binario `syncfiles-ui`.
- `syncfiles-cli` (Rust) — cliente CLI para pruebas.
- `syncfiles-android` (Kotlin/Compose) — app Android completa (login, sync en fondo con WorkManager, archivos, conflictos, ajustes).

No existe documentación de instalación/usuario: `docs/48-documentacion-usuario.md` está casi vacío (solo formato previsto). El APK no tiene firma release: `syncfiles-android/app/build.gradle.kts` define `release { isMinifyEnabled = false }` sin `signingConfig` ni keystore. `network_security_config.xml` solo permite HTTP plano contra `10.0.2.2`/`127.0.0.1`/`localhost` (emulador); un teléfono físico en LAN requiere añadir la IP del servidor o usar HTTPS.

Decisiones tomadas con el usuario:
1. Manual completo (server + desktop + CLI + web + APK) en `docs/48-documentacion-usuario.md`.
2. Firma release con keystore local nuevo (no en git).
3. El manual explica cómo habilitar HTTP en LAN (edición de `network_security_config.xml`); sin cambios de código ahora.
4. El manual contiene los pasos de build; el usuario ejecuta los builds (no se automatiza con script).

## Tareas

### 1. Configurar firma release del APK (source edit)

**1.1. Keystore** — el manual (tarea 2) instruye al usuario crearlo; el repo NO incluye el keystore. No obstante, el agente implementa el soporte en Gradle:

- Editar `syncfiles-android/app/build.gradle.kts`:
  - Añadir bloque `signingConfigs` que lea `SF_KEYSTORE_FILE`, `SF_KEYSTORE_PASSWORD`, `SF_KEY_ALIAS`, `SF_KEY_PASSWORD` desde `project.findProperty(...)` (o valores vacíos por defecto).
  - En `buildTypes.release` asignar `signingConfig = signingConfigs.getByName("release")` solo si las propiedades existen; si no, `assembleRelease` produce el APK sin firmar (comportamiento actual) y el manual lo indica.
- Editar `syncfiles-android/gradle.properties` NO: las contraseñas van en `~/.gradle/gradle.properties` o en `syncfiles-android/gradle.properties.local` (documentar en manual y añadir al `.gitignore` si se usa archivo local).
- Añadir a `.gitignore`: `/syncfiles-android/gradle.properties.local` y `/syncfiles-android/*.jks` (defensivo).

**1.2. Verificación**: `./gradlew assembleDebug` desde `syncfiles-android/` debe seguir compilando (ya compila según plan.md §5b). `assembleRelease` debe funcionar con y sin propiedades de firma presentes.

### 2. Escribir el manual de instalación en `docs/48-documentacion-usuario.md`

Estructura del manual (en español, estilo de los demás docs, con bloques de código bash):

**2.1. Prerrequisitos generales**
- Rust (rustup, `cargo`), Android SDK + JDK 17 (para APK), curl opcional.
- Plataforma soportada: macOS (dev actual); indicar equivalentes Linux.

**2.2. Servidor**
- Compilar: `cargo build --release -p syncfiles-server` (desde raíz del repo).
- Variables de entorno con tabla: `SF_BIND_ADDRESS` (default `127.0.0.1:8080`), `SF_SERVER_URL`, `SF_DATABASE_URL` (default `sqlite:data/syncfiles.db`), `SF_STORAGE_ROOT` (default `data/storage`), usuarios `SF_USER_{i}_EMAIL/_PASSWORD/_ID`.
- Cuenta demo por defecto: `admin@syncfiles.local / syncfiles` (advertir: solo para pruebas).
- Aviso de CWD: DB y storage relativos al directorio de lanzamiento (lección de plan.md §5b.1); recomendar lanzar desde un directorio fijo o usar rutas absolutas.
- Para LAN: lanzar con `SF_BIND_ADDRESS=0.0.0.0:8080` y cortafuegos permitido.

**2.3. Dashboard web**
- Servido por el propio server en `http://<host>:8080/` (también `/ui` → redirect). Login con cuenta del servidor. Sin build step adicional.
- Requisitos desde otros dispositivos: CORS ya activo (permissive).

**2.4. Cliente desktop (macOS/Linux)**
- Compilar: `cargo build --release -p syncfiles-client`.
- Ejecutar UI: `SF_SERVER_URL` y `SF_SYNC_ROOT` opcionales; `cargo run --bin syncfiles-ui`.
- Primer uso: login → elegir carpeta → sincronización automática; "Sincronizar ahora".

**2.5. Cliente CLI**
- Compilar: `cargo build --release -p syncfiles-cli`.
- Subcomandos esenciales (login, upload, diff, download, delete) con ejemplo de sesión real contra el server.
- Nota: es la herramienta usada por `scripts/e2e-test.sh` / `e2e-phase5.sh`.

**2.6. App Android — compilación del APK**
- Requisitos: Android SDK (API 34), JDK 17, `local.properties` con `sdk.dir` (Android Studio lo genera).
- **APK debug** (instalable en propio teléfono): `./gradlew assembleDebug` → `app/build/outputs/apk/debug/app-debug.apk`.
- **APK release firmado**:
  1. Crear keystore: `keytool -genkeypair -v -keystore syncfiles-release.jks -keyalg RSA -keysize 2048 -validity 10000 -alias syncfiles` (documentar respaldo: perderlo impide actualizar la app).
  2. Definir en `~/.gradle/gradle.properties` (o `gradle.properties.local`): `SF_KEYSTORE_FILE`, `SF_KEYSTORE_PASSWORD`, `SF_KEY_ALIAS`, `SF_KEY_PASSWORD` con las claves que leyó la tarea 1.
  3. `./gradlew assembleRelease` → `app/build/outputs/apk/release/app-release.apk`.
  4. Verificar firma: `apksigner verify --print-certs app-release.apk`.
- **Instalación**: `adb install -r <apk>` (USB debug) o copiar el APK y abrirlo (habilitar "orígenes desconocidos"). Nota: desinstalar la versión debug antes de instalar la release (firma distinta → conflicto de firma).

**2.7. Configuración de la app Android**
- Ajustes: URL del servidor, carpeta (SAF), intervalo de sync (15 min–6 h), logout.
- **Emulador**: URL `http://10.0.2.2:8080` (ya permitido en `network_security_config.xml`).
- **Teléfono físico en LAN** (sección destacada): la app bloquea HTTP a dominios no listados.
  - Opción A (recomendada para producción): servidor con HTTPS (reverso tipo Caddy/nginx) — sin cambios.
  - Opción B (pruebas en LAN): editar `syncfiles-android/app/src/main/res/xml/network_security_config.xml`, añadir `<domain includeSubdomains="false">IP_DE_TU_MAC</domain>` dentro del `domain-config` con `cleartextTrafficPermitted="true"`, recompilar el APK. Advertir: nunca permitir cleartext en `base-config`; la IP de la Mac puede cambiar (DHCP) → reservar IP fija o usar hostname local. En la app usar `http://<ip-mac>:8080` con `SF_BIND_ADDRESS=0.0.0.0:8080` en el server.
- Primer uso: login → elegir carpeta → el WorkManager sincroniza en background; estado visible en Home.

**2.8. Solución de problemas** (tabla breve)
- Login falla → credenciales demo vs `SF_USER_*`, URL malformada.
- "Error de sincronización HTTP 400" en pull → storage vs DB en CWDs distintos (plan.md §5b.1): lanzar server siempre desde el mismo directorio.
- App no conecta desde teléfono → cleartext bloqueado (ver 2.7), server no escucha en LAN (`SF_BIND_ADDRESS=0.0.0.0`), cortafuegos, misma red Wi-Fi.
- `adb` no detecta dispositivo → depuración USB, `adb kill-server && adb start-server`.

### 3. Actualizar README.md

- Añadir en "Estructura de Documentación" la mención ya existente de doc 48 (ya está listada) y una sección corta "Instalación" que enlace a `docs/48-documentacion-usuario.md` con el resumen de un comando por componente (server/web, desktop, CLI, APK).

## Archivos afectados

| Archivo | Cambio |
|---|---|
| `docs/48-documentacion-usuario.md` | Manual completo (reemplaza el stub de 21 líneas) |
| `syncfiles-android/app/build.gradle.kts` | signingConfig release condicional (tarea 1.1) |
| `.gitignore` | `gradle.properties.local`, `*.jks` defensivo |
| `README.md` | Sección "Instalación" con enlace al manual |

## Riesgos y notas

- El keystore y sus contraseñas NUNCA van al repo (ya cubierto por `.gitignore`; el manual debe decirlo explícitamente).
- `versionCode = 1 / versionName = 0.1.0` en `defaultConfig`: el manual debe recordar incrementar `versionCode` en cada release para que `adb install -r`/Play acepten la actualización.
- El manual no inventa comandos inexistentes: verificar contra código real (`config.rs` vars, `gradlew` tasks, subcomandos del CLI leyendo `syncfiles-cli/src/main.rs`) antes de documentar.
- No se cambia `network_security_config.xml` en esta iteración; solo se documenta cómo (decisión del usuario).

## Validación

1. `cargo build --release -p syncfiles-server -p syncfiles-client -p syncfiles-cli` compila sin warnings nuevos.
2. `./gradlew assembleDebug` (desde `syncfiles-android/`) exitoso.
3. `./gradlew assembleRelease` exitoso sin propiedades de firma (APK sin firmar, advertido en manual).
4. Con keystore creado según el manual: `assembleRelease` produce APK firmado y `apksigner verify` imprime el certificado.
5. `bash scripts/e2e-test.sh` sigue en verde (regresión de server/cliente).
6. Revisar que cada comando del manual exista tal cual (sin placeholders rotos) y que las variables de entorno coincidan con `syncfiles-server/src/config.rs`.
