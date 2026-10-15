#!/usr/bin/env bash
# certbot-init.sh — Ejecutar UNA VEZ en la VM para obtener certificado inicial
# Uso: chmod +x certbot-init.sh && ./certbot-init.sh

set -euo pipefail

DOMAIN="TU_DOMINIO.COM"
EMAIL="admin@tudominio.com"

echo "=== Inicializando certificado Let's Encrypt para $DOMAIN ==="

mkdir -p certbot/conf certbot/www

docker compose up -d nginx

docker run -it --rm \
  -v "$(pwd)/certbot/conf:/etc/letsencrypt" \
  -v "$(pwd)/certbot/www:/var/www/certbot" \
  certbot/certbot certonly \
  --webroot -w /var/www/certbot \
  --email "$EMAIL" \
  --agree-tos \
  --no-eff-email \
  -d "$DOMAIN"

docker compose restart nginx

echo "=== Certificado obtenido. Renovación automática recomendada via cron ==="
