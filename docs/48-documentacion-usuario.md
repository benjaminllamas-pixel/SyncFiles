# Documentación de Usuario: Instalación y Primer Uso

Manual de instalación y uso de los 4 componentes entregables de SyncFiles V1:

| Componente | Tecnología | Entregable |
|---|---|---|
| Servidor + dashboard web | Rust (actix-web) | binario `syncfiles-server` + `static/` servido en `/` |
| Cliente desktop | Rust (egui) | binario `syncfiles-client` |
| Cliente CLI | Rust | binario `syncfiles-cli` |
| App Android | Kotlin (Jetpack Compose) | APK (`app-debug.apk` / `app-release.apk`) |

## 1. Prerrequisitos generales

### 1.1. Rust (server, desktop y CLI)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable
```

Verificar: `cargo --version`.

### 1.2. Android (solo para el APK)

- **Android SDK** con platform API 34 y build-tools 34.x (vía Android Studio, o `sdkmanager "platforms;android-34" "build-tools;34.0.0"`).
- **JDK 17**. El archivo `syncfiles-android/gradle.properties` fija `org.gradle.java.home=/opt/homebrew/opt/openjdk@17` (Homebrew en macOS; ajústalo a tu ruta de JDK 17 si difiere).
- **`local.properties`** con `sdk.dir=/ruta/al/Android/sdk` (Android Studio lo genera automáticamente; también puedes crearlo a mano).

### 1.3. Opcional

- `curl` (pruebas rápidas contra la API), `adb` (plataform-tools), `keytool` y `apksigner` (build-tools; vienen con el SDK).

### 1.4. Plataforma

Los comandos están probados en **macOS** (desarrollo actual). En Linux los equivalentes son los mismos salvo rutas (`~` → `$HOME`) y gestor de paquetes. El repo **no tiene workspace de Cargo en la raíz**: cada componente Rust se compila desde su propio directorio o con `--manifest-path`.

## 2. Servidor

### 2.1. Compilar

Desde la raíz del repo:

```bash
cargo build --release --manifest-path syncfiles-server/Cargo.toml
```

El binario queda en `syncfiles-server/target/release/syncfiles-server`.

### 2.2. Configuración (variables de entorno)

El servidor lee `.env` si existe (`dotenvy`) y variables de entorno:

| Variable | Default | Descripción |
|---|---|---|
| `SF_BIND_ADDRESS` | `127.0.0.1:8080` | Dirección de escucha. Usar `0.0.0.0:8080` para aceptar conexiones LAN |
| `SF_SERVER_URL` | `http://127.0.0.1:8080` | URL pública del server (se devuelve en respuestas/cola) |
| `SF_DATABASE_URL` | `sqlite:data/syncfiles.db` | Ruta de la DB SQLite. **Relativa al CWD de lanzamiento** |
| `SF_STORAGE_ROOT` | `data/storage` | Carpeta de blobs. **Relativa al CWD de lanzamiento** |
| `SF_USER_{i}_EMAIL` / `_PASSWORD` / `_ID` | — | Usuarios estáticos (i = 0, 1, 2…); las contraseñas se hashean con bcrypt al arrancar |
| `SF_STATIC_DIR` | `./static` (fallback `./syncfiles-server/static`) | Carpeta del dashboard web |

Si no se define ningún `SF_USER_*`, se crea la **cuenta demo**:

```
admin@syncfiles.local / syncfiles
```

> ⚠️ Solo para pruebas. Para uso real define `SF_USER_0_*` (o más usuarios) con contraseñas fuertes.

### 2.3. Lanzar

```bash
cd syncfiles-server
cargo run --release
```

> ⚠️ **Importante — CWD**: `SF_DATABASE_URL` y `SF_STORAGE_ROOT` por defecto son rutas relativas al directorio desde el que lanzas el proceso. Si hoy lanzas el server desde `syncfiles-server/` y mañana desde la raíz del repo, la DB y el storage apuntarán a carpetas distintas: el diff listará archivos de una DB pero el download buscará el contenido en otro storage y fallará con HTTP 400 "Contenido no encontrado en storage". Lanza siempre desde el mismo directorio (recomendado: `syncfiles-server/`) o usa rutas absolutas en ambas variables.

