#!/usr/bin/env bash
# E2E — Pull de rename/move/copy remotos + deletes locales + changelog del server.
# Escenarios (Tarea 3.3 extendida):
#   1. upload A → rename A → diff B muestra operation="rename" con old_path.
#   2. move → diff; copy → diff + download de destino con checksum idéntico.
#   3. delete → diff con delete.
#   4. Cursor: diff since=<seq> no repite entradas ya vistas.
#   5. Backfill/continuidad: reinicio de server → diff con cursor previo no
#      devuelve lo ya aplicado.
#   6. Cliente desktop headless: A crea archivo local → sube; CLI renombra en
#      server → el ciclo de A mueve el archivo en su sync_root; A borra un
#      archivo local → siguiente ciclo propaga delete (diff desde CLI lo muestra).
# Requisitos: server + cli + client compilados (cargo build).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SERVER_BIN="$ROOT/syncfiles-server/target/debug/syncfiles-server"
CLI_BIN="$ROOT/syncfiles-cli/target/debug/syncfiles-cli"
CLIENT_BIN="$ROOT/syncfiles-client/target/debug/syncfiles-client"
WORKDIR="/tmp/syncfiles-e2e-rename-move-copy"
SERVER_URL="http://127.0.0.1:8083"
DB_PATH="$WORKDIR/data/syncfiles.db"
STORAGE_PATH="$WORKDIR/storage"
EMAIL="admin@syncfiles.local"
PASSWORD="syncfiles"
DEV_A="e2e-dev-a"
DEV_CLI="e2e-dev-cli"

PASS=0
FAIL=0
CLEANUP_PIDS=()

log()  { printf '      %s\n' "$*"; }
ok()   { PASS=$((PASS+1)); printf '  [PASS] %s\n' "$*"; }
bad()  { FAIL=$((FAIL+1)); printf '  [FAIL] %s\n' "$*"; }
section() { printf '\n=== %s ===\n' "$*"; }

start_server() {
    SF_BIND_ADDRESS="127.0.0.1:8083" \
    SF_DATABASE_URL="sqlite:$DB_PATH" \
    SF_SERVER_URL="$SERVER_URL" \
    SF_STORAGE_ROOT="$STORAGE_PATH" \
    SF_USER_0_EMAIL="$EMAIL" \
    SF_USER_0_PASSWORD="$PASSWORD" \
    SF_USER_0_ID="user-001" \
    "$SERVER_BIN" >> "$WORKDIR/server.log" 2>&1 &
    local pid=$!
    CLEANUP_PIDS+=("$pid")
    for _ in $(seq 1 15); do
        curl -s -o /dev/null -m 2 "$SERVER_URL/api/v1/session/status" && break
        sleep 1
    done
    curl -s -o /dev/null -m 2 "$SERVER_URL/api/v1/session/status" || {
        echo "FATAL: servidor no arranca"; tail -20 "$WORKDIR/server.log"; exit 1;
    }
    echo "$pid"
}

stop_server() {
    local pid="$1"
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
}

cli() { # cli <device> <command> [args...] — salida del comando
    local dev="$1"; shift
    SF_SERVER_URL="$SERVER_URL" SF_EMAIL="$EMAIL" SF_PASSWORD="$PASSWORD" SF_DEVICE_ID="$dev" \
        "$CLI_BIN" "$@"
}

# Extrae el file_id de una respuesta JSON de files/list
file_id_of() { # file_id_of <json> <relative_path>
    grep -o '"file_id":"[^"]*","relative_path":"[^"]*"' <<< "$1" \
        | grep "\"relative_path\":\"$2\"" | head -1 | cut -d'"' -f4
}

cleanup() {
    if [ "${#CLEANUP_PIDS[@]}" -gt 0 ]; then
        for pid in "${CLEANUP_PIDS[@]}"; do kill "$pid" 2>/dev/null || true; done
    fi
}
trap cleanup EXIT

rm -rf "$WORKDIR"
mkdir -p "$WORKDIR/data" "$STORAGE_PATH"

echo "=== SyncFiles — E2E rename/move/copy + deletes locales + changelog ==="

section "[1] Servidor local + upload inicial"
SERVER_PID=$(start_server)
log "Servidor OK (PID=$SERVER_PID, $SERVER_URL)"

