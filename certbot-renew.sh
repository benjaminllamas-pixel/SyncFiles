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
