# GCP Docker Deploy + Release Packaging

Plan generado: 2026-10-14. Objetivo: generar instalador para Mac, APK Android release, imagen Docker offline, y desplegar en GCP free tier (e2-micro).

---

## 0. Pre-Roadmap: Packaging y Generación de Artefactos

**Objetivo**: Generar todos los artefactos necesarios para despliegue, sin requerir compilación en la VM.

### Paso 0.1: Build de imagen Docker (en tu máquina local)

```bash
# 1. Build imagen y exportar como archivo tar (para usar offline en GCP)
cd /Users/firebender/Documents/SyncFiles\ Project
docker compose build --no-cache

# 2. Exportar como tar.gz (opcional si no usas registry)
docker save syncfiles-server:local | gzip > syncfiles-server-v0.1.0.tar.gz

# 3. O subir a GHCR (recomendado)
docker tag syncfiles-server:local ghcr.io/TU_USUARIO/syncfiles-server:v0.1.0
docker push ghcr.io/TU_USUARIO/syncfiles-server:v0.1.0
```

**Entradas/salidas esperadas:**
| Acción | Input | Output | Verificación |
|--------|-------|--------|--------------|
| `docker compose build` | `Dockerfile` + código fuente | Imagen `syncfiles-server:local` | `docker images syncfiles-server` |
| `docker save` | Imagen local | `syncfiles-server-v0.1.0.tar.gz` | `ls -la *.tar.gz` (≈10-15 MB) |
| `docker push` | Imagen local | Imagen en `ghcr.io` | `docker pull ghcr.io/...` |

### Paso 0.2: Generar APK Android (release firmado)

```bash
cd /Users/firebender/Documents/SyncFiles\ Project/syncfiles-android

# 1. Generar keystore (UNA VEZ) — script en §8
chmod +x generate-keystore.sh && ./generate-keystore.sh
# → syncfiles-release.keystore (GUARDAR EN GESTOR CONTRASEÑAS)

# 2. Configurar gradle.properties con SF_KEYSTORE_* props (ver §7)
#    Añade estas líneas a gradle.properties (NO commitear):
#    SF_KEYSTORE_FILE=syncfiles-release.keystore
#    SF_KEYSTORE_PASSWORD=TU_PASSWORD
#    SF_KEYSTORE_ALIAS=syncfiles
#    SF_KEY_PASSWORD=TU_PASSWORD

# 3. Build APK release
./gradlew assembleRelease
# Output: app/build/outputs/apk/release/app-release.apk
```

**Entradas/salidas esperadas:**
| Acción | Input | Output | Verificación |
|--------|-------|--------|--------------|
| `generate-keystore.sh` | keytool en Java | `syncfiles-release.keystore` | `ls -la *.keystore` |
| `gradle.properties` | Editor | Config signing | `cat gradle.properties` |
| `./gradlew assembleRelease` | Keystore + código | `app-release.apk` | `ls -la app/build/outputs/apk/release/app-release.apk` |

### Paso 0.3: Generar instalador para Mac (CLI)

```bash
cd /Users/firebender/Documents/SyncFiles\ Project/syncfiles-cli

# 1. Build release binary
cargo build --release

# 2. Crear instalador .pkg o .dmg (opcional, si existe script)
# El binary está en: target/release/syncfiles-cli

# 3. Crear DMG (usando create-dmg si está disponible, o manualmente)
# Opción sin herramienta externa: zip con instrucciones
zip -j syncfiles-cli-macos-arm64.zip target/release/syncfiles-cli
```

**Entradas/salidas esperadas:**
| Acción | Input | Output | Verificación |
|--------|-------|--------|--------------|
| `cargo build --release` | Código Rust + CLI | `target/release/syncfiles-cli` | `file target/release/syncfiles-cli` |
| `zip` (opcional .pkg) | Binary | `syncfiles-cli-macos-arm64.zip` | `ls -la *.zip` |

### Paso 0.4: Preparar archivo Docker offline

