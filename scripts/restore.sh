#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BACKUP_FILE="${1:-}"
if [[ -z "$BACKUP_FILE" ]]; then
    printf 'Uso: %s <backup_*.tar.gz>\n' "$0" >&2
    exit 2
fi

DATABASE_URL="${SF_DATABASE_URL:-sqlite:data/syncfiles.db}"
STORAGE_ROOT="${SF_STORAGE_ROOT:-data/storage}"
BACKUP_DIR="${SF_BACKUP_DIR:-data/restore-backups}"
RETENTION_LOG="${SF_RESTORE_LOG:-data/restores.log}"
DRY_RUN="${SF_RESTORE_DRY_RUN:-0}"
FORCE="${SF_RESTORE_FORCE:-0}"
STARTED_EPOCH="$(date +%s)"
RESULT=FAIL
PREVIOUS_DATABASE=""
PREVIOUS_STORAGE=""
DATABASE_REPLACED=0
STORAGE_REPLACED=0

if [[ "$DATABASE_URL" != sqlite:* ]]; then
    printf 'Solo se admite SF_DATABASE_URL con prefijo sqlite:.\n' >&2
    exit 2
fi
DATABASE_PATH="${DATABASE_URL#sqlite:}"
DATABASE_PATH="${DATABASE_PATH#//}"
if [[ "$DATABASE_PATH" != /* ]]; then
    DATABASE_PATH="$ROOT/$DATABASE_PATH"
fi
if [[ "$STORAGE_ROOT" != /* ]]; then
    STORAGE_ROOT="$ROOT/$STORAGE_ROOT"
fi
if [[ "$BACKUP_DIR" != /* ]]; then
    BACKUP_DIR="$ROOT/$BACKUP_DIR"
fi
if [[ "$RETENTION_LOG" != /* ]]; then
    RETENTION_LOG="$ROOT/$RETENTION_LOG"
fi
if [[ ! -f "$BACKUP_FILE" ]]; then
    printf 'Backup no encontrado: %s\n' "$BACKUP_FILE" >&2
    exit 1
fi
BACKUP_FILE="$(cd "$(dirname "$BACKUP_FILE")" && pwd)/$(basename "$BACKUP_FILE")"
mkdir -p "$(dirname "$DATABASE_PATH")" "$(dirname "$STORAGE_ROOT")" "$BACKUP_DIR" "$(dirname "$RETENTION_LOG")"

checksum256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | tr -s ' ' | cut -d' ' -f1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | tr -s ' ' | cut -d' ' -f1
    else
        python3 - "$1" <<'PY'
import hashlib
import sys
with open(sys.argv[1], "rb") as source:
    print(hashlib.sha256(source.read()).hexdigest())
PY
    fi
}

integrity_check() {
    if command -v sqlite3 >/dev/null 2>&1; then
        sqlite3 "$1" 'PRAGMA integrity_check;' 2>/dev/null || true
    else
        python3 - "$1" <<'PY'
import sqlite3
import sys
try:
    connection = sqlite3.connect(sys.argv[1])
    print(connection.execute('PRAGMA integrity_check').fetchone()[0])
except Exception:
    print('error')
PY
    fi
}

log() {
    local message="$*"
    printf '[%s] %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$message" | tee -a "$RETENTION_LOG"
}

cleanup() {
    if [[ "$RESULT" != "OK" ]]; then
        if [[ "$DATABASE_REPLACED" == "1" && -n "$PREVIOUS_DATABASE" && ! -e "$DATABASE_PATH" ]]; then
            mv "$PREVIOUS_DATABASE" "$DATABASE_PATH" 2>/dev/null || true
        fi
        if [[ "$STORAGE_REPLACED" == "1" && -n "$PREVIOUS_STORAGE" && ! -e "$STORAGE_ROOT" ]]; then
            mv "$PREVIOUS_STORAGE" "$STORAGE_ROOT" 2>/dev/null || true
        fi
    fi
    if [[ -n "${WORKDIR:-}" ]]; then
        rm -rf "$WORKDIR"
    fi
}
trap cleanup EXIT

if [[ -f "$BACKUP_FILE.sha256" ]]; then
    expected="$(tr -s ' ' < "$BACKUP_FILE.sha256" | cut -d' ' -f1)"
    actual="$(checksum256 "$BACKUP_FILE")"
    if [[ "$expected" != "$actual" ]]; then
        log "ERROR checksum del backup"
        exit 1
    fi
    log "Checksum del backup verificado"
fi

WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/syncfiles-restore.XXXXXX")"
EXTRACT="$WORKDIR/extract"
RESTORED="$WORKDIR/restored"
mkdir -p "$EXTRACT" "$RESTORED"
tar -tzf "$BACKUP_FILE" >/dev/null
tar -xzf "$BACKUP_FILE" -C "$EXTRACT"

METADATA_FILES=()
while IFS= read -r file; do
    METADATA_FILES+=("$file")
done < <(find "$EXTRACT" -maxdepth 1 -type f -name 'metadata_*.db' -print)
STORAGE_ARCHIVES=()
while IFS= read -r file; do
    STORAGE_ARCHIVES+=("$file")
done < <(find "$EXTRACT" -maxdepth 1 -type f -name 'storage_*.tar.gz' -print)
if [[ ${#METADATA_FILES[@]} -ne 1 ]]; then
    log "ERROR se esperaba un metadata_*.db"
    exit 1
fi
if [[ ${#STORAGE_ARCHIVES[@]} -gt 1 ]]; then
    log "ERROR se encontró más de un storage_*.tar.gz"
    exit 1
fi
cp "${METADATA_FILES[0]}" "$RESTORED/syncfiles.db"
if [[ ${#STORAGE_ARCHIVES[@]} -eq 1 ]]; then
    mkdir -p "$RESTORED/storage"
    tar -xzf "${STORAGE_ARCHIVES[0]}" -C "$RESTORED/storage"
else
    mkdir -p "$RESTORED/storage/users"
fi

if [[ "$DRY_RUN" == "1" ]]; then
    printf 'Dry-run: %s contiene %s y %s\n' "$BACKUP_FILE" "$(basename "${METADATA_FILES[0]}")" "${#STORAGE_ARCHIVES[@]}"
    exit 0
fi

if [[ -e "$DATABASE_PATH" || -e "$STORAGE_ROOT" ]]; then
    if [[ "$FORCE" != "1" ]]; then
        log "ERROR el destino existe; use SF_RESTORE_FORCE=1 después de detener el servicio"
        exit 1
    fi
    if [[ -n "${SF_SERVER_PID_FILE:-}" && -f "$SF_SERVER_PID_FILE" ]]; then
        old_pid="$(tr -d '[:space:]' < "$SF_SERVER_PID_FILE")"
        if kill -0 "$old_pid" 2>/dev/null; then
            log "ERROR el servidor sigue activo (PID=$old_pid)"
            exit 1
        fi
    fi
    stamp="$(date +%Y%m%d_%H%M%S)"
    if [[ -e "$DATABASE_PATH" ]]; then
        PREVIOUS_DATABASE="$DATABASE_PATH.pre-restore-$stamp"
        mv "$DATABASE_PATH" "$PREVIOUS_DATABASE"
        DATABASE_REPLACED=1
    fi
    if [[ -e "$STORAGE_ROOT" ]]; then
        PREVIOUS_STORAGE="$STORAGE_ROOT.pre-restore-$stamp"
        mv "$STORAGE_ROOT" "$PREVIOUS_STORAGE"
        STORAGE_REPLACED=1
    fi
fi

mv "$RESTORED/syncfiles.db" "$DATABASE_PATH"
mv "$RESTORED/storage" "$STORAGE_ROOT"

integrity="$(integrity_check "$DATABASE_PATH")"
if [[ "$integrity" != "ok" ]]; then
    log "ERROR integrity_check falló"
    exit 1
fi

python3 - "$DATABASE_PATH" "$STORAGE_ROOT" <<'PY'
import hashlib
import pathlib
import sqlite3
import sys

db_path, storage_root = sys.argv[1:3]
connection = sqlite3.connect(db_path)
rows = connection.execute(
    "SELECT user_id, relative_path, checksum, size_bytes FROM files WHERE deleted_at IS NULL"
).fetchall()
for user_id, relative_path, expected_hash, expected_size in rows:
    path = pathlib.Path(storage_root) / "users" / user_id / "files" / pathlib.PurePosixPath(relative_path)
    if not path.is_file():
        raise SystemExit(f"archivo ausente: {path}")
    data = path.read_bytes()
    actual_hash = hashlib.sha256(data).hexdigest()
    if actual_hash != expected_hash or len(data) != expected_size:
        raise SystemExit(f"checksum o tamaño inválido: {path}")
print(f"archivos verificados: {len(rows)}")
PY

DURATION="$(( $(date +%s) - STARTED_EPOCH ))"
RESULT=OK
log "RESTORE backup=$BACKUP_FILE database=$DATABASE_PATH storage=$STORAGE_ROOT result=$RESULT duration=${DURATION}s"
