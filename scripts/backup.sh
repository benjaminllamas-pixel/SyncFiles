#!/usr/bin/env bash
# SyncFiles — backup automatizado
#
# 1. VACUUM INTO → copia atómica y online de SQLite
# 2. tar -czf del storage (users/)
# 3. Combina todo en un archive timestamped
# 4. Rotación: elimina backups > BACKUP_RETENTION_DAYS (default 30)
# 5. Loguea resultado
# 6. (Opcional) Sube a Google Drive via rclone
#
# Uso:
#   ./scripts/backup.sh
#   BACKUP_DIR=/backups BACKUP_RETENTION_DAYS=14 ./scripts/backup.sh
#
# Variables de entorno (core):
#   SF_DATABASE_URL         (default: sqlite:data/syncfiles.db)
#   SF_STORAGE_ROOT         (default: data/storage)
#   BACKUP_DIR              (default: data/backups)
#   BACKUP_RETENTION_DAYS   (default: 30)
#   SF_BACKUP_LOG           (default: data/backup.log)
#   SF_ADMIN_TOKEN          (opcional, para validar antes de backup)
#
# Variables de entorno (Google Drive / rclone) — TODAS OPCIONALES:
#   BACKUP_GDRIVE_ENABLED   (default: "false" — "true" para activar)
#   BACKUP_GDRIVE_REMOTE    (default: "gdrive" — nombre del remote rclone configurado)
#   BACKUP_GDRIVE_PATH      (default: "SyncFilesBackups" — carpeta destino en Drive)
#   BACKUP_GDRIVE_ARGS      (default: "" — args extra para rclone copy, ej: "--bwlimit 1M")
#   RCLONE_CONFIG           (opcional — path a rclone.conf si no está en ubicación estándar)
#
# Requisitos para Google Drive:
#   1. Instalar rclone: https://rclone.org/install.sh
#   2. Configurar remote: rclone config  → name: gdrive, type: drive, scope: drive.file
#   3. Activar: BACKUP_GDRIVE_ENABLED=true ./scripts/backup.sh
#
# Ejemplo completo:
#   BACKUP_GDRIVE_ENABLED=true \
#   BACKUP_GDRIVE_REMOTE=gdrive \
#   BACKUP_GDRIVE_PATH=SyncFilesBackups \
#   BACKUP_GDRIVE_ARGS="--bwlimit 2M --progress" \
#   ./scripts/backup.sh
#
# Notas:
#   - El backup LOCAL siempre se ejecuta primero; la subida a Drive es best-effort.
#   - Si rclone falla, el script NO falla (logea WARN y continúa).
#   - rclone usa su config estándar (~/.config/rclone/rclone.conf) salvo que
#     RCLONE_CONFIG apunte a otro archivo.
#   - Para cifrado: configura un remote 'crypt' sobre gdrive y úsalo como REMOTE.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

SF_DATABASE_URL="${SF_DATABASE_URL:-sqlite:data/syncfiles.db}"
SF_STORAGE_ROOT="${SF_STORAGE_ROOT:-data/storage}"
BACKUP_DIR="${BACKUP_DIR:-data/backups}"
BACKUP_RETENTION_DAYS="${BACKUP_RETENTION_DAYS:-30}"
SF_BACKUP_LOG="${SF_BACKUP_LOG:-data/backup.log}"

# Google Drive / rclone (opcional)
BACKUP_GDRIVE_ENABLED="${BACKUP_GDRIVE_ENABLED:-false}"
BACKUP_GDRIVE_REMOTE="${BACKUP_GDRIVE_REMOTE:-gdrive}"
BACKUP_GDRIVE_PATH="${BACKUP_GDRIVE_PATH:-SyncFilesBackups}"
BACKUP_GDRIVE_ARGS="${BACKUP_GDRIVE_ARGS:-}"
RCLONE_CONFIG="${RCLONE_CONFIG:-}"

TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
BACKUP_NAME="backup_${TIMESTAMP}"
BACKUP_PATH="${BACKUP_DIR}/${BACKUP_NAME}.tar.gz"

mkdir -p "$BACKUP_DIR" "$(dirname "$SF_BACKUP_LOG")"

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

log() {
    local level="$1"
    shift
    local msg="$*"
    echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [$level] $msg" | tee -a "$SF_BACKUP_LOG"
}

log INFO "Iniciando backup: $BACKUP_NAME"

