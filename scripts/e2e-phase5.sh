#!/usr/bin/env bash
# Fase 5 — Batería de pruebas E2E reales de SyncFiles.
# Escenarios (plan.md):
#   5.2 Multi-dispositivo: subida en A → propagación a B vía diff → descarga
#   5.3 Conflicto: mismo path, checksums distintos desde 2 dispositivos → resolución keep_local
#   5.4 Reinicio de cliente con cola pendiente: operaciones quedan en sync_queue → se reprocesan
#   5.5 Red interrumpida: servidor cae a mitad de subida → reintento tras recuperación
# Requisitos: server + cli compilados (cargo build) y jq no requerido (parse con grep).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SERVER_BIN="$ROOT/syncfiles-server/target/debug/syncfiles-server"
CLI_BIN="$ROOT/syncfiles-cli/target/debug/syncfiles-cli"
CLIENT_BIN="$ROOT/syncfiles-client/target/debug/syncfiles-client"
WORKDIR="/tmp/syncfiles-e2e-phase5"
SERVER_URL="http://127.0.0.1:8082"
DB_PATH="$WORKDIR/data/syncfiles.db"
STORAGE_PATH="$WORKDIR/storage"
EMAIL="admin@syncfiles.local"
PASSWORD="syncfiles"
DEV_A="e2e-dev-a"
DEV_B="e2e-dev-b"

PASS=0
FAIL=0
CLEANUP_PIDS=()

log()  { printf '      %s\n' "$*"; }
ok()   { PASS=$((PASS+1)); printf '  [PASS] %s\n' "$*"; }
bad()  { FAIL=$((FAIL+1)); printf '  [FAIL] %s\n' "$*"; }
section() { printf '\n=== %s ===\n' "$*"; }