```bash
# Opción A: Imagen como tar.gz (para transferir a GCP sin registry)
docker save syncfiles-server:local | gzip > syncfiles-server-v0.1.0.tar.gz

# Opción B: Subir a registry (recomendado)
docker push ghcr.io/TU_USUARIO/syncfiles-server:v0.1.0
```

**Entradas/salidas esperadas:**
| Acción | Input | Output | Verificación |
|--------|-------|--------|--------------|
| `docker save` | Imagen local | `syncfiles-server-v0.1.0.tar.gz` | `ls -lh *.tar.gz` (≈10-15MB) |
| `docker push` | Imagen local | Tag en registry | `docker pull ghcr.io/...` |

---

## 1. GCP Free Tier Setup (Pre-requisitos)

### 1.1 Instalar gcloud CLI (macOS)

```bash
brew install google-cloud-sdk
gcloud auth login
gcloud config set project syncfiles-prod-XXXX  # AJUSTA al ID de tu proyecto GCP
gcloud services enable compute.googleapis.com artifactregistry.googleapis.com
```

### 1.2 Reglas de oro del Free Tier (LEER ANTES DE CREAR LA VM)

| # | Regla | Detalle |
|---|-------|---------|
| 1 | **Solo 1 VM e2-micro** | El free tier da ~744 horas/mes. 1 VM = 720h (24/7). Si creas 2 VMs, el segundo consume el resto del crédito y se paga el resto del mes. **Crea solo 1 VM.** |
| 2 | **Regiones elegibles** | El free tier de e2-micro **solo aplica** en: `us-central1` (Iowa), `us-east1` (South Carolina), `us-west1` (Oregon). No usar `us-west2` (LA), `us-east4`, ni regiones europeas/latinas. |
| 3 | **Disco HDD estándar** | El disco por defecto en GCP Console es "Disco persistente balanceado" (SSD) y **tiene costo**. Debes cambiar a "Disco persistente estándar (HDD)" que sí está en free tier (5 GB/mes). |

### 1.3 Facturación gratuita confirmada

| Recurso GCP | Gratis | Límite mensual |
|-------------|--------|----------------|
| **e2-micro** (1 vCPU, 1 GB RAM) | ✅ Siempre gratuito | 744 horas (1 VM 24/7) |
| **Disk PD Standard (HDD)** | ✅ Gratis | 5 GB al mes (usa 30 GB ≈ $1.07, dentro del $300 free credit) |
| **IP estática externa** | ✅ Gratis | Mientras la IP está en uso |
| **Network egress** | ❌ Pago | 1 GB gratis/día (~$0.01/GB extra) |
| **Cloud Storage** | ✅ Gratis | 5 GB estándar |

**Costo estimado mensual (e2-micro 24/7 + 30GB HDD):**
- e2-micro: $0 (free tier)
- 30 GB HDD: ~$1.07 (dentro del free credit de $300)
- IP: $0
- **Total: ~$1.07/mes** (gratis el primer mes, luego ~$1/mes hasta agotar $300)

---

## 2. `.env` (raíz del proyecto) — **NO commitear**

```bash
# ============================================================
# SyncFiles Production Environment
# ============================================================

# --- Server ---
SF_BIND_ADDRESS=0.0.0.0:8080
SF_SERVER_URL=https://TU_DOMINIO.COM
SF_MAX_UPLOAD_BYTES=104857600
SF_RATE_LIMIT_RPS=60
SF_RATE_LIMIT_BURST=120

# --- Admin token ---
SF_ADMIN_TOKEN=cambia-esto-por-un-token-seguro-64-chars

# --- Usuario inicial ---
SF_USER_0_EMAIL=admin@tudominio.com
SF_USER_0_PASSWORD=password-super-seguro-aqui
SF_USER_0_ID=user-001

# --- Google Drive Backup ---
BACKUP_GDRIVE_ENABLED=true
BACKUP_GDRIVE_REMOTE=gdrive
BACKUP_GDRIVE_PATH=SyncFilesBackups
BACKUP_GDRIVE_ARGS="--bwlimit 1M --progress"

# --- Logs ---
RUST_LOG=info

# --- Docker Compose ---
SF_HOST_PORT=8080
SF_ADMINER_PORT=8081
```

