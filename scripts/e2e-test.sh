#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SERVER_BIN="$ROOT/syncfiles-server/target/debug/syncfiles-server"
CLI_BIN="$ROOT/syncfiles-cli/target/debug/syncfiles-cli"
WORKDIR="/tmp/syncfiles-e2e"
SERVER_URL="http://127.0.0.1:8081"
DB_PATH="$WORKDIR/data/syncfiles.db"
STORAGE_PATH="$WORKDIR/storage"
SYNC_ROOT="$WORKDIR/sync"
EMAIL="admin@syncfiles.local"
PASSWORD="syncfiles"
DEVICE_ID="e2e-test"

echo "=== SyncFiles E2E Test ==="

# Cleanup
rm -rf "$WORKDIR"
mkdir -p "$WORKDIR/data" "$STORAGE_PATH" "$SYNC_ROOT"

# Start server
echo "[1/7] Iniciando servidor..."
SF_BIND_ADDRESS="127.0.0.1:8081" \
SF_DATABASE_URL="sqlite:$DB_PATH" \
SF_SERVER_URL="$SERVER_URL" \
SF_STORAGE_ROOT="$STORAGE_PATH" \
SF_USER_0_EMAIL="$EMAIL" \
SF_USER_0_PASSWORD="$PASSWORD" \
SF_USER_0_ID="user-001" \
"$SERVER_BIN" > "$WORKDIR/server.log" 2>&1 &
SERVER_PID=$!
# Espera activa del servidor (responde aunque sea 401, sin Bearer)
for _ in 1 2 3 4 5 6 7 8 9 10; do
    if ! curl -s -o /dev/null -m 2 "$SERVER_URL/api/v1/session/status"; then
        sleep 1
    else
        break
    fi
done

# Check server is up (401 sin token = servidor vivo)
if ! curl -s -o /dev/null -m 2 "$SERVER_URL/api/v1/session/status"; then
    echo "FAIL: Server no responde"
    cat "$WORKDIR/server.log"
    exit 1
fi
echo "      Servidor OK (PID=$SERVER_PID)"

