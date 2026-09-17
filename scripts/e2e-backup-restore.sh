#!/usr/bin/env bash
# SyncFiles — E2E Test: Backup → Delete → Restore → Verify
#
# 1. Inicia server, sube archivos, verifica checksums
# 2. Ejecuta backup (scripts/backup.sh)
# 3. Detiene server, borra DB + storage
# 4. Restaura backup
# 5. Reinicia server, verifica archivos y checksums
#
# Uso: ./scripts/e2e-backup-restore.sh

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SERVER_BIN="$ROOT/syncfiles-server/target/debug/syncfiles-server"
CLI_BIN="$ROOT/syncfiles-cli/target/debug/syncfiles-cli"
WORKDIR="/tmp/syncfiles-backup-e2e"
SERVER_URL="http://127.0.0.1:8082"
DB_PATH="$WORKDIR/data/syncfiles.db"
STORAGE_PATH="$WORKDIR/storage"
BACKUP_DIR="$WORKDIR/backups"
EMAIL="admin@syncfiles.local"
PASSWORD="syncfiles"
DEVICE_ID="e2e-backup"

echo "=== SyncFiles E2E Backup & Restore Test ==="

# Cleanup
rm -rf "$WORKDIR"
mkdir -p "$WORKDIR/data" "$STORAGE_PATH" "$BACKUP_DIR"

# Start server
echo "[1/6] Iniciando servidor..."
SF_BIND_ADDRESS="127.0.0.1:8082" \
SF_DATABASE_URL="sqlite:$DB_PATH" \
SF_SERVER_URL="$SERVER_URL" \
SF_STORAGE_ROOT="$STORAGE_PATH" \
SF_USER_0_EMAIL="$EMAIL" \
SF_USER_0_PASSWORD="$PASSWORD" \
SF_USER_0_ID="user-001" \
"$SERVER_BIN" > "$WORKDIR/server.log" 2>&1 &
SERVER_PID=$!

# Espera activa
for _ in $(seq 1 10); do
    curl -s -o /dev/null -m 2 "$SERVER_URL/api/v1/session/status" && break
    sleep 1
done

if ! curl -s -o /dev/null -m 2 "$SERVER_URL/api/v1/session/status"; then
    echo "FAIL: Server no responde"
    cat "$WORKDIR/server.log"
    exit 1
fi
echo "      Servidor OK (PID=$SERVER_PID)"

# Login
echo "[2/6] Login y upload..."
LOGIN_RESP=$(SF_SERVER_URL="$SERVER_URL" SF_EMAIL="$EMAIL" SF_PASSWORD="$PASSWORD" SF_DEVICE_ID="$DEVICE_ID" "$CLI_BIN" login)
SESSION_ID=$(echo "$LOGIN_RESP" | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)

# Upload text file
echo "hola backup e2e" > "$WORKDIR/test1.txt"
echo "segundo archivo" > "$WORKDIR/test2.bin"
UPLOAD_RESP=$(SF_SERVER_URL="$SERVER_URL" SF_EMAIL="$EMAIL" SF_PASSWORD="$PASSWORD" SF_DEVICE_ID="$DEVICE_ID" "$CLI_BIN" upload "$WORKDIR/test1.txt")
if ! echo "$UPLOAD_RESP" | grep -q '"status":"ok"'; then
    echo "FAIL: Upload test1 falló"
    echo "$UPLOAD_RESP"
    exit 1
fi
UPLOAD_RESP2=$(SF_SERVER_URL="$SERVER_URL" SF_EMAIL="$EMAIL" SF_PASSWORD="$PASSWORD" SF_DEVICE_ID="$DEVICE_ID" "$CLI_BIN" upload "$WORKDIR/test2.bin")
if ! echo "$UPLOAD_RESP2" | grep -q '"status":"ok"'; then
    echo "FAIL: Upload test2 falló"
    echo "$UPLOAD_RESP2"
    exit 1
fi

# Compute checksums of original files
CHECKSUM1=$(sha256sum "$WORKDIR/test1.txt" | awk '{print $1}')
CHECKSUM2=$(sha256sum "$WORKDIR/test2.bin" | awk '{print $1}')
echo "      Upload OK (checksums: $CHECKSUM1, $CHECKSUM2)"