---

## 3. `docker-compose.yml` (actualizado para producción)

```yaml
name: syncfiles

services:
  server:
    image: ghcr.io/TU_USUARIO/syncfiles-server:v0.1.0
    # Si no usas registry, comenta 'image:' y usa 'build:' abajo:
    # build:
    #   context: .
    #   dockerfile: Dockerfile
    init: true
    restart: unless-stopped
    profiles: ["default", "prod"]
    environment:
      SF_BIND_ADDRESS: "0.0.0.0:8080"
      SF_MIGRATIONS_DIR: "/app/migrations"
      SF_STATIC_DIR: "/app/static"
      SF_DATABASE_URL: "sqlite:/data/db/syncfiles.db"
      SF_SERVER_URL: "${SF_SERVER_URL:-http://localhost:8080}"
      SF_STORAGE_ROOT: "/data/storage"
      SF_MAX_UPLOAD_BYTES: "${SF_MAX_UPLOAD_BYTES:-104857600}"
      SF_RATE_LIMIT_RPS: "${SF_RATE_LIMIT_RPS:-60}"
      SF_RATE_LIMIT_BURST: "${SF_RATE_LIMIT_BURST:-120}"
      BACKUP_DIR: "/data/backups"
      SF_BACKUP_LOG: "/data/backup.log"
      SF_ADMIN_TOKEN: "${SF_ADMIN_TOKEN:-}"
      SF_USER_0_EMAIL: "${SF_USER_0_EMAIL:-admin@syncfiles.local}"
      SF_USER_0_PASSWORD: "${SF_USER_0_PASSWORD:-syncfiles}"
      SF_USER_0_ID: "${SF_USER_0_ID:-user-001}"
      RUST_LOG: "${RUST_LOG:-info}"
    healthcheck:
      test: ["CMD", "curl", "-sf", "http://localhost:8080/health/live"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 15s
    volumes:
      - sf-data:/data
    ports:
      - "${SF_HOST_PORT:-8080}:8080"
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"

  nginx:
    image: nginx:alpine
    restart: unless-stopped
    profiles: ["prod"]
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
      - ./certbot/conf:/etc/letsencrypt:ro
      - ./certbot/www:/var/www/certbot:ro
    depends_on:
      server:
        condition: service_healthy
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"

  backup:
    image: ghcr.io/TU_USUARIO/syncfiles-server:v0.1.0
    # build:
    #   context: .
    #   dockerfile: Dockerfile
    init: true
    profiles: ["default"]
    entrypoint: ["/bin/bash", "/scripts/backup.sh"]
    environment:
      SF_DATABASE_URL: "sqlite:/data/db/syncfiles.db"
      SF_STORAGE_ROOT: "/data/storage"
      BACKUP_DIR: "/data/backups"
      SF_BACKUP_LOG: "/data/backup.log"
      BACKUP_GDRIVE_ENABLED: "${BACKUP_GDRIVE_ENABLED:-false}"
      BACKUP_GDRIVE_REMOTE: "${BACKUP_GDRIVE_REMOTE:-gdrive}"
      BACKUP_GDRIVE_PATH: "${BACKUP_GDRIVE_PATH:-SyncFilesBackups}"
      BACKUP_GDRIVE_ARGS: "${BACKUP_GDRIVE_ARGS:-}"
      RCLONE_CONFIG: "/config/rclone/rclone.conf"
    depends_on:
      server:
        condition: service_healthy
    volumes:
      - sf-data:/data
      - ~/.config/rclone:/config/rclone:ro
    restart: "no"

  backup-cron:
    image: ghcr.io/TU_USUARIO/syncfiles-server:v0.1.0
    # build:
    #   context: .
    #   dockerfile: Dockerfile
    init: true
    profiles: ["cron"]
    entrypoint: ["/bin/bash", "-c"]
    command:
      - |
        while true; do
          /scripts/backup.sh || true
          sleep 86400
        done
    environment:
      SF_DATABASE_URL: "sqlite:/data/db/syncfiles.db"
      SF_STORAGE_ROOT: "/data/storage"
      BACKUP_DIR: "/data/backups"
      SF_BACKUP_LOG: "/data/backup.log"
      BACKUP_GDRIVE_ENABLED: "${BACKUP_GDRIVE_ENABLED:-false}"
      BACKUP_GDRIVE_REMOTE: "${BACKUP_GDRIVE_REMOTE:-gdrive}"
      BACKUP_GDRIVE_PATH: "${BACKUP_GDRIVE_PATH:-SyncFilesBackups}"
      BACKUP_GDRIVE_ARGS: "${BACKUP_GDRIVE_ARGS:-}"
      RCLONE_CONFIG: "/config/rclone/rclone.conf"
    depends_on:
      server:
        condition: service_healthy
    volumes:
      - sf-data:/data
      - ~/.config/rclone:/config/rclone:ro
    restart: unless-stopped

  adminer:
    image: adminer:latest
    profiles: ["dev"]
    restart: unless-stopped
    depends_on:
      server:
        condition: service_healthy
    ports:
      - "${SF_ADMINER_PORT:-8081}:8080"

volumes:
  sf-data:
```