### 2.4. Exponerlo en la LAN

```bash
SF_BIND_ADDRESS=0.0.0.0:8080 SF_SERVER_URL=http://192.168.1.50:8080 ./target/release/syncfiles-server
```

Permite el puerto 8080 en el cortafuegos (macOS: "Cortafuegos" en Ajustes; Linux: `ufw allow 8080`). La IP la puedes consultar con `ifconfig | grep "inet "` (macOS) o `ip addr` (Linux).

## 3. Dashboard web

El propio servidor sirve el dashboard en `http://<host>:8080/` (y `/ui` redirige a `/`). No hay build step adicional: los archivos `static/index.html`, `static/app.js` y `static/styles.css` se sirven tal cual (con cabeceras no-cache en desarrollo).

Funciones: login/logout con una cuenta del servidor, dashboard con estadísticas y dispositivos, subir/renombrar/mover/copiar/borrar/descargar archivos, vista de cola de sincronización, resolución de conflictos, actividad y revocación de dispositivos.

- Desde el mismo equipo: `http://127.0.0.1:8080/`.
- Desde otros dispositivos de la LAN: `http://<ip-del-servidor>:8080/` (con `SF_BIND_ADDRESS=0.0.0.0:8080`).
- CORS ya está activo (permissive), por lo que el dashboard también funciona servido desde otro origen si lo copias manualmente.

## 4. Cliente desktop (macOS/Linux)

### 4.1. Compilar

```bash
cargo build --release --manifest-path syncfiles-client/Cargo.toml
```

El binario queda en `syncfiles-client/target/release/syncfiles-client`.

### 4.2. Ejecutar

```bash
cd syncfiles-client
cargo run --release
```

Variables opcionales en el primer arranque (después se guardan en `~/Library/Application Support/syncfiles/config.json` en macOS, `~/.local/share/syncfiles/` en Linux):

| Variable | Default |
|---|---|
| `SF_SERVER_URL` | `http://127.0.0.1:8080` |
| `SF_EMAIL` / `SF_PASSWORD` | vacío (se piden en el login de la UI) |
| `SF_SYNC_ROOT` | `~/SyncFiles` |
| `SF_POLLING_INTERVAL` | `30` (segundos) |

### 4.3. Primer uso

1. En la pantalla de login introduce servidor (si no es el default), email y contraseña (p. ej. la cuenta demo del server).
2. Elige la carpeta de sincronización con el botón "Elegir..." (por defecto `~/SyncFiles`, se crea si no existe).
3. La sincronización automática arranca sola (watcher + polling); "Sincronizar ahora" fuerza un ciclo manual.

## 5. Cliente CLI

### 5.1. Compilar

```bash
cargo build --release --manifest-path syncfiles-cli/Cargo.toml
```

Binario: `syncfiles-cli/target/release/syncfiles-cli`.

### 5.2. Subcomandos

Estilo: `syncfiles-cli <comando> [args]`. Conexión vía env (con defaults): `SF_SERVER_URL` (`http://127.0.0.1:8080`), `SF_EMAIL` (`admin@syncfiles.local`), `SF_PASSWORD` (`syncfiles`), `SF_DEVICE_ID` (`cli-test`).

| Comando | Uso |
|---|---|
| `login` | Valida credenciales y devuelve `session_id` |
| `upload <archivo>` | Sube el archivo (usa su nombre como ruta relativa) |
| `download <file_id> <destino>` | Descarga el contenido a `<destino>` |
| `delete <file_id>` | Marca el archivo como borrado |
| `rename <file_id> <nuevo-nombre>` | Renombra en el servidor |
| `diff <since_ms>` | Lista cambios desde un timestamp (0 = todo) |
| `session-status` | Estado de la sesión actual |
| `logout` | Cierra la sesión |

### 5.3. Ejemplo de sesión real

Con el servidor corriendo (sección 2):