# List files to get file_ids
LIST_RESP=$(curl -s -H "Authorization: Bearer $SESSION_ID" "$SERVER_URL/api/v1/files/list")
echo "$LIST_RESP" | python3 -c "
import json, sys
data = json.load(sys.stdin)
assert data['total'] == 2, f'Expected 2 files, got {data[\"total\"]}'
print('      files/list verifica 2 archivos')
" || {
    echo "FAIL: files/list"
    echo "$LIST_RESP"
    exit 1
}

# Step 2: Execute backup
echo "[3/6] Ejecutando backup..."
SF_DATABASE_URL="sqlite:$DB_PATH" \
SF_STORAGE_ROOT="$STORAGE_PATH" \
BACKUP_DIR="$BACKUP_DIR" \
SF_BACKUP_LOG="$WORKDIR/backup.log" \
bash "$ROOT/scripts/backup.sh" > "$WORKDIR/backup-output.log" 2>&1 || {
    echo "FAIL: backup.sh falló"
    cat "$WORKDIR/backup-output.log"
    exit 1
}
echo "      Backup completado"

# Verify backup file exists
BACKUP_FILE=$(find "$BACKUP_DIR" -name "backup_*.tar.gz" | head -1)
if [ -z "$BACKUP_FILE" ]; then
    echo "FAIL: No se encontró backup_*.tar.gz"
    exit 1
fi
BACKUP_CHECKSUM=$(sha256sum "$BACKUP_FILE" | awk '{print $1}')
echo "      Backup: $BACKUP_FILE (sha256=$BACKUP_CHECKSUM)"

# Step 3: Stop server, delete DB + storage
echo "[4/6] Deteniendo server y borrando datos..."
kill "$SERVER_PID" 2>/dev/null || true
wait "$SERVER_PID" 2>/dev/null || true

rm -f "$DB_PATH"
rm -rf "$STORAGE_PATH"
echo "      DB y storage borrados"

# Step 4: Restore backup
echo "[5/6] Restaurando backup..."
# Extract combined backup
TMP_RESTORE="/tmp/syncfiles-restore-$$"
rm -rf "$TMP_RESTORE"
mkdir -p "$TMP_RESTORE"
tar -xzf "$BACKUP_FILE" -C "$TMP_RESTORE"

# Restore metadata DB
METADATA_BACKUP=$(find "$TMP_RESTORE" -name "metadata_*.db" | head -1)
if [ -z "$METADATA_BACKUP" ]; then
    echo "FAIL: No se encontró metadata_*.db en el backup"
    exit 1
fi
mkdir -p "$WORKDIR/data"
cp "$METADATA_BACKUP" "$DB_PATH"

# Restore storage
STORAGE_BACKUP=$(find "$TMP_RESTORE" -name "storage_*.tar.gz" | head -1)
if [ -z "$STORAGE_BACKUP" ]; then
    echo "WARN: No se encontró storage_*.tar.gz en el backup; storage vacío"
else
    mkdir -p "$STORAGE_PATH"
    tar -xzf "$STORAGE_BACKUP" -C "$STORAGE_PATH"
fi
rm -rf "$TMP_RESTORE"
echo "      Restore completado"

# Step 5: Restart server, verify
echo "[6/6] Reiniciando server y verificando..."
SF_BIND_ADDRESS="127.0.0.1:8082" \
SF_DATABASE_URL="sqlite:$DB_PATH" \
SF_SERVER_URL="$SERVER_URL" \
SF_STORAGE_ROOT="$STORAGE_PATH" \
SF_USER_0_EMAIL="$EMAIL" \
SF_USER_0_PASSWORD="$PASSWORD" \
SF_USER_0_ID="user-001" \
"$SERVER_BIN" > "$WORKDIR/server2.log" 2>&1 &
SERVER_PID=$!

for _ in $(seq 1 10); do
    curl -s -o /dev/null -m 2 "$SERVER_URL/api/v1/session/status" && break
    sleep 1
done