---

## 4. `nginx.conf` (reverse proxy + TLS)

```nginx
# nginx.conf
# Coloca en la raíz del proyecto junto a docker-compose.yml

user nginx;
worker_processes auto;
error_log /var/log/nginx/error.log warn;
pid /var/run/nginx.pid;

events {
    worker_connections 1024;
    use epoll;
    multi_accept on;
}

http {
    include /etc/nginx/mime.types;
    default_type application/octet-stream;

    # Logging
    log_format main '$remote_addr - $remote_user [$time_local] "$request" '
                    '$status $body_bytes_sent "$http_referer" '
                    '"$http_user_agent" "$http_x_forwarded_for"';
    access_log /var/log/nginx/access.log main;

    # Performance
    sendfile on;
    tcp_nopush on;
    tcp_nodelay on;
    keepalive_timeout 65;
    types_hash_max_size 2048;
    client_max_body_size 100M;

    # Gzip
    gzip on;
    gzip_vary on;
    gzip_min_length 1024;
    gzip_types text/plain text/css text/xml text/javascript application/javascript application/xml application/json;

    # Upstream SyncFiles server
    upstream syncfiles_backend {
        server server:8080;
        keepalive 32;
    }

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=api:10m rate=60r/s;
    limit_req_zone $binary_remote_addr zone=login:10m rate=5r/s;

    # HTTP -> HTTPS redirect
    server {
        listen 80;
        server_name TU_DOMINIO.COM;

        # ACME challenge para Let's Encrypt
        location /.well-known/acme-challenge/ {
            root /var/www/certbot;
            try_files $uri =404;
        }

        # Redirect todo lo demás a HTTPS
        location / {
            return 301 https://$host$request_uri;
        }
    }

    # HTTPS server
    server {
        listen 443 ssl http2;
        server_name TU_DOMINIO.COM;

        # SSL certificates (Let's Encrypt)
        ssl_certificate /etc/letsencrypt/live/TU_DOMINIO.COM/fullchain.pem;
        ssl_certificate_key /etc/letsencrypt/live/TU_DOMINIO.COM/privkey.pem;
        ssl_trusted_certificate /etc/letsencrypt/live/TU_DOMINIO.COM/chain.pem;

        # SSL security
        ssl_protocols TLSv1.2 TLSv1.3;
        ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;
        ssl_prefer_server_ciphers off;
        ssl_session_cache shared:SSL:10m;
        ssl_session_timeout 10m;

        # HSTS
        add_header Strict-Transport-Security "max-age=31536000" always;

        # Security headers
        add_header X-Frame-Options DENY always;
        add_header X-Content-Type-Options nosniff always;
        add_header X-XSS-Protection "1; mode=block" always;
        add_header Referrer-Policy "strict-origin-when-cross-origin" always;

        # API rate limiting
        location /api/ {
            limit_req zone=api burst=120 nodelay;
            limit_req_status 429;

            proxy_pass http://syncfiles_backend;
            proxy_http_version 1.1;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
            proxy_set_header X-Forwarded-Host $host;
            proxy_set_header X-Forwarded-Port $server_port;

            # Timeouts
            proxy_connect_timeout 30s;
            proxy_send_timeout 60s;
            proxy_read_timeout 60s;

            # Buffering
            proxy_buffering on;
            proxy_buffer_size 4k;
            proxy_buffers 8 4k;
        }

        # Login rate limiting (más estricto)
        location /api/v1/auth/login {
            limit_req zone=login burst=10 nodelay;
            limit_req_status 429;

            proxy_pass http://syncfiles_backend;
            proxy_http_version 1.1;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
        }

        # WebSocket (si se usa en futuro)
        location /ws {
            proxy_pass http://syncfiles_backend;
            proxy_http_version 1.1;
            proxy_set_header Upgrade $http_upgrade;
            proxy_set_header Connection "upgrade";
            proxy_set_header Host $host;
            proxy_read_timeout 86400;
        }

        # Static files (dashboard) - served directly by nginx for performance
        location / {
            proxy_pass http://syncfiles_backend;
            proxy_http_version 1.1;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;

            # Cache static assets
            location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg|woff|woff2)$ {
                expires 1y;
                add_header Cache-Control "public, immutable";
                proxy_pass http://syncfiles_backend;
            }
        }

        # Health checks (sin rate limit)
        location /health/ {
            proxy_pass http://syncfiles_backend;
            access_log off;
        }
    }
}
```