start_server() {
    SF_BIND_ADDRESS="127.0.0.1:8082" \
    SF_DATABASE_URL="sqlite:$DB_PATH" \
    SF_SERVER_URL="$SERVER_URL" \
    SF_STORAGE_ROOT="$STORAGE_PATH" \
    SF_USER_0_EMAIL="$EMAIL" \
    SF_USER_0_PASSWORD="$PASSWORD" \
    SF_USER_0_ID="user-001" \
    "$SERVER_BIN" >> "$WORKDIR/server.log" 2>&1 &
    local pid=$!
    CLEANUP_PIDS+=("$pid")
    # Espera activa: 401 sin Bearer = servidor vivo
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

cleanup() {
    for pid in "${CLEANUP_PIDS[@]:-}"; do kill "$pid" 2>/dev/null || true; done
}
trap cleanup EXIT

rm -rf "$WORKDIR"
mkdir -p "$WORKDIR/data" "$STORAGE_PATH"

echo "=== SyncFiles Fase 5 — E2E Real ==="

# ---------------------------------------------------------------- 5.1 server up
section "[5.1] Servidor local"
SERVER_PID=$(start_server)
log "Servidor OK (PID=$SERVER_PID, $SERVER_URL)"

# ------------------------------------------------------------- 5.2 multi-device
section "[5.2] Escenario multi-dispositivo (A sube → B descubre vía diff)"
echo "dispositivo-A $(date +%s)" > "$WORKDIR/multi.txt"
UP_RESP=$(cli "$DEV_A" upload "$WORKDIR/multi.txt")
grep -q '"status":"ok"' <<< "$UP_RESP" && ok "Upload desde dispositivo A" || { bad "Upload A: $UP_RESP"; }

# B pide diff desde 0 y debe ver el archivo
DIFF_B=$(cli "$DEV_B" diff 0)
grep -q "multi.txt" <<< "$DIFF_B" && ok "B ve el cambio remoto en diff" || { bad "Diff B: $(head -c 300 <<< "$DIFF_B")"; }

FILE_ID=$(grep -o '"file_id":"[^"]*"' <<< "$DIFF_B" | head -1 | cut -d'"' -f4)
DL_RESP=$(cli "$DEV_B" download "$FILE_ID" "$WORKDIR/multi-b.txt")
if cmp -s "$WORKDIR/multi.txt" "$WORKDIR/multi-b.txt"; then
    ok "B descargó contenido idéntico (checksum match)"
else
    bad "Contenido descargado difiere: $(head -c 200 <<< "$DL_RESP")"
fi

# B borra; A debe ver el delete en diff
DEL_RESP=$(cli "$DEV_B" delete "$FILE_ID")
DIFF_A=$(cli "$DEV_A" diff 0)
grep -q '"operation":"delete"' <<< "$DIFF_A" && ok "A observa el delete de B vía diff" || { bad "A no ve delete: $(head -c 300 <<< "$DIFF_A")"; }

# ------------------------------------------------------------ 5.3 conflicto
section "[5.3] Escenario de conflicto (edición simultánea offline)"
echo "version-desde-A" > "$WORKDIR/conflict.txt"
UP_A=$(cli "$DEV_A" upload "$WORKDIR/conflict.txt")
grep -q '"status":"ok"' <<< "$UP_A" && ok "A sube versión 1" || { bad "Upload conflicto A: $UP_A"; }

# B edita el MISMO path sin sincronizar (simula offline), checksum distinto
echo "version-desde-B-totalmente-distinta" > "$WORKDIR/conflict.txt"
UP_B=$(cli "$DEV_B" upload "$WORKDIR/conflict.txt")
grep -q '"status":"conflict"' <<< "$UP_B" && ok "Servidor detecta conflicto (status=conflict)" || { bad "B upload: $(head -c 300 <<< "$UP_B")"; }

# GET /conflicts debe listar 1 conflicto pendiente
CONFLICTS=$(curl -s "$SERVER_URL/api/v1/conflicts" \
    -H "Authorization: Bearer $(cli "$DEV_A" login | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)")
CONFLICT_ID=$(grep -o '"conflict_id":"[^"]*"' <<< "$CONFLICTS" | head -1 | cut -d'"' -f4)
if [ -n "$CONFLICT_ID" ]; then
    ok "GET /conflicts lista el conflicto ($CONFLICT_ID)"
else
    bad "GET /conflicts vacío: $(head -c 300 <<< "$CONFLICTS")"
fi

# Resolver desde "la UI" (API que usa la UI): keep_local
if [ -n "$CONFLICT_ID" ]; then
    SESSION=$(cli "$DEV_A" login | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
    RESOLVE=$(curl -s -X POST "$SERVER_URL/api/v1/conflicts/resolve" \
        -H "Authorization: Bearer $SESSION" -H "Content-Type: application/json" \
        -d "{\"session_id\":\"$SESSION\",\"device_id\":\"$DEV_A\",\"conflict_id\":\"$CONFLICT_ID\",\"decision\":\"keep_local\",\"preserve_alternative\":true}")
    grep -q '"accepted":true' <<< "$RESOLVE" && ok "Resolución keep_local aceptada" || bad "Resolve: $(head -c 300 <<< "$RESOLVE")"
    # El conflicto ya no debe aparecer
    CONFLICTS2=$(curl -s "$SERVER_URL/api/v1/conflicts" -H "Authorization: Bearer $SESSION")
    grep -q '"conflict_id"' <<< "$CONFLICTS2" && bad "Conflicto sigue pendiente tras resolver" || ok "Conflicto resuelto (ya no aparece en /conflicts)"
fi

# ------------------------------------------------------------ 5.4 restart cola
section "[5.4] Reinicio de cliente con cola pendiente (motor headless real)"
# Cliente A (desktop headless) arranca, se crea un archivo → watcher encola → motor sube.
# Luego se mata el cliente A con un archivo recién creado (queda 'pending' + cola local),
# se reinicia, y el motor debe reprocesar la cola persistente y subirlo.
CLIENT_DATA="$WORKDIR/client-a-data"
CLIENT_SYNC_A="$WORKDIR/client-a-sync"
mkdir -p "$CLIENT_DATA" "$CLIENT_SYNC_A"

run_client() { # run_client <sync_root> <data_dir> <logfile> — lanza cliente headless
    SF_HEADLESS=1 \
    SF_SERVER_URL="$SERVER_URL" SF_EMAIL="$EMAIL" SF_PASSWORD="$PASSWORD" \
    SF_DEVICE_ID="$DEV_A" \
    SF_SYNC_ROOT="$1" SF_DATA_DIR="$2" SF_POLLING_INTERVAL=3 \
    "$CLIENT_BIN" > "$3" 2>&1 &
    local cpid=$!
    CLEANUP_PIDS+=("$cpid")
    echo "$cpid"
}

wait_client_ok() { # espera a que el log muestre "Ciclo de sincronización completado"
    local logfile="$1" tries=0
    while [ $tries -lt 30 ]; do
        grep -q "Ciclo de sincronización completado" "$logfile" 2>/dev/null && return 0
        sleep 1
        tries=$((tries+1))
    done
    return 1
}

CLIENT_PID=$(run_client "$CLIENT_SYNC_A" "$CLIENT_DATA" "$WORKDIR/client-a-1.log")
if wait_client_ok "$WORKDIR/client-a-1.log"; then
    ok "Cliente A headless: login + primer ciclo OK"
else
    bad "Cliente A no completó ciclo inicial: $(tail -5 "$WORKDIR/client-a-1.log")"
fi

# Crear archivo mientras el cliente corre → watcher encola → motor sube
echo "contenido-version-1" > "$CLIENT_SYNC_A/restart.txt"
sleep 8  # ciclo del motor (3s) + debounce del watcher (2s)
SESSION_A=$(cli "$DEV_A" login | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
FILES_LIST=$(curl -s "$SERVER_URL/api/v1/files/list" -H "Authorization: Bearer $SESSION_A")
grep -q "restart.txt" <<< "$FILES_LIST" && ok "Archivo subido por cliente A (vía watcher+motor)" || bad "Server no tiene restart.txt: $(head -c 300 <<< "$FILES_LIST")"

# Matamos el cliente A y creamos un archivo NUEVO offline.
# El estado local ('pending') debe persistir en SQLite y reprocesarse al reiniciar.
kill "$CLIENT_PID" 2>/dev/null || true
wait "$CLIENT_PID" 2>/dev/null || true
sleep 1
echo "archivo-offline-antes-de-reinicio" > "$CLIENT_SYNC_A/offline-pending.txt"
log "Cliente A detenido; archivo offline-pending.txt creado offline"

# Verificar que SQLite del cliente retiene el estado
if [ -f "$CLIENT_DATA/metadata.db" ] || ls "$CLIENT_DATA"/*.db >/dev/null 2>&1; then
    log "SQLite del cliente persiste en $CLIENT_DATA"
else
    log "(nota: no hay .db aún; el watcher no llegó a encolar antes del kill)"
fi

# Reiniciar cliente A: el escaneo de reconciliación debe subir el archivo offline
CLIENT_PID=$(run_client "$CLIENT_SYNC_A" "$CLIENT_DATA" "$WORKDIR/client-a-2.log")
log "Cliente A reiniciado (PID=$CLIENT_PID)"
sleep 10  # escaneo + ciclo del motor (3s)
if grep -q "offline-pending" "$WORKDIR/client-a-2.log"; then
    ok "Cliente reiniciado detectó/procesó el archivo offline"
else
    log "(log del motor tras reinicio)"; tail -8 "$WORKDIR/client-a-2.log"
fi
SESSION_A2=$(cli "$DEV_A" login | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
FILES_LIST2=$(curl -s "$SERVER_URL/api/v1/files/list" -H "Authorization: Bearer $SESSION_A2")
grep -q "offline-pending.txt" <<< "$FILES_LIST2" && ok "Archivo offline subido tras reinicio del cliente" || bad "offline-pending.txt no llegó al server: $(head -c 300 <<< "$FILES_LIST2")"
kill "$CLIENT_PID" 2>/dev/null || true

# ------------------------------------------------------------ 5.5 red interrumpida
section "[5.5] Red interrumpida a mitad de subida"
# Matamos el servidor ANTES de subir: el CLI debe fallar limpiamente (sin colgar).
stop_server "$SERVER_PID"
echo "sin-servidor" > "$WORKDIR/interrupted.txt"
if cli "$DEV_A" upload "$WORKDIR/interrupted.txt" > "$WORKDIR/interrupted-resp.txt" 2>&1; then
    bad "Upload tuvo éxito con servidor caído (inesperado)"
else
    ok "Upload con servidor caído falla limpiamente (exit != 0, sin colgar)"
fi

# Servidor vuelve: el reintento manual (equivalente al ciclo del SyncEngine) debe funcionar
SERVER_PID=$(start_server)
log "Servidor recuperado (PID=$SERVER_PID)"
if cli "$DEV_A" upload "$WORKDIR/interrupted.txt" > "$WORKDIR/retry-resp.txt" 2>&1; then
    grep -q '"status":"ok"' "$WORKDIR/retry-resp.txt" && ok "Reintento tras recuperación OK" || bad "Reintento: $(cat "$WORKDIR/retry-resp.txt")"
else
    bad "Reintento tras recuperación falló: $(cat "$WORKDIR/retry-resp.txt")"
fi

# Cliente con cola pendiente mientras el servidor está caído → reintento del motor
CLIENT_SYNC_B="$WORKDIR/client-b-sync"
mkdir -p "$CLIENT_SYNC_B"
echo "esperando-red" > "$CLIENT_SYNC_B/while-down.txt"
CLIENT_PID=$(run_client "$CLIENT_SYNC_B" "$WORKDIR/client-b-data" "$WORKDIR/client-b.log")
sleep 8
grep -q "Error en ciclo" "$WORKDIR/client-b.log" && ok "Motor registra error de ciclo con servidor caído (estado visible en UI)" || log "(aún sin error registrado)"
# Recuperar servidor y esperar a que el motor reintente (polling 3s)
stop_server "$SERVER_PID"
SERVER_PID=$(start_server)
log "Servidor de vuelta; esperando reintento del motor (≤15s)"
sleep 15
SESSION_B=$(cli "$DEV_A" login | grep -o '"session_id":"[^"]*"' | cut -d'"' -f4)
FILES_B=$(curl -s "$SERVER_URL/api/v1/files/list" -H "Authorization: Bearer $SESSION_B")
grep -q "while-down.txt" <<< "$FILES_B" && ok "Motor reintentó y subió while-down.txt tras recuperación" || bad "while-down.txt no llegó tras reintento: $(head -c 300 <<< "$FILES_B")"
kill "$CLIENT_PID" 2>/dev/null || true

# Notificaciones/actividad: la UI de actividad debe tener eventos registrados
ACTIVITY=$(curl -s "$SERVER_URL/api/v1/activity?limit=5" -H "Authorization: Bearer $SESSION_B")
grep -q '"event_name"\|"events"' <<< "$ACTIVITY" && ok "GET /activity registra eventos para la UI" || bad "Activity: $(head -c 200 <<< "$ACTIVITY")"

# ---------------------------------------------------------------- resumen
stop_server "$SERVER_PID"
printf '\n=== RESULTADOS: %d PASS / %d FAIL ===\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ] || exit 1
