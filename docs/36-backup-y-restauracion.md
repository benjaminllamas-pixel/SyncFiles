# Backup y Restauración

## 1. Estrategia de Backup

SyncFiles V1 protege dos componentes críticos:

1. **Metadatos (SQLite):** `data/syncfiles.db` — contiene usuarios, sesiones,
   change_log, files, sync_queue, audit_log.
2. **Almacenamiento de archivos:** `data/storage/` — contiene los archivos sincronizados
   por usuario/dispositivo, hashes, y copias de conflicto.

## 2. Objetivos (RPO/RTO)

| Métrica | Objetivo | Descripción |
|--------|----------|-------------|
| **RPO** (Recovery Point Objective) | ≤ 24 horas | Máxima pérdida de datos aceptable: 1 día. |
| **RTO** (Recovery Time Objective) | < 4 horas | Tiempo máximo para restaurar el servicio. |
| **Retención** | 4–12 semanas | Se conservan backups entre 4 y 12 semanas. |
| **Verificación** | 100% de restores | Todo backup restoreado se verifica con checksums. |

## 3. Backup Diario Automatizado (`scripts/backup.sh`)

El script `backup.sh`:

1. **`VACUUM INTO '<backup_dir>/metadata_<timestamp>.db'`** — copia atómica y
   online de la base de metadatos (SQLite 3.27+).
2. **`tar -czf storage-backup.tar.gz -C "$SF_STORAGE_ROOT" users/`** — archivo
   comprimido del storage.
3. **Combina** en un archive timestamped: `backup_YYYYMMDD_HHMM.tar.gz`.
4. **Rotación:** elimina backups con más de `BACKUP_RETENTION_DAYS`
   (default 30).
5. **Loguea** resultado a `$SF_BACKUP_LOG` (default: `data/backup.log`).
6. **(Opcional) Sube a Google Drive** via rclone si `BACKUP_GDRIVE_ENABLED=true`.

### Variables de entorno (core)

| Variable | Default | Descripción |
|----------|---------|-------------|
| `SF_DATABASE_URL` | `sqlite:data/syncfiles.db` | URL de la BD (resuelta a absoluta). |
| `SF_STORAGE_ROOT` | `data/storage` | Raíz del storage (resuelta a absoluta). |
| `BACKUP_DIR` | `data/backups` | Directorio de backups. |
| `BACKUP_RETENTION_DAYS` | `30` | Días a retener. |
| `SF_BACKUP_LOG` | `data/backup.log` | Archivo de log. |

### Variables de entorno (Google Drive / rclone) — **Todas opcionales**

| Variable | Default | Descripción |
|----------|---------|-------------|
| `BACKUP_GDRIVE_ENABLED` | `false` | `"true"` para activar subida a Google Drive. |
| `BACKUP_GDRIVE_REMOTE` | `gdrive` | Nombre del remote rclone configurado (`rclone config`). |
| `BACKUP_GDRIVE_PATH` | `SyncFilesBackups` | Carpeta destino en Google Drive (se crea automáticamente). |
| `BACKUP_GDRIVE_ARGS` | `""` | Args extra para `rclone copy` (ej: `--bwlimit 1M --progress`). |
| `RCLONE_CONFIG` | *(auto)* | Path a `rclone.conf` si no está en ubicación estándar (`~/.config/rclone/rclone.conf`). |

### Uso

```bash
# Backup manual (solo local)
./scripts/backup.sh

# Backup con subida a Google Drive
BACKUP_GDRIVE_ENABLED=true ./scripts/backup.sh

# Backup con configuración personalizada
BACKUP_GDRIVE_ENABLED=true \
BACKUP_GDRIVE_REMOTE=gdrive \
BACKUP_GDRIVE_PATH=SyncFilesBackups \
BACKUP_GDRIVE_ARGS="--bwlimit 2M --progress" \
./scripts/backup.sh

# Desde cron (diario a las 2 AM)
0 2 * * * /opt/syncfiles/scripts/backup.sh >> /var/log/syncfiles/backup.log 2>&1

# Desde cron con Google Drive
0 2 * * * BACKUP_GDRIVE_ENABLED=true BACKUP_GDRIVE_REMOTE=gdrive /opt/syncfiles/scripts/backup.sh >> /var/log/syncfiles/backup.log 2>&1
```