if ! curl -s -o /dev/null -m 2 "$SERVER_URL/api/v1/session/status"; then
    echo "FAIL: Server no responde tras restore"
    cat "$WORKDIR/server2.log"
    exit 1
fi

# Verify data persisted
LOGIN_RESP=$(SF_SERVER_URL="$SERVER_URL" SF_EMAIL="$EMAIL" SF_PASSWORD="$PASSWORD" SF_DEVICE_ID="$DEVICE_ID" "$CLI_BIN" login)
SESSION_ID=$(echo "$LOGIN_RESP" | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)

LIST_RESP=$(curl -s -H "Authorization: Bearer $SESSION_ID" "$SERVER_URL/api/v1/files/list")
echo "$LIST_RESP" | python3 -c "
import json, sys
data = json.load(sys.stdin)
assert data['total'] == 2, f'Expected 2 files after restore, got {data[\"total\"]}'
print('      files/list verifica 2 archivos tras restore')
" || {
    echo "FAIL: files/list tras restore"
    echo "$LIST_RESP"
    exit 1
}

# Download and verify checksums
FILE_ID1=$(echo "$LIST_RESP" | python3 -c "
import json, sys
data = json.load(sys.stdin)
for f in data.get('files', []):
    if f.get('relative_path') == 'test1.txt':
        print(f['file_id']); break
" 2>/dev/null || true)
FILE_ID2=$(echo "$LIST_RESP" | python3 -c "
import json, sys
data = json.load(sys.stdin)
for f in data.get('files', []):
    if f.get('relative_path') == 'test2.bin':
        print(f['file_id']); break
" 2>/dev/null || true)

curl -s -H "Authorization: Bearer $SESSION_ID" "$SERVER_URL/api/v1/sync/download" \
    -H "Content-Type: application/json" \
    -d "{\"session_id\":\"$SESSION_ID\",\"device_id\":\"$DEVICE_ID\",\"file_id\":\"$FILE_ID1\",\"path_hash\":\"\",\"idempotency_key\":\"$(uuidgen)\"}" > "$WORKDIR/dl1.json"

echo "$WORKDIR/dl1.json"
DL1_B64=$(python3 -c "import json; print(json.load(open('$WORKDIR/dl1.json'))['content'])")
DL1=$(echo "$DL1_B64" | base64 -d)
DL1_CHECKSUM=$(echo -n "$DL1" | sha256sum | awk '{print $1}')
if [ "$DL1_CHECKSUM" = "$CHECKSUM1" ]; then
    echo "      test1.txt checksum verificado: $DL1_CHECKSUM"
else
    echo "FAIL: test1.txt checksum mismatch (esperado=$CHECKSUM1, obtenido=$DL1_CHECKSUM)"
    exit 1
fi

curl -s -H "Authorization: Bearer $SESSION_ID" "$SERVER_URL/api/v1/sync/download" \
    -H "Content-Type: application/json" \
    -d "{\"session_id\":\"$SESSION_ID\",\"device_id\":\"$DEVICE_ID\",\"file_id\":\"$FILE_ID2\",\"path_hash\":\"\",\"idempotency_key\":\"$(uuidgen)\"}" > "$WORKDIR/dl2.json"

DL2_B64=$(python3 -c "import json; print(json.load(open('$WORKDIR/dl2.json'))['content'])")
DL2=$(echo "$DL2_B64" | base64 -d)
DL2_CHECKSUM=$(echo -n "$DL2" | sha256sum | awk '{print $1}')
if [ "$DL2_CHECKSUM" = "$CHECKSUM2" ]; then
    echo "      test2.bin checksum verificado: $DL2_CHECKSUM"
else
    echo "FAIL: test2.bin checksum mismatch (esperado=$CHECKSUM2, obtenido=$DL2_CHECKSUM)"
    exit 1
fi

# Cleanup
kill "$SERVER_PID" 2>/dev/null || true
wait "$SERVER_PID" 2>/dev/null || true

echo ""
echo "=== E2E Backup & Restore Test: PASS ==="
echo "Backup: $BACKUP_FILE"
echo "Checksums verificados post-restore"
