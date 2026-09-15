//! Sistema de migraciones versionado (sin dependencia sqlx-migrate).
//!
//! Las migraciones se almacenan en `syncfiles-server/migrations/` como archivos
//! `.sql` numerados (001, 002, ...). Cada una es idempotente (CREATE TABLE IF NOT EXISTS).
//!
//! La tabla `_migrations` registra los scripts aplicados. Al arrancar, el server
//! aplica los scripts pendientes en orden. Es compatible con la base de datos
//! existente (SERVER_INIT_SQL en syncfiles-models ya crea las tablas con IF NOT EXISTS).

use sqlx::SqlitePool;
use std::fs;
use std::path::Path;

/// Nombre de la tabla de control de migraciones.
const MIGRATIONS_TABLE: &str = "_migrations";

/// Ejecuta las migraciones pendientes desde el directorio dado.
/// Los archivos deben estar numerados como `NNN__nombre.sql`.
pub async fn run_migrations(pool: &SqlitePool, migrations_dir: &Path) -> anyhow::Result<usize> {
    ensure_migrations_table(pool).await?;

    let mut applied = applied_versions(pool).await?;
    let mut files: Vec<_> = fs::read_dir(migrations_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |e| e == "sql"))
        .collect();
    files.sort();

    let mut count = 0;
    for path in &files {
        let version = file_version(path);
        if version == 0 || applied.contains(&version) {
            continue;
        }
        let sql = fs::read_to_string(path)?;
        // Cada script es idempotente (IF NOT EXISTS) y puede tener múltiples statements.
        let mut tx = pool.begin().await?;
        sqlx::query(&sql).execute(&mut *tx).await?;
        sqlx::query(&format!(
            "INSERT INTO {} (version, name) VALUES (?, ?)",
            MIGRATIONS_TABLE
        ))
        .bind(version)
        .bind(path.file_stem().unwrap_or_default().to_string_lossy().to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        applied.insert(version);
        count += 1;
        tracing::info!("Migración {} aplicada", version);
    }

    Ok(count)
}

/// Crea la tabla de control si no existe.
async fn ensure_migrations_table(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(&format!(
        "CREATE TABLE IF NOT EXISTS {} (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at INTEGER NOT NULL DEFAULT (strftime('%s','now') * 1000)
        )",
        MIGRATIONS_TABLE
    ))
    .execute(pool)
    .await?;
    Ok(())
}

/// Devuelve el conjunto de versiones ya aplicadas.
async fn applied_versions(pool: &SqlitePool) -> anyhow::Result<std::collections::HashSet<i64>> {
    let rows: Vec<(i64,)> = sqlx::query_as(&format!(
        "SELECT version FROM {} ORDER BY version",
        MIGRATIONS_TABLE
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(v,)| v).collect())
}

/// Extrae el número de versión del archivo (ej: `001__init.sql` → 1).
fn file_version(path: &Path) -> i64 {
    path.file_stem()
        .and_then(|s| s.to_str())
        .and_then(|s| s.split('_').next())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0)
}