```bash
CLI=syncfiles-cli/target/release/syncfiles-cli

$CLI login
# {"status":"ok","session_id":"...","email":"admin@syncfiles.local",...}

echo "hola" > /tmp/prueba.txt
$CLI upload /tmp/prueba.txt
# {"status":"ok",...}

$CLI diff 0
# {"changes":[{"file_id":"...","relative_path":"prueba.txt",...}]}
# (copia el file_id del resultado)

$CLI download <file_id> /tmp/bajada.txt
# Saved 5 bytes to /tmp/bajada.txt

$CLI delete <file_id>
```

> Nota: el CLI hace login interno en cada comando; también existe un estilo legacy posicional (`<url> <email> <password> <device> <cmd>`) heredado de las primeras fases, pero se recomienda el estilo subcomando. Es la herramienta usada por `scripts/e2e-test.sh` y `scripts/e2e-phase5.sh`.

## 6. App Android — compilación del APK

### 6.1. Requisitos

Sección 1.2: Android SDK (API 34), JDK 17 y `local.properties` con `sdk.dir`. Verificar con:

```bash
cd syncfiles-android
./gradlew --version   # debe mostrar JVM 17
```

### 6.2. APK debug (instalable en tu propio teléfono)

```bash
./gradlew assembleDebug
```

Salida: `app/build/outputs/apk/debug/app-debug.apk`. Está firmado con la clave debug autogenerada de Android: sirve para pruebas y desarrollo.

### 6.3. APK release firmado

`assembleRelease` **sin** propiedades de firma produce `app-release-unsigned.apk` (no instalable tal cual). Para firmar:

**Paso 1 — Crear el keystore** (una sola vez):

```bash
keytool -genkeypair -v -keystore syncfiles-release.jks \
  -keyalg RSA -keysize 2048 -validity 10000 -alias syncfiles
```

> ⚠️ **Guarda este archivo y sus contraseñas en un lugar seguro y haz respaldo.** Si lo pierdes no podrás publicar actualizaciones de la misma app (Android rechaza actualizaciones firmadas con otra clave). El keystore **nunca se sube al repo** (está en `.gitignore`).

**Paso 2 — Definir las propiedades de firma** en `~/.gradle/gradle.properties` (global, recomendado) o en `syncfiles-android/gradle.properties.local` (local al proyecto; también ignorado por git):

```properties
SF_KEYSTORE_FILE=/ruta/absoluta/a/syncfiles-release.jks
SF_KEYSTORE_PASSWORD=tu_password_del_keystore
SF_KEYSTORE_ALIAS=syncfiles
SF_KEY_PASSWORD=tu_password_de_la_clave
```

**Paso 3 — Compilar**:

```bash
./gradlew assembleRelease
```

Salida: `app/build/outputs/apk/release/app-release.apk` (firmado).

**Paso 4 — Verificar la firma**:

```bash
$HOME/Library/Android/sdk/build-tools/34.0.0/apksigner verify --print-certs \
  app/build/outputs/apk/release/app-release.apk
```

(La ruta corresponde a macOS con el SDK en `~/Library/Android/sdk`; en Linux suele ser `$ANDROID_HOME/build-tools/34.0.0/apksigner`.)

> 📌 **versionCode**: en cada release incrementa `versionCode` (y `versionName`) en `syncfiles-android/app/build.gradle.kts`; Android exige un código mayor que el instalado para actualizar (`adb install -r` incluido).

### 6.4. Instalación en el teléfono

- **USB (adb)**: activa "Depuración USB" en Opciones de desarrollador y ejecuta `adb install -r app/build/outputs/apk/<debug|release>/app-*.apk`. Si `adb` no detecta el dispositivo: revisa el cable/depuración USB y prueba `adb kill-server && adb start-server`.
- **Manual**: copia el APK al teléfono (cable, Drive…) y ábrelo; tendrás que habilitar "Instalar apps desconocidas" para esa fuente.
- ⚠️ **Conflicto de firmas**: la APK debug usa una firma distinta a la release. Antes de instalar la release sobre una instalación debug, desinstala primero la app (`adb uninstall com.syncfiles.client.android` o desde el launcher).

## 7. Configuración de la app Android