---

## 5. `certbot-init.sh` (inicializar certificados Let's Encrypt)

```bash
#!/usr/bin/env bash
# certbot-init.sh — Ejecutar UNA VEZ en la VM para obtener certificado inicial
# Uso: chmod +x certbot-init.sh && ./certbot-init.sh

set -euo pipefail

DOMAIN="TU_DOMINIO.COM"
EMAIL="admin@tudominio.com"

echo "=== Inicializando certificado Let's Encrypt para $DOMAIN ==="

# 1. Crear directorios
mkdir -p certbot/conf certbot/www

# 2. Levantar nginx temporal solo para validación HTTP-01
docker compose up -d nginx

# 3. Obtener certificado
docker run -it --rm \
  -v "$(pwd)/certbot/conf:/etc/letsencrypt" \
  -v "$(pwd)/certbot/www:/var/www/certbot" \
  certbot/certbot certonly \
  --webroot -w /var/www/certbot \
  --email "$EMAIL" \
  --agree-tos \
  --no-eff-email \
  -d "$DOMAIN"

# 4. Reiniciar nginx con certificado real
docker compose restart nginx

echo "=== Certificado obtenido. Renovación automática recomendada via cron ==="
```

---

## 6. `certbot-renew.sh` (renovación automática)

```bash
#!/usr/bin/env bash
# certbot-renew.sh — Añadir a crontab: 0 3 * * * /path/certbot-renew.sh

set -euo pipefail

cd /path/to/SyncFiles  # AJUSTA ESTA RUTA

echo "[$(date)] Intentando renovación de certificado..."

docker run -it --rm \
  -v "$(pwd)/certbot/conf:/etc/letsencrypt" \
  -v "$(pwd)/certbot/www:/var/www/certbot" \
  certbot/certbot renew --quiet --webroot -w /var/www/certbot

if [ $? -eq 0 ]; then
  echo "[$(date)] Renovación exitosa, recargando nginx..."
  docker compose exec nginx nginx -s reload
else
  echo "[$(date)] No hay certificados para renovar o error"
fi
```