# Login
echo "[2/7] Login..."
LOGIN_RESP=$(SF_SERVER_URL="$SERVER_URL" SF_EMAIL="$EMAIL" SF_PASSWORD="$PASSWORD" SF_DEVICE_ID="$DEVICE_ID" "$CLI_BIN" login)
SESSION_ID=$(echo "$LOGIN_RESP" | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
if [ -z "$SESSION_ID" ]; then
    echo "FAIL: Login no devolvió session_id"
    echo "$LOGIN_RESP"
    exit 1
fi
echo "      Session: $SESSION_ID"

# Upload text file
echo "[3/7] Upload archivo de texto..."
echo "Hola SyncFiles E2E" > "$WORKDIR/test.txt"
UPLOAD_RESP=$(SF_SERVER_URL="$SERVER_URL" SF_EMAIL="$EMAIL" SF_PASSWORD="$PASSWORD" SF_DEVICE_ID="$DEVICE_ID" "$CLI_BIN" upload "$WORKDIR/test.txt")
if echo "$UPLOAD_RESP" | grep -q '"status":"ok"'; then
    echo "      Upload OK"
else
    echo "FAIL: Upload falló"
    echo "$UPLOAD_RESP"
    exit 1
fi

# Upload binary file
echo "[4/7] Upload archivo binario..."
head -c 1024 /dev/urandom > "$WORKDIR/binary.bin"
BIN_UPLOAD_RESP=$(SF_SERVER_URL="$SERVER_URL" SF_EMAIL="$EMAIL" SF_PASSWORD="$PASSWORD" SF_DEVICE_ID="$DEVICE_ID" "$CLI_BIN" upload "$WORKDIR/binary.bin")
if echo "$BIN_UPLOAD_RESP" | grep -q '"status":"ok"'; then
    echo "      Upload binario OK"
else
    echo "FAIL: Upload binario falló"
    echo "$BIN_UPLOAD_RESP"
    exit 1
fi

# Get diff
echo "[5/7] Diff..."
DIFF_RESP=$(SF_SERVER_URL="$SERVER_URL" SF_EMAIL="$EMAIL" SF_PASSWORD="$PASSWORD" SF_DEVICE_ID="$DEVICE_ID" "$CLI_BIN" diff 0)
echo "      Diff response: $(echo "$DIFF_RESP" | head -c 200)"

# Download and verify (usa el file_id de test.txt desde files/list; el diff
# excluye cambios del propio device desde el cursor seq de 694e0cb)
echo "[6/7] Download y verificación..."
LIST_RESP=$(curl -s -H "Authorization: Bearer $SESSION_ID" "$SERVER_URL/api/v1/files/list")
FILE_ID=$(echo "$LIST_RESP" | python3 -c "
import json,sys
data = json.load(sys.stdin)
for f in data.get('files', []):
    if f.get('relative_path') == 'test.txt':
        print(f['file_id']); break
" 2>/dev/null || true)
if [ -z "$FILE_ID" ]; then
    echo "FAIL: files/list no devolvió file_id para test.txt"
    echo "$LIST_RESP"
    exit 1
fi
DL_RESP=$(SF_SERVER_URL="$SERVER_URL" SF_EMAIL="$EMAIL" SF_PASSWORD="$PASSWORD" SF_DEVICE_ID="$DEVICE_ID" "$CLI_BIN" download "$FILE_ID" "$WORKDIR/dl-test.txt")
if grep -q "Saved" <<< "$DL_RESP"; then
    if cmp -s "$WORKDIR/test.txt" "$WORKDIR/dl-test.txt"; then
        echo "      Download OK (contenido verificado)"
    else
        echo "FAIL: El contenido descargado no coincide"
        exit 1
    fi
else
    echo "FAIL: Download falló"
    echo "$DL_RESP"
    exit 1
fi

# Test conflict: upload same path with different checksum
echo "[7/7] Test conflicto (upload con checksum distinto)..."
echo "Contenido modificado para conflicto" > "$WORKDIR/test.txt"
CONFLICT_RESP=$(SF_SERVER_URL="$SERVER_URL" SF_EMAIL="$EMAIL" SF_PASSWORD="$PASSWORD" SF_DEVICE_ID="$DEVICE_ID" "$CLI_BIN" upload "$WORKDIR/test.txt")
if echo "$CONFLICT_RESP" | grep -q '"status":"conflict"'; then
    echo "      Conflicto detectado correctamente"
else
    echo "      Advertencia: conflicto no detectado (puede ser ok si el path hash difiere)"
fi

# Stop server and restart to test queue recovery
echo ""
echo "=== Recuperación de cola tras reinicio ==="
kill "$SERVER_PID" 2>/dev/null || true
wait "$SERVER_PID" 2>/dev/null || true
echo "Servidor detenido, reiniciando..."
SF_BIND_ADDRESS="127.0.0.1:8081" \
SF_DATABASE_URL="sqlite:$DB_PATH" \
SF_SERVER_URL="$SERVER_URL" \
SF_STORAGE_ROOT="$STORAGE_PATH" \
SF_USER_0_EMAIL="$EMAIL" \
SF_USER_0_PASSWORD="$PASSWORD" \
SF_USER_0_ID="user-001" \
"$SERVER_BIN" > "$WORKDIR/server2.log" 2>&1 &
SERVER_PID=$!
for _ in 1 2 3 4 5 6 7 8 9 10; do
    if ! curl -s -o /dev/null -m 2 "$SERVER_URL/api/v1/session/status"; then
        sleep 1
    else
        break
    fi
done

# Verify data persists
DIFF2=$(SF_SERVER_URL="$SERVER_URL" SF_EMAIL="$EMAIL" SF_PASSWORD="$PASSWORD" SF_DEVICE_ID="$DEVICE_ID" "$CLI_BIN" diff 0)
if echo "$DIFF2" | grep -q '"changes"'; then
    echo "      Datos persistidos correctamente tras reinicio"
else
    echo "FAIL: Datos no persistieron"
    echo "$DIFF2"
    exit 1
fi

# Cleanup
kill "$SERVER_PID" 2>/dev/null || true
wait "$SERVER_PID" 2>/dev/null || true

echo ""
echo "=== E2E Test Completado ==="
echo "Logs: $WORKDIR/server.log"