echo "contenido-original" > "$WORKDIR/rename-src.txt"
UP_RESP=$(cli "$DEV_A" upload "$WORKDIR/rename-src.txt")
grep -q '"status":"ok"' <<< "$UP_RESP" && ok "Upload desde dispositivo A" || { bad "Upload A: $UP_RESP"; }

FILES_LIST=$(cli "$DEV_A" login >/dev/null; curl -s "$SERVER_URL/api/v1/files/list" \
    -H "Authorization: Bearer $(cli "$DEV_A" login | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)")
FILE_ID=$(file_id_of "$FILES_LIST" "rename-src.txt")
[ -n "$FILE_ID" ] && ok "file_id obtenido: $FILE_ID" || bad "No se encontró rename-src.txt: $(head -c 300 <<< "$FILES_LIST")"

section "[2] Rename remoto visible en diff"
RENAME_RESP=$(cli "$DEV_A" rename "$FILE_ID" "docs/renamed.txt")
grep -q '"status":"ok"' <<< "$RENAME_RESP" && ok "Rename aceptado por el server" || { bad "Rename: $RENAME_RESP"; }

DIFF_B=$(cli "$DEV_CLI" diff 0)
grep -q '"operation":"rename"' <<< "$DIFF_B" && ok "Diff B muestra operation=rename" || { bad "Diff B sin rename: $(head -c 400 <<< "$DIFF_B")"; }
grep -q '"old_path":"rename-src.txt"' <<< "$DIFF_B" && ok "Diff B incluye old_path=rename-src.txt" || bad "Diff B sin old_path: $(head -c 400 <<< "$DIFF_B")"
grep -q '"relative_path":"docs/renamed.txt"' <<< "$DIFF_B" && ok "Diff B incluye path destino docs/renamed.txt" || bad "Diff B sin path destino"

