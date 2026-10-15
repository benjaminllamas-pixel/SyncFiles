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
