# Recuperación de backups y restauración

## Qué contiene un backup

`backup_YYYYMMDD_HHMMSS.tar.gz` contiene:

- `metadata_YYYYMMDD_HHMMSS.db`: SQLite con usuarios, sesiones, change log, cola, conflictos y auditoría.
- `storage_YYYYMMDD_HHMMSS.tar.gz`: árbol `users/` con los archivos y conflictos.
- `backup_YYYYMMDD_HHMMSS.tar.gz.sha256`: checksum generado por `scripts/backup.sh`.

El backup de metadatos se crea con `VACUUM INTO`; el storage se comprime con `tar`. Para una restauración coherente, detenga el servidor antes de una restauración de emergencia.

## Crear y verificar un backup

```bash
SF_DATABASE_URL=sqlite:/var/lib/syncfiles/db/syncfiles.db \
SF_STORAGE_ROOT=/var/lib/syncfiles/storage \
BACKUP_DIR=/var/lib/syncfiles/backups \
scripts/backup.sh
```

Verificación:

```bash
sha256sum -c /var/lib/syncfiles/backups/backup_*.tar.gz.sha256
sqlite3 /var/lib/syncfiles/backups/metadata_*.db 'PRAGMA integrity_check;'
tar -tzf /var/lib/syncfiles/backups/backup_*.tar.gz >/dev/null
```

Copie el archive y el `.sha256` a un destino externo. No borre el último backup local hasta verificar la copia externa.

## Restaurar en Linux/systemd

1. Detenga el servicio:

```bash
sudo systemctl stop syncfiles-server
```

2. Ejecute el restaurador. Por seguridad exige `SF_RESTORE_FORCE=1` cuando el destino ya existe y conserva el estado anterior con sufijo `.pre-restore-*`:

```bash
sudo SF_DATABASE_URL=sqlite:/var/lib/syncfiles/db/syncfiles.db \
   SF_STORAGE_ROOT=/var/lib/syncfiles/storage \
   SF_RESTORE_LOG=/var/lib/syncfiles/restores.log \
   SF_RESTORE_FORCE=1 \
   /opt/syncfiles/restore.sh /var/lib/syncfiles/backups/backup_20260920_020000.tar.gz
```

El script verifica el checksum, extrae en un directorio temporal, valida `PRAGMA integrity_check`, comprueba que cada archivo activo existe y coincide con su SHA-256/tamaño, y solo entonces reemplaza el destino.

3. Arranque y valide:

```bash
sudo systemctl start syncfiles-server
curl -fsS http://127.0.0.1:8080/health/ready
sqlite3 /var/lib/syncfiles/db/syncfiles.db 'PRAGMA integrity_check;'
```

Haga login y pruebe un upload/download de un archivo pequeño. El script registra resultado, ruta del backup y duración en `restores.log`.

## Restaurar en Docker Compose

Detenga el stack y ejecute el restaurador dentro de la imagen:

```bash
docker compose down
docker compose run --rm --no-deps \
  --entrypoint /scripts/restore.sh \
  -e SF_DATABASE_URL=sqlite:/data/db/syncfiles.db \
  -e SF_STORAGE_ROOT=/data/storage \
  -e SF_RESTORE_FORCE=1 \
  server /data/backups/backup_20260920_020000.tar.gz
docker compose up -d
```

Si el backup está en el host, primero copie el archivo al volumen o monte el directorio de backups. No ejecute el servidor y el restore contra el mismo SQLite simultáneamente.

## Rollback manual

Si el restore falla antes de reemplazar el destino, el script no modifica los datos. Si falla después del reemplazo, conserva las rutas `.pre-restore-*`; detenga el servicio y copie la ruta anterior de vuelta antes de reiniciar. No elimine esas copias hasta completar la validación post-restore.

## Checklist post-restore

- [ ] checksum del archive coincide con `.sha256`
- [ ] `PRAGMA integrity_check` devuelve `ok`
- [ ] tabla `_migrations` y tablas de SyncFiles existen
- [ ] `/health/ready` devuelve 200
- [ ] login funciona
- [ ] upload/download de prueba funciona
- [ ] checksums y tamaños de archivos activos coinciden
- [ ] `change_log` y `audit_log` son legibles
- [ ] copia externa del backup está verificada