En **Ajustes** dentro de la app: URL del servidor, carpeta de sincronización (selector SAF con `OpenDocumentTree`), intervalo de sincronización (15 min, 30 min, 1 h, 6 h) y logout.

Primer uso: login con una cuenta del servidor → elegir carpeta → el WorkManager sincroniza en segundo plano según el intervalo; el estado se ve en Home (y en Archivos/Conflictos).

### 7.1. Emulador

La URL del servidor es `http://10.0.2.2:8080` (alias del host desde el emulador). Ya está permitido por `network_security_config.xml` (ver 7.2).

### 7.2. Teléfono físico en la LAN ⚠️

Android bloquea tráfico HTTP plano (cleartext) hacia dominios/IPs no listados. La app solo permite cleartext hacia `10.0.2.2`, `127.0.0.1` y `localhost` (`syncfiles-android/app/src/main/res/xml/network_security_config.xml`). Opciones:

- **Opción A — HTTPS (recomendada para producción)**: pon el servidor detrás de un proxy inverso con TLS (Caddy, nginx) y usa `https://tu-dominio` en la app. Sin cambios de código.
- **Opción B — pruebas en LAN con HTTP**: edita `network_security_config.xml` añadiendo la IP de tu Mac dentro del `domain-config` existente:

```xml
<domain-config cleartextTrafficPermitted="true">
    <domain includeSubdomains="true">10.0.2.2</domain>
    <domain includeSubdomains="true">127.0.0.1</domain>
    <domain includeSubdomains="true">localhost</domain>
    <!-- Añade la IP de tu equipo: -->
    <domain includeSubdomains="false">192.168.1.50</domain>
</domain-config>
```

  y recompila el APK (`./gradlew assembleDebug`). Luego, en la app, usa `http://192.168.1.50:8080` con el servidor lanzado con `SF_BIND_ADDRESS=0.0.0.0:8080` (sección 2.4).

  Advertencias:
  - **Nunca** pongas `cleartextTrafficPermitted="true"` en el `base-config` (ese flag afecta a todo el tráfico de la app).
  - La IP local puede cambiar por DHCP: reserva una IP fija en el router o usa un hostname local (`mimac.local`).
  - Esta opción es solo para pruebas; para uso continuo usa la Opción A.

## 8. Solución de problemas

| Síntoma | Causa probable | Solución |
|---|---|---|
| Login falla | Credenciales incorrectas o URL malformada | ¿Definiste `SF_USER_*` en el server? Si no, la demo es `admin@syncfiles.local / syncfiles`. Revisa que la URL no tenga barra final ni `https` por error |
| "Error de sincronización: HTTP 400" al descargar en Android | Storage y DB en CWDs distintos | El server fue lanzado alguna vez desde otro directorio y la DB/storage quedaron separados. Lanza siempre desde el mismo CWD o usa rutas absolutas en `SF_DATABASE_URL`/`SF_STORAGE_ROOT` (sección 2.3) |
| La app Android no conecta desde un teléfono físico | Cleartext bloqueado, server no escucha en LAN, cortafuegos o distinta red Wi-Fi | Ver sección 7.2 (IP en `network_security_config.xml`), `SF_BIND_ADDRESS=0.0.0.0:8080`, cortafuegos (2.4) y que ambos estén en la misma red |
| `adb` no detecta el dispositivo | Depuración USB desactivada o servidor adb colgado | Activa "Depuración USB"; `adb kill-server && adb start-server` |
| `assembleRelease` produce `app-release-unsigned.apk` | Faltan propiedades de firma | Define las 4 propiedades `SF_KEYSTORE_*` (sección 6.3) en `~/.gradle/gradle.properties` o `gradle.properties.local` |
| No puedo actualizar la app release ya instalada | `versionCode` no incrementado | Sube `versionCode` en `app/build.gradle.kts` antes de compilar la nueva release |
| El dashboard no se carga | Carpeta `static/` no encontrada | Lanza el server desde `syncfiles-server/`, o define `SF_STATIC_DIR` con la ruta a `static/` (el log avisa al arrancar) |