### Validación

```bash
sqlite3 data/backups/metadata_*.db "PRAGMA integrity_check"
tar -tzf data/backups/backup_*.tar.gz | head
```

### Requisitos previos para Google Drive

1. **Instalar rclone:**
   ```bash
   curl https://rclone.org/install.sh | sudo bash
   # o: brew install rclone / apt install rclone
   ```

2. **Configurar remote Google Drive (una sola vez):**
   ```bash
   rclone config
   # n) New remote
   # name: gdrive
   # type: drive
   # client_id/client_secret: (dejar vacío para usar app interna de rclone)
   # scope: drive.file  (solo archivos creados por la app)
   # Use auto config: Yes  (abre navegador para autorizar)
   ```

3. **Verificar:**
   ```bash
   rclone lsd gdrive:
   rclone ls gdrive:SyncFilesBackups
   ```

### Comportamiento y garantías

- **El backup LOCAL siempre se ejecuta primero.** La subida a Drive es best-effort.
- **Si rclone falla, el script NO falla** — logea `WARN` y continúa. El backup local queda intacto en `BACKUP_DIR`.
- **rclone usa su config estándar** (`~/.config/rclone/rclone.conf` o `%APPDATA%\rclone\rclone.conf` en Windows). Usa `RCLONE_CONFIG` para sobrescribir.
- **Cifrado en reposo (opcional):** Configura un remote `crypt` sobre `gdrive:` y úsalo como `BACKUP_GDRIVE_REMOTE=gdrive_crypt`.
- **Ancho de banda:** Usa `BACKUP_GDRIVE_ARGS="--bwlimit 1M"` para limitar subida.
- **Retención en Drive:** rclone no rota automáticamente. Usa `rclone delete --min-age 30d gdrive:SyncFilesBackups` en cron separado si necesitas retención remota.

### Ejemplo de log exitoso con Google Drive

```text
[2026-10-08T03:00:00Z] [INFO] Iniciando backup: backup_20261008_030000
[2026-10-08T03:00:01Z] [INFO] VACUUM INTO: /path/syncfiles.db → /path/backups/metadata_20261008_030000.db
[2026-10-08T03:00:02Z] [INFO] tar storage: /path/storage → /path/backups/storage_20261008_030000.tar.gz
[2026-10-08T03:00:03Z] [INFO] Archive combinado: /path/backups/backup_20261008_030000.tar.gz
[2026-10-08T03:00:03Z] [INFO] Checksum SHA256: a1b2c3...
[2026-10-08T03:00:03Z] [INFO] Tamaño: 1048576 bytes
[2026-10-08T03:00:03Z] [INFO] Rotación: eliminando backups > 30 días
[2026-10-08T03:00:03Z] [INFO] Subiendo backup a Google Drive (remote: gdrive, path: SyncFilesBackups)
[2026-10-08T03:00:15Z] [INFO] Subida a Google Drive completada exitosamente
[2026-10-08T03:00:15Z] [INFO] Backup completado exitosamente: /path/backups/backup_20261008_030000.tar.gz
```

## 4. Endpoint de Backup Manual (`POST /admin/backup`)

Protegido por token de administrador (`SF_ADMIN_TOKEN`). Ejecuta `VACUUM INTO`
del metadata + `tar` del storage. Devuelve path y checksum del backup.

**Request:**
```bash
curl -X POST -H "Authorization: Bearer <admin_token>" http://localhost:8080/admin/backup
```

**Response:**
```json
{
  "status": "ok",
  "backup_path": "/data/backups/backup_20260916_0200.tar.gz",
  "checksum_sha256": "a1b2c3...",
  "size_bytes": 1048576
}
```