---

## 7. Android signing config (ya implementado — configurar propiedades)

El `build.gradle.kts` ya tiene el signing config implementado. Solo necesitas
proporcionar las 4 propiedades. Puedes hacerlo de 3 formas:

### Opción A: `gradle.properties` local (no versionado)

```properties
# syncfiles-android/gradle.properties — AÑADE estas líneas (NO commitear)
SF_KEYSTORE_FILE=syncfiles-release.keystore
SF_KEYSTORE_PASSWORD=TU_STORE_PASSWORD
SF_KEYSTORE_ALIAS=syncfiles
SF_KEY_PASSWORD=TU_KEY_PASSWORD
```

### Opción B: Variables de entorno

```bash
export SF_KEYSTORE_FILE=syncfiles-release.keystore
export SF_KEYSTORE_PASSWORD=TU_PASSWORD
export SF_KEYSTORE_ALIAS=syncfiles
export SF_KEY_PASSWORD=TU_PASSWORD
./gradlew assembleRelease
```

### Opción C: `gradle.properties` global de usuario (`~/.gradle/gradle.properties`)

```properties
SF_KEYSTORE_FILE=syncfiles-release.keystore
SF_KEYSTORE_PASSWORD=TU_PASSWORD
SF_KEYSTORE_ALIAS=syncfiles
SF_KEY_PASSWORD=TU_PASSWORD
```

**Entradas/salidas esperadas:**
| Acción | Input | Output | Verificación |
|--------|-------|--------|--------------|
| `generate-keystore.sh` | Java keytool | `syncfiles-release.keystore` | `ls -la *.keystore` |
| Define `SF_*` props | Editor/env vars | Signing config disponible | `cat gradle.properties` |
| `./gradlew assembleRelease` | Keystore + props | `app-release.apk` | `ls -la app/build/outputs/apk/release/app-release.apk` |

---

## 8. `generate-keystore.sh` (generar keystore release)

```bash
#!/usr/bin/env bash
# generate-keystore.sh — Ejecutar UNA VEZ para crear keystore
# Guarda las passwords en un gestor de contraseñas

set -euo pipefail

KEYSTORE_FILE="syncfiles-release.keystore"
ALIAS="syncfiles"
VALIDITY=10000  # ~27 años

echo "=== Generando keystore de release ==="

read -s -p "Store password: " STORE_PASS
echo
read -s -p "Key password (Enter para usar mismo): " KEY_PASS
echo
KEY_PASS=${KEY_PASS:-$STORE_PASS}

keytool -genkey -v \
  -keystore "$KEYSTORE_FILE" \
  -alias "$ALIAS" \
  -keyalg RSA \
  -keysize 2048 \
  -validity "$VALIDITY" \
  -storepass "$STORE_PASS" \
  -keypass "$KEY_PASS" \
  -dname "CN=SyncFiles, OU=Mobile, O=SyncFiles, L=City, ST=State, C=US"

echo
echo "✅ Keystore generado: $KEYSTORE_FILE"
echo "🔐 Guarda estas credenciales en tu gestor de contraseñas:"
echo "   Store password: $STORE_PASS"
echo "   Key password:   $KEY_PASS"
echo "   Alias:          $ALIAS"
```

---

## 9. Despliegue paso a paso

### En tu máquina local:

```bash
# 1. Posicionarse en root del proyecto
cd /Users/firebender/Documents/SyncFiles\ Project

# 2. Crear .env con valores reales (usa el template del plan)
#    Edita SF_ADMIN_TOKEN, SF_SERVER_URL, SF_USER_0_* con tus valores

# 3. Build imagen Docker (compila Rust aquí, NO en VM — ahorra 15-20 min en la VM)
docker compose build --no-cache

# 4. (Opcional) Exportar como tar.gz para transferir offline si no usas registry
docker save syncfiles-server:local | gzip > syncfiles-server-v0.1.0.tar.gz

# 5. (Opcional) Subir a registry — RECOMENDADO para facilitar updates
docker tag syncfiles-server:local ghcr.io/TU_USUARIO/syncfiles-server:v0.1.0
docker push ghcr.io/TU_USUARIO/syncfiles-server:v0.1.0

# 6. Generar APK Android firmado
cd syncfiles-android
#    6a. Generar keystore (UNA VEZ) — usa el template de §8
#    6b. Configurar gradle.properties con SF_KEYSTORE_* props (§7)
#    6c. Build
./gradlew assembleRelease
#    Output: app/build/outputs/apk/release/app-release.apk

# 7. (Opcional) Build CLI para Mac
cd ../syncfiles-cli
cargo build --release
#    Output: target/release/syncfiles-cli
```

### En la VM (GCP e2-micro):

```bash
# 1. Crear VM en GCP Console O vía gcloud:
#    REGLAS GRATIS: zona us-central1/us-east1/us-west1, disco pd-standard (HDD), solo 1 VM
gcloud compute instances create syncfiles-vm \
  --zone=us-central1-a \
  --machine-type=e2-micro \
  --boot-disk-size=30GB \
  --boot-disk-type=pd-standard \
  --tags=http-server,https-server
#    IMPORTANTE: En GCP Console (web), al crear la VM:
#      - Región: us-central1 (Iowa)  ← NO usar otras regiones
#      - Tipo de disco: "Disco persistente estándar (HDD)"  ← NO usar "balanceado" o "SSD"
#      - Firewall: marcar "Permitir tráfico HTTP" y "Permitir tráfico HTTPS"

# 2. SSH a la VM
gcloud compute ssh syncfiles-vm --zone=us-central1-a

# 3. Instalar Docker + compose plugin
curl https://get.docker.com | sh
sudo usermod -aG docker $USER
newgrp docker
sudo apt-get update && sudo apt-get install -y docker-compose-plugin

# 4. Clonar repo
git clone https://github.com/TU_USUARIO/SyncFiles.git
cd SyncFiles

# 5. Elegir cómo obtener la imagen:
#    Opción A (registry, recomendado):
#      docker compose pull  # descarga de ghcr.io
#    Opción B (offline, si exportaste tar.gz):
#      scp syncfiles-server-v0.1.0.tar.gz syncfiles-vm:/tmp/
#      docker load < /tmp/syncfiles-server-v0.1.0.tar.gz
#      # Edita docker-compose.yml: comenta 'image:', descomenta 'build:'

# 6. Crear .env (usa los valores reales)
#    Nano o pega el .env de tu máquina local

# 7. Iniciar stack (server + backup-cron, sin nginx todavía)
docker compose --profile default --profile cron up -d

# 8. Verificar server responde en puerto 8080
curl http://localhost:8080/health/live
# Debería responder: {"status":"live"}

# 9. Configurar nginx + TLS (solo si usas dominio HTTPS)
#    9a. Crear nginx.conf (reemplaza TU_DOMINIO.COM por tu dominio)
#    9b. Inicializar certificado Let's Encrypt
chmod +x certbot-init.sh && ./certbot-init.sh
#    9c. Añadir nginx al compose (requiere nginx.conf + certbot/conf/)
#        docker compose --profile prod up -d nginx

# 10. Configurar rclone para backup (opcional)
rclone config
# name: gdrive, type: drive, scope: drive.file, root_folder_id: XXXXX

# 11. Verificar backup manual
docker compose exec backup /scripts/backup.sh

# 12. Añadir renovación automática a crontab
(crontab -l 2>/dev/null; echo "0 3 * * * /home/YOURUSER/SyncFiles/certbot-renew.sh") | crontab -
```

### Desde el móvil (prueba real):