# --- Paso 1: Backup de metadatos (SQLite VACUUM INTO) ---
DB_PATH=""
if [[ "$SF_DATABASE_URL" == sqlite:* ]]; then
    DB_PATH="${SF_DATABASE_URL#sqlite:}"
    # Asegurar que sea absoluto
    if [[ "$DB_PATH" != /* ]]; then
        DB_PATH="$PROJECT_ROOT/$DB_PATH"
    fi
fi

if [[ -z "$DB_PATH" || ! -f "$DB_PATH" ]]; then
    log WARN "No se encontró la BD en $DB_PATH; saltando backup de metadatos"
else
    METADATA_BACKUP="${BACKUP_DIR}/metadata_${TIMESTAMP}.db"
    log INFO "VACUUM INTO: $DB_PATH → $METADATA_BACKUP"
    SQL_METADATA_BACKUP="${METADATA_BACKUP//\'/\'\'}"
    sqlite3 "$DB_PATH" "VACUUM INTO '${SQL_METADATA_BACKUP}';" 2>&1 | tee -a "$SF_BACKUP_LOG" || {
        log ERROR "VACUUM INTO falló"
        exit 1
    }
    log INFO "Backup de metadatos completado: $METADATA_BACKUP"
fi

# --- Paso 2: Backup de storage (tar.gz) ---
if [[ -d "$SF_STORAGE_ROOT" ]]; then
    if [[ "$SF_STORAGE_ROOT" != /* ]]; then
        SF_STORAGE_ROOT="$PROJECT_ROOT/$SF_STORAGE_ROOT"
    fi
    STORAGE_BACKUP="${BACKUP_DIR}/storage_${TIMESTAMP}.tar.gz"
    log INFO "tar storage: $SF_STORAGE_ROOT → $STORAGE_BACKUP"
    tar -czf "$STORAGE_BACKUP" -C "$SF_STORAGE_ROOT" users/ 2>&1 | tee -a "$SF_BACKUP_LOG" || {
        log ERROR "tar storage falló"
        exit 1
    }
    log INFO "Backup de storage completado: $STORAGE_BACKUP"
else
    log WARN "Storage root no existe: $SF_STORAGE_ROOT; saltando backup de storage"
    STORAGE_BACKUP=""
fi

# --- Paso 3: Combinar en archive unified ---
log INFO "Creando archive combinado: $BACKUP_PATH"
COMBINED_ITEMS=""
if [[ -f "${BACKUP_DIR}/metadata_${TIMESTAMP}.db" ]]; then
    COMBINED_ITEMS="${BACKUP_DIR}/metadata_${TIMESTAMP}.db"
fi
if [[ -n "$STORAGE_BACKUP" && -f "$STORAGE_BACKUP" ]]; then
    if [[ -n "$COMBINED_ITEMS" ]]; then
        COMBINED_ITEMS="$COMBINED_ITEMS $STORAGE_BACKUP"
    else
        COMBINED_ITEMS="$STORAGE_BACKUP"
    fi
fi

if [[ -n "$COMBINED_ITEMS" ]]; then
    tar -czf "$BACKUP_PATH" -C "$BACKUP_DIR" \
        "$(basename "${BACKUP_DIR}/metadata_${TIMESTAMP}.db")" \
        "$(basename "$STORAGE_BACKUP")" 2>/dev/null || \
    tar -czf "$BACKUP_PATH" -C "$BACKUP_DIR" \
        $(for item in $COMBINED_ITEMS; do basename "$item"; done)
    log INFO "Archive combinado: $BACKUP_PATH"
else
    log ERROR "No hay datos para backup"
    exit 1
fi

# --- Paso 4: Checksum ---
CHECKSUM="$(checksum256 "$BACKUP_PATH")"
SIZE="$(stat -f%z "$BACKUP_PATH" 2>/dev/null || stat -c%s "$BACKUP_PATH" 2>/dev/null || echo 0)"
log INFO "Checksum SHA256: $CHECKSUM"
printf '%s  %s\n' "$CHECKSUM" "$(basename "$BACKUP_PATH")" > "$BACKUP_PATH.sha256"
log INFO "Tamaño: $SIZE bytes"

# --- Paso 5: Rotación ---
log INFO "Rotación: eliminando backups > ${BACKUP_RETENTION_DAYS} días"
find "$BACKUP_DIR" -name "backup_*.tar.gz" -mtime +"$BACKUP_RETENTION_DAYS" -delete 2>/dev/null || true
find "$BACKUP_DIR" -name "backup_*.sha256" -mtime +"$BACKUP_RETENTION_DAYS" -delete 2>/dev/null || true
find "$BACKUP_DIR" -name "metadata_*.db" -mtime +"$BACKUP_RETENTION_DAYS" -delete 2>/dev/null || true
find "$BACKUP_DIR" -name "storage_*.tar.gz" -mtime +"$BACKUP_RETENTION_DAYS" -delete 2>/dev/null || true

# --- Paso 6: Subida a Google Drive (opcional) ---
# Compatibilidad bash 3.2 (macOS): usar tr para lowercase
_gdrive_enabled="$(printf '%s' "${BACKUP_GDRIVE_ENABLED}" | tr '[:upper:]' '[:lower:]')"
if [[ "${_gdrive_enabled}" == "true" ]]; then
    if command -v rclone >/dev/null 2>&1; then
        log INFO "Subiendo backup a Google Drive (remote: ${BACKUP_GDRIVE_REMOTE}, path: ${BACKUP_GDRIVE_PATH})"
        RCLONE_CMD=(rclone copy "$BACKUP_PATH" "${BACKUP_GDRIVE_REMOTE}:${BACKUP_GDRIVE_PATH}/")
        if [[ -n "$RCLONE_CONFIG" ]]; then
            RCLONE_CMD=(rclone --config "$RCLONE_CONFIG" copy "$BACKUP_PATH" "${BACKUP_GDRIVE_REMOTE}:${BACKUP_GDRIVE_PATH}/")
        fi
        if [[ -n "$BACKUP_GDRIVE_ARGS" ]]; then
            # shellcheck disable=SC2206
            RCLONE_CMD+=($BACKUP_GDRIVE_ARGS)
        fi
        if "${RCLONE_CMD[@]}" 2>&1 | tee -a "$SF_BACKUP_LOG"; then
            log INFO "Subida a Google Drive completada exitosamente"
        else
            log WARN "rclone falló (backup local OK en $BACKUP_PATH). Revisa conectividad y configuración de rclone."
        fi
    else
        log WARN "BACKUP_GDRIVE_ENABLED=true pero rclone no está instalado. Saltando subida. Instala: https://rclone.org/install.sh"
    fi
fi

log INFO "Backup completado exitosamente: $BACKUP_PATH"
log INFO "---"
