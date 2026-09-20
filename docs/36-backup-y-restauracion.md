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

### Variables de entorno

| Variable | Default | Descripción |
|----------|---------|-------------|
| `SF_DATABASE_URL` | `sqlite:data/syncfiles.db` | URL de la BD (resuelta a absoluta). |
| `SF_STORAGE_ROOT` | `data/storage` | Raíz del storage (resuelta a absoluta). |
| `BACKUP_DIR` | `data/backups` | Directorio de backups. |
| `BACKUP_RETENTION_DAYS` | `30` | Días a retener. |
| `SF_BACKUP_LOG` | `data/backup.log` | Archivo de log. |

### Uso

```bash
# Backup manual
./scripts/backup.sh

# Desde cron (diario a las 2 AM)
0 2 * * * /opt/syncfiles/scripts/backup.sh >> /var/log/syncfiles/backup.log 2>&1
```

### Validación

```bash
sqlite3 data/backups/metadata_*.db "PRAGMA integrity_check"
tar -tzf data/backups/backup_*.tar.gz | head
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
