//! Background jobs: limpieza de logs, verificación de FK, vacuum, prunes.
//!
//! Spawned desde main.rs con tokio::task::spawn. Cada job ejecuta periódicamente:
//! - prune_audit_log — diario (>90 días)
//! - check_foreign_keys — diario
//! - vacuum_optimize — semanal
//! - prune_sessions_expired — diario (>7 días)
//! - prune_idempotency_keys — diario (>30 días)

use sqlx::SqlitePool;
use std::time::Duration;

const AUDIT_RETENTION_DAYS: i64 = 90;
const SESSION_RETENTION_DAYS: i64 = 7;
const IDEMPOTENCY_RETENTION_DAYS: i64 = 30;

pub fn spawn_background_jobs(pool: SqlitePool) {
    // prune_audit_log — diario
    let pool_a = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(86400));
        loop {
            interval.tick().await;
            if let Err(e) = prune_audit_log(&pool_a).await {
                tracing::error!("prune_audit_log falló: {}", e);
            }
        }
    });

    // check_foreign_keys — diario
    let pool_b = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(86400));
        loop {
            interval.tick().await;
            if let Err(e) = check_foreign_keys(&pool_b).await {
                tracing::error!("check_foreign_keys falló: {}", e);
            }
        }
    });

    // vacuum_optimize — semanal (7 días)
    let pool_c = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(7 * 86400));
        loop {
            interval.tick().await;
            if let Err(e) = vacuum_optimize(&pool_c).await {
                tracing::error!("vacuum_optimize falló: {}", e);
            }
        }
    });

    // prune_sessions_expired — diario
    let pool_d = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(86400));
        loop {
            interval.tick().await;
            if let Err(e) = prune_sessions_expired(&pool_d).await {
                tracing::error!("prune_sessions_expired falló: {}", e);
            }
        }
    });

    // prune_idempotency_keys — diario
    let pool_e = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(86400));
        loop {
            interval.tick().await;
            if let Err(e) = prune_idempotency_keys(&pool_e).await {
                tracing::error!("prune_idempotency_keys falló: {}", e);
            }
        }
    });
}

async fn prune_audit_log(pool: &SqlitePool) -> anyhow::Result<()> {
    let cutoff = chrono::Utc::now().timestamp_millis() - (AUDIT_RETENTION_DAYS * 86400 * 1000);
    let result = sqlx::query("DELETE FROM audit_log WHERE created_at < ?")
        .bind(cutoff)
        .execute(pool)
        .await?;
    tracing::info!(
        "prune_audit_log: {} filas eliminadas",
        result.rows_affected()
    );
    Ok(())
}

async fn check_foreign_keys(pool: &SqlitePool) -> anyhow::Result<()> {
    let rows: Vec<(i64,)> = sqlx::query_as("PRAGMA foreign_key_check")
        .fetch_all(pool)
        .await?;
    if rows.is_empty() {
        tracing::info!("check_foreign_keys: OK (sin violaciones)");
    } else {
        tracing::warn!("check_foreign_keys: {} violaciones detectadas", rows.len());
    }
    Ok(())
}

async fn vacuum_optimize(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query("VACUUM").execute(pool).await?;
    sqlx::query("PRAGMA optimize").execute(pool).await?;
    tracing::info!("vacuum_optimize: completado");
    Ok(())
}

async fn prune_sessions_expired(pool: &SqlitePool) -> anyhow::Result<()> {
    let cutoff = chrono::Utc::now().timestamp_millis() - (SESSION_RETENTION_DAYS * 86400 * 1000);
    let result = sqlx::query("DELETE FROM sessions WHERE status != 'active' AND created_at < ?")
        .bind(cutoff)
        .execute(pool)
        .await?;
    tracing::info!(
        "prune_sessions_expired: {} filas eliminadas",
        result.rows_affected()
    );
    Ok(())
}

async fn prune_idempotency_keys(pool: &SqlitePool) -> anyhow::Result<()> {
    let cutoff =
        chrono::Utc::now().timestamp_millis() - (IDEMPOTENCY_RETENTION_DAYS * 86400 * 1000);
    let result = sqlx::query("DELETE FROM idempotency_keys WHERE created_at < ?")
        .bind(cutoff)
        .execute(pool)
        .await?;
    tracing::info!(
        "prune_idempotency_keys: {} filas eliminadas",
        result.rows_affected()
    );
    Ok(())
}