## 5. Test E2E Backup y Restore (`scripts/e2e-backup-restore.sh`)

Script de validación que:

1. Inicia el server, sube archivos, verifica checksums.
2. Ejecuta backup (via endpoint o script).
3. Detiene el server, elimina DB y storage.
4. Restaura el backup (extrae tar.gz, restaura DB con `VACUUM INTO ... FROM`).
5. Reinicia el server, verifica que los archivos y checksums coinciden.

```bash
./scripts/e2e-backup-restore.sh
```

### 5.1. Evidencia de ejecución

Ejecutado el 2026-09-20 en macOS con el servidor y el CLI reconstruidos localmente:

```text
E2E Backup & Restore Test: PASS
Archivos restaurados: 2
test1.txt: b0ccf3faeb6bf186a82f532a8466b8a969592c20e978d5cab6e1ba76947ee0df
test2.bin: 527d55f48f1a6bb55b89fb52cd21901b72ab8d7dcea58f437a02862c4f2bbcf5
Backup verificado: /tmp/syncfiles-backup-e2e/backups/backup_20260919_210409.tar.gz
```

El script ahora usa `sha256sum`, `shasum` o Python según la plataforma y limpia el servidor en caso de fallo.

## 6. Restore manual y script automatizado

El procedimiento operativo completo está en [`docs/53-recuperacion-backups.md`](53-recuperacion-backups.md). El script `scripts/restore.sh` verifica el checksum, extrae el archive en un directorio temporal, valida SQLite, comprueba checksums/tamaños de los archivos activos y conserva el destino anterior con sufijo `.pre-restore-*`.

```bash
sudo systemctl stop syncfiles-server
sudo SF_DATABASE_URL=sqlite:/var/lib/syncfiles/db/syncfiles.db \
  SF_STORAGE_ROOT=/var/lib/syncfiles/storage \
  SF_RESTORE_FORCE=1 \
  /opt/syncfiles/restore.sh /var/lib/syncfiles/backups/backup_20260920_020000.tar.gz
sudo systemctl start syncfiles-server
```

No se usa `VACUUM INTO ... FROM`: la copia de backup ya es una base SQLite válida y se instala directamente tras validarse.

## 7. Rotación y Retención

| Política | Detalle |
|----------|---------|
| **Retención** | 4 semanas mínimo, 12 semanas máximo. |
| **Rotación** | `backup.sh` elimina backups > `BACKUP_RETENTION_DAYS` (default 30). |
| **Offsite** | En despliegue Docker/Cloud Run, los backups se suben a Cloud Storage/S3. |
| **Incremental** | En V1 los backups son full diarios. En V2 se añadirá incremental (WAL + delta). |

## 8. Registro de Restauraciones

Cada restauración se registra en `data/restores.log` con:
- Timestamp UTC
- Backup restaurado (filename)
- Usuario que ejecutó el restore
- Resultado (ok/fail)
- Duración

```bash
echo "$(date -u) RESTORE backup=$BACKUP_FILE user=$USER result=$RESULT duration=$DURATION" >> data/restores.log
```

## 9. Checklist de Verificación Post-Restore

| Paso | Verificación |
|------|-------------|
| 1 | `PRAGMA integrity_check` = `ok` |
| 2 | `_migrations` tabla accesible |
| 3 | `/health/ready` responde 200 |
| 4 | Login con credenciales admin funciona |
| 5 | Upload/download de archivo de prueba funciona |
| 6 | Checksums de archivos coinciden con backup |
| 7 | change_log tiene datos consistentes |
| 8 | audit_log preservado |

## 10. Dual-Write Opcional (V1 stub)

`SF_DUAL_WRITE_TARGET=<path>` — cuando está activado, cada escritura de archivo
se replica también al target path. Es un mecanismo de migración hacia NAS/PostgreSQL.
En V1 es documentación + stub; la implementación completa viene en V2.
