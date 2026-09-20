# Servicio HTTP de SyncFiles fuera de localhost

## Reglas de seguridad

- No exponga `SF_BIND_ADDRESS=0.0.0.0:8080` directamente a Internet.
- Use HTTPS en el reverse proxy, firewall restringido y un `SF_ADMIN_TOKEN` largo.
- Los endpoints `/api/v1/*` requieren sesión; `/admin/backup` requiere el token administrativo.
- En LAN, use HTTPS cuando sea posible. El cleartext HTTP queda limitado a pruebas debug o redes de confianza.

## Opción A: systemd en Linux

### 1. Instalar el release

```bash
sudo useradd --system --home /var/lib/syncfiles --create-home syncfiles
sudo mkdir -p /opt/syncfiles /var/lib/syncfiles/{db,storage,backups}
sudo chown -R syncfiles:syncfiles /opt/syncfiles /var/lib/syncfiles

tar -xzf syncfiles-*-linux-x86_64.tar.gz
sudo mkdir -p /opt/syncfiles/bin
sudo cp syncfiles-*/syncfiles-server syncfiles-*/syncfiles-client syncfiles-*/syncfiles-cli /opt/syncfiles/bin/
sudo cp -R syncfiles-*/migrations syncfiles-*/static syncfiles-*/docs /opt/syncfiles/
sudo cp syncfiles-*/syncfiles-server.service /etc/systemd/system/syncfiles-server.service
```

### 2. Configurar

`/etc/default/syncfiles`:

```bash
SF_BIND_ADDRESS=127.0.0.1:8080
SF_DATABASE_URL=sqlite:/var/lib/syncfiles/db/syncfiles.db
SF_STORAGE_ROOT=/var/lib/syncfiles/storage
SF_MIGRATIONS_DIR=/opt/syncfiles/migrations
SF_STATIC_DIR=/opt/syncfiles/static
BACKUP_DIR=/var/lib/syncfiles/backups
SF_BACKUP_LOG=/var/lib/syncfiles/backup.log
SF_ADMIN_TOKEN=cambiar-por-un-token-largo
SF_USER_0_EMAIL=admin@example.com
SF_USER_0_PASSWORD=cambiar-por-una-contrasena
SF_USER_0_ID=user-001
RUST_LOG=info
```

### 3. Arrancar y comprobar

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now syncfiles-server
sudo systemctl status syncfiles-server
curl -fsS http://127.0.0.1:8080/health/live
curl -fsS http://127.0.0.1:8080/health/ready
sudo journalctl -u syncfiles-server -f
```

## Opción B: Docker

```bash
cp .env.example .env
# Edite .env y cambie credenciales/token.
docker compose up -d
docker compose ps
docker compose exec server curl -fsS http://localhost:8080/health/ready
docker compose run --rm backup
```

El volumen `sf-data` conserva SQLite, storage y backups. Para backup diario:

```bash
docker compose --profile cron up -d
```

## Opción C: reverse proxy con TLS

Instale el certificado con Let's Encrypt y active el sitio:

```bash
sudo nginx -t
sudo systemctl reload nginx
```

La plantilla está en `nginx/syncfiles.conf`. Cambie `syncfiles.example.com`, certificados y el upstream. El proxy envía `X-Forwarded-For`; el rate limiter de V1 lo usa como clave de IP.

## LAN

```bash
scripts/setup-lan.sh --server-ip 192.168.1.25 --port 8080
scripts/setup-lan.sh --server-ip 192.168.1.25 --check
scripts/setup-lan.sh --open-firewall --port 8080
```

La APK debug permite cleartext para pruebas LAN. La APK release debe usar HTTPS o una configuración Android específica para la IP del servidor; no habilite cleartext global en producción.

## Backup antes de una actualización

```bash
sudo systemctl stop syncfiles-server
sudo -u syncfiles /opt/syncfiles/backup.sh
sudo systemctl start syncfiles-server
```

En Docker:

```bash
docker compose run --rm backup
```

Verifique el archivo `backup_*.tar.gz` y su `.sha256` antes de copiarlo fuera del host.