section "[3] Move remoto visible en diff"
SESSION_A=$(cli "$DEV_A" login | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
MOVE_JSON=$(curl -s -X POST "$SERVER_URL/api/v1/sync/move" \
    -H "Authorization: Bearer $SESSION_A" \
    -H "Content-Type: application/json" \
    -d "{\"session_id\":\"$SESSION_A\",\"device_id\":\"$DEV_A\",\"file_id\":\"$FILE_ID\",\"old_path\":\"docs/renamed.txt\",\"new_path\":\"docs/moved/deep.txt\",\"idempotency_key\":\"move-$RANDOM\"}")
grep -q '"status":"ok"' <<< "$MOVE_JSON" && ok "Move aceptado por el server" || { bad "Move: $(head -c 300 <<< "$MOVE_JSON")"; }

DIFF_B=$(cli "$DEV_CLI" diff 0)
grep -q '"operation":"move"' <<< "$DIFF_B" && ok "Diff B muestra operation=move" || { bad "Diff B sin move: $(head -c 400 <<< "$DIFF_B")"; }
grep -q '"old_path":"docs/renamed.txt"' <<< "$DIFF_B" && grep -q '"relative_path":"docs/moved/deep.txt"' <<< "$DIFF_B" \
    && ok "Diff B incluye old/new path del move" || bad "Diff B sin rutas del move"

section "[4] Copy remoto + download con checksum idéntico"
COPY_JSON=$(curl -s -X POST "$SERVER_URL/api/v1/sync/copy" \
    -H "Authorization: Bearer $SESSION_A" \
    -H "Content-Type: application/json" \
    -d "{\"session_id\":\"$SESSION_A\",\"device_id\":\"$DEV_A\",\"file_id\":\"$FILE_ID\",\"source_path\":\"docs/moved/deep.txt\",\"destination_path\":\"docs/copia.txt\",\"idempotency_key\":\"copy-$RANDOM\"}")
grep -q '"status":"ok"' <<< "$COPY_JSON" && ok "Copy aceptado por el server" || { bad "Copy: $(head -c 300 <<< "$COPY_JSON")"; }

DIFF_B=$(cli "$DEV_CLI" diff 0)
grep -q '"operation":"copy"' <<< "$DIFF_B" && ok "Diff B muestra operation=copy" || { bad "Diff B sin copy: $(head -c 400 <<< "$DIFF_B")"; }

# La copia es un archivo nuevo: download debe devolver el mismo contenido
COPY_FILE_ID=$(file_id_of "$(curl -s "$SERVER_URL/api/v1/files/list" -H "Authorization: Bearer $SESSION_A")" "docs/copia.txt")
if [ -n "$COPY_FILE_ID" ]; then
    cli "$DEV_CLI" download "$COPY_FILE_ID" "$WORKDIR/copy-bajada.txt" > /dev/null
    if cmp -s "$WORKDIR/rename-src.txt" "$WORKDIR/copy-bajada.txt"; then
        ok "Download de la copia con checksum idéntico"
    else
        bad "Contenido de la copia difiere del original"
    fi
else
    bad "No se encontró docs/copia.txt en files/list"
fi

section "[5] Delete remoto visible en diff"
DEL_RESP=$(cli "$DEV_A" delete "$FILE_ID")
grep -q '"status":"ok"' <<< "$DEL_RESP" && ok "Delete aceptado por el server" || { bad "Delete: $DEL_RESP"; }

DIFF_B=$(cli "$DEV_CLI" diff 0)
grep -q '"operation":"delete"' <<< "$DIFF_B" && ok "Diff B muestra operation=delete" || { bad "Diff B sin delete: $(head -c 400 <<< "$DIFF_B")"; }

section "[6] Cursor: diff since=<seq> no repite entradas ya vistas"
# Consumir todo el diff y usar el server_seq como cursor
CURSOR=$(cli "$DEV_CLI" diff 0 | grep -o '"server_seq":[0-9]*' | cut -d: -f2)
if [ -n "$CURSOR" ] && [ "$CURSOR" -gt 0 ]; then
    ok "Cursor obtenido del diff: seq=$CURSOR"
else
    bad "No se pudo leer server_seq del diff"
fi
DIFF_EMPTY=$(cli "$DEV_CLI" diff "$CURSOR")
CHANGES_COUNT=$(grep -o '"file_id":"[^"]*"' <<< "$DIFF_EMPTY" | wc -l | tr -d ' ' || true)
[ "$CHANGES_COUNT" -eq 0 ] && ok "Diff con cursor=$CURSOR no repite entradas ya vistas" \
    || bad "Diff con cursor devolvió $CHANGES_COUNT entradas: $(head -c 300 <<< "$DIFF_EMPTY")"
NEW_SEQ=$(grep -o '"server_seq":[0-9]*' <<< "$DIFF_EMPTY" | cut -d: -f2 || true)
[ "$NEW_SEQ" = "$CURSOR" ] && ok "server_seq estable con cursor al día ($NEW_SEQ)" || bad "server_seq cambió sin cambios: $CURSOR -> $NEW_SEQ"

section "[7] Backfill/continuidad tras reinicio del server"
# Un cambio nuevo + reinicio: el diff con cursor previo solo debe traer lo nuevo
echo "post-restart" > "$WORKDIR/post-restart.txt"
UP2=$(cli "$DEV_A" upload "$WORKDIR/post-restart.txt")
grep -q '"status":"ok"' <<< "$UP2" && ok "Upload post-restart.txt" || bad "Upload post-restart: $UP2"

stop_server "$SERVER_PID"
SERVER_PID=$(start_server)
log "Servidor reiniciado (PID=$SERVER_PID); el backfill no debe duplicar entradas"

DIFF_AFTER=$(cli "$DEV_CLI" diff "$CURSOR")
grep -q "post-restart.txt" <<< "$DIFF_AFTER" && ok "Diff tras reinicio trae solo el cambio nuevo (post-restart.txt)" \
    || bad "Diff tras reinicio no trae post-restart.txt: $(head -c 300 <<< "$DIFF_AFTER")"
RESTARTED_COUNT=$(grep -o '"file_id":"[^"]*"' <<< "$DIFF_AFTER" | wc -l | tr -d ' ' || true)
[ "$RESTARTED_COUNT" -eq 1 ] && ok "Exactamente 1 cambio nuevo tras reinicio (sin duplicar backfill)" \
    || bad "Diff tras reinicio devolvió $RESTARTED_COUNT entradas (esperaba 1)"

section "[8] Cliente desktop headless: pull de rename + propagación de delete local"
CLIENT_DATA="$WORKDIR/client-a-data"
CLIENT_SYNC_A="$WORKDIR/client-a-sync"
mkdir -p "$CLIENT_DATA" "$CLIENT_SYNC_A"

run_client() { # run_client <sync_root> <data_dir> <logfile>
    SF_HEADLESS=1 \
    SF_SERVER_URL="$SERVER_URL" SF_EMAIL="$EMAIL" SF_PASSWORD="$PASSWORD" \
    SF_DEVICE_ID="$DEV_A" \
    SF_SYNC_ROOT="$1" SF_DATA_DIR="$2" SF_POLLING_INTERVAL=3 \
    "$CLIENT_BIN" > "$3" 2>&1 &
    local cpid=$!
    CLEANUP_PIDS+=("$cpid")
    echo "$cpid"
}

wait_log() { # wait_log <logfile> <pattern> [tries]
    local logfile="$1" pattern="$2" tries=0 max="${3:-30}"
    while [ $tries -lt "$max" ]; do
        grep -q "$pattern" "$logfile" 2>/dev/null && return 0
        sleep 1
        tries=$((tries+1))
    done
    return 1
}

CLIENT_PID=$(run_client "$CLIENT_SYNC_A" "$CLIENT_DATA" "$WORKDIR/client-a.log")
wait_log "$WORKDIR/client-a.log" "Ciclo de sincronización completado" \
    && ok "Cliente A headless: primer ciclo OK" \
    || bad "Cliente A no completó ciclo: $(tail -5 "$WORKDIR/client-a.log")"

# A crea un archivo local → el motor lo sube
echo "archivo-local-de-A" > "$CLIENT_SYNC_A/local-a.txt"
wait_log "$WORKDIR/client-a.log" "Upload aceptado: local-a.txt" 20 \
    && ok "A sube local-a.txt vía watcher+motor" \
    || bad "A no subió local-a.txt: $(tail -8 "$WORKDIR/client-a.log")"

# CLI renombra en el server → el ciclo de A debe mover el archivo en su sync_root
FILES_LIST=$(curl -s "$SERVER_URL/api/v1/files/list" -H "Authorization: Bearer $SESSION_A")
LOCAL_FILE_ID=$(file_id_of "$FILES_LIST" "local-a.txt")
RENAME_A=$(cli "$DEV_CLI" rename "$LOCAL_FILE_ID" "docs/local-a-movido.txt")
grep -q '"status":"ok"' <<< "$RENAME_A" && ok "CLI renombra local-a.txt en el server" || bad "Rename CLI: $RENAME_A"

if wait_log "$WORKDIR/client-a.log" "Renombrado local" 25; then
    if [ -f "$CLIENT_SYNC_A/docs/local-a-movido.txt" ] && [ ! -f "$CLIENT_SYNC_A/local-a.txt" ]; then
        ok "Ciclo de A movió el archivo en su sync_root (docs/local-a-movido.txt)"
    else
        bad "Archivo no movido en sync_root de A: $(ls -R "$CLIENT_SYNC_A" 2>/dev/null | head -20)"
    fi
else
    bad "El ciclo de A no aplicó el rename: $(tail -10 "$WORKDIR/client-a.log")"
fi

# A borra un archivo local → siguiente ciclo propaga el delete
rm "$CLIENT_SYNC_A/docs/local-a-movido.txt"
log "Archivo local-a-movido.txt borrado en A (fuera de la UI)"
if wait_log "$WORKDIR/client-a.log" "Delete local detectado por escaneo" 15; then
    ok "Reconcile de A detectó el delete local"
else
    bad "A no detectó el delete local: $(tail -10 "$WORKDIR/client-a.log")"
fi
if wait_log "$WORKDIR/client-a.log" "Delete propagado al server" 15; then
    ok "Motor de A propagó el delete al server"
else
    bad "A no propagó el delete: $(tail -10 "$WORKDIR/client-a.log")"
fi

# Verificar desde el CLI que el server ya no tiene el archivo
FILES_AFTER=$(curl -s "$SERVER_URL/api/v1/files/list" -H "Authorization: Bearer $SESSION_A")
if ! grep -q '"status":"synced"' <(grep -F '"relative_path":"docs/local-a-movido.txt"' <<< "$FILES_AFTER" 2>/dev/null) || \
   ! grep -qF '"relative_path":"docs/local-a-movido.txt"' <<< "$FILES_AFTER"; then
    ok "Server ya no lista docs/local-a-movido.txt como activo (delete propagado)"
else
    bad "Server sigue listando docs/local-a-movido.txt: $(grep -o '"relative_path":"docs/local-a-movido.txt"[^}]*' <<< "$FILES_AFTER")"
fi

kill "$CLIENT_PID" 2>/dev/null || true

# ---------------------------------------------------------------- resumen
stop_server "$SERVER_PID"
printf '\n=== RESULTADOS: %d PASS / %d FAIL ===\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ] || exit 1