```bash
# 1. Instalar APK en el teléfono (desde navegador o ADB)
#    - Abrir app-release.apk en el teléfono
#    - Si error de "Unknown sources", permitir en Ajustes → Seguridad

# 2. Conectar al server
#    - Server URL: http://TU_IP_PÚBLICA_GCP:8080
#      (obtenla con: gcloud compute instances list --format=... externalIP)
#    - Credenciales: SF_USER_0_EMAIL / SF_USER_0_PASSWORD del .env

# 3. Probar sincronización:
#    - Subir un archivo desde Android
#    - Ver en web: http://TU_IP:8080 (login con mismas credenciales)
#    - Verificar que el archivo aparece en la interfaz web
```

### Para pruebas rápidas (sin dominio ni SSL):

Si solo quieres probar localmente sin comprar dominio ni configurar nginx:
```bash
# En GCP VM:
docker compose --profile default --profile cron up -d
# Luego accede directamente:
# Android: http://TU_IP_PÚBLICA:8080
# Web: http://TU_IP_PÚBLICA:8080
# SSH tunnel (más seguro):
#   ssh -L 8080:localhost:8080 YOURUSER@TU_IP_GCP
#   luego accede a http://localhost:8080 en tu Mac
```

---

## 10. Verificación post-despliegue

```bash
# Health (sin nginx)
curl http://TU_IP_PÚBLICA:8080/health/live
curl http://TU_IP_PÚBLICA:8080/health/ready

# Con nginx (HTTPS)
curl -sf https://TU_DOMINIO.COM/health/ready

# Dashboard
open http://TU_IP_PÚBLICA:8080
# o con SSL: open https://TU_DOMINIO.COM

# Login web
# Email: admin@tudominio.com
# Pass:  password-seguro-aqui (del .env)

# Android app
# Server URL: http://TU_IP_PÚBLICA:8080
# Credenciales: mismas que arriba

# Backup manual a Drive
docker compose exec backup /scripts/backup.sh

# Verificar en Drive
rclone ls gdrive:SyncFilesBackups/
```

---

## Archivos a generar/modificar

| Archivo | Ubicación | Acción |
|---------|-----------|--------|
| `.env` | Raíz proyecto | **Crear** (no versionado) — template en §2 |
| `docker-compose.yml` | Raíz proyecto | **Ya existe** — usar con profiles |
| `nginx.conf` | Raíz proyecto | **Crear** — template en §4 |
| `certbot-init.sh` | Raíz proyecto | **Crear** — template en §5 |
| `certbot-renew.sh` | Raíz proyecto | **Crear** — template en §6 |
| `gradle.properties` | syncfiles-android/ | **Editar** — añadir SF_KEYSTORE_* props (§7) |
| `generate-keystore.sh` | syncfiles-android/ | **Crear** — template en §8 |
| `syncfiles-release.keystore` | syncfiles-android/ | **Generar** via script (§8) |

---

## Notas importantes

1. **NO commitees** `.env`, `gradle.properties` (si contiene passwords), `syncfiles-release.keystore`, `certbot/conf/`
2. **docker-compose.yml ya existe** — usa `profiles` para activar servicios: `default` (server+backup), `cron` (backup-cron), `dev` (adminer), `prod` (nginx)
3. **Volúmenes**: `sf-data` persiste SQLite + storage + backups
4. **Backup cron**: Corre cada 24h dentro de contenedor `backup-cron` (profile `cron`)
5. **TLS**: Certbot valida vía HTTP-01 en puerto 80, luego nginx sirve en 443
6. **Android**: `10.0.2.2` es localhost del host desde emulador; en celular real usa IP pública de GCP
7. **Sin dominio**: Usa IP pública GCP (puerto 8080) para pruebas rápidas — sin comprar dominio ni SSL
8. **GCP Free Tier — REGLAS DE ORO**:
   - **Región**: `us-central1`, `us-east1`, o `us-west1` únicamente. Otras regiones facturan al 100%.
   - **Disco**: Usa **Disco persistente estándar (HDD)** (`pd-standard`). El disco "balanceado" o "SSD" tiene costo.
   - **Una sola VM**: El free tier da ~744 horas/mes. Solo 1 e2-micro alcanza para 24/7. No crear VMs adicionales.