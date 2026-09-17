//! Endpoint de backup manual: POST /admin/backup
//!
//! Protegido por token de administrador (SF_ADMIN_TOKEN). Ejecuta VACUUM INTO
//! del metadata + tar del storage. Devuelve path y checksum del backup.

use std::sync::Arc;

use actix_web::{post, web, HttpRequest, HttpResponse, HttpMessage, Result as ActixResult};
use serde_json::json;
use sha2::Digest;

use crate::state::AppState;

#[post("/backup")]
pub async fn backup_handler(
    state: web::Data<Arc<AppState>>,
    req: HttpRequest,
) -> ActixResult<HttpResponse> {
    let request_id = req
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_default();

    let admin_token = std::env::var("SF_ADMIN_TOKEN");
    if let Ok(expected) = admin_token {
        let provided = req
            .headers()
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .unwrap_or("");
        if provided != expected {
            return Ok(HttpResponse::Unauthorized().json(json!({
                "code": "UNAUTHORIZED",
                "message": "Token de administrador inválido",
                "request_id": request_id,
            })));
        }
    } else {
        return Ok(HttpResponse::Forbidden().json(json!({
            "code": "ADMIN_NOT_CONFIGURED",
            "message": "SF_ADMIN_TOKEN no está configurado",
            "request_id": request_id,
        })));
    }

    if let Err(e) = run_backup(&state).await {
        tracing::error!(
            request_id = %request_id,
            error = %e,
            "Backup manual falló"
        );
        return Ok(HttpResponse::InternalServerError().json(json!({
            "code": "BACKUP_FAILED",
            "message": e.to_string(),
            "request_id": request_id,
        })));
    }

    let backup_info = state
        .last_backup
        .read()
        .unwrap()
        .clone();

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "backup_path": backup_info.path,
        "checksum_sha256": backup_info.checksum,
        "size_bytes": backup_info.size_bytes,
        "request_id": request_id,
    })))
}

#[derive(Debug, Clone, Default)]
pub struct BackupInfo {
    pub path: String,
    pub checksum: String,
    pub size_bytes: u64,
}

pub async fn run_backup(state: &Arc<AppState>) -> anyhow::Result<()> {
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_dir = std::env::var("BACKUP_DIR")
        .unwrap_or_else(|_| "data/backups".to_string());
    let backup_dir = std::path::Path::new(&backup_dir);
    std::fs::create_dir_all(backup_dir)?;

    let config = &state.config;

    if !config.database_url.starts_with("sqlite:") {
        return Err(anyhow::anyhow!("database_url no es sqlite"));
    }

    let metadata_backup = backup_dir.join(format!("metadata_{}.db", timestamp));
    let vacuum_sql = format!(
        "VACUUM INTO '{}'",
        metadata_backup.to_string_lossy()
    );
    sqlx::query(&vacuum_sql).execute(&state.pool).await?;
    tracing::info!("VACUUM INTO backup: {}", metadata_backup.display());

    let storage_backup = backup_dir.join(format!("storage_{}.tar.gz", timestamp));
    let storage_root = std::path::Path::new(&config.storage_root);
    let storage_tar = std::process::Command::new("tar")
        .arg("-czf")
        .arg(&storage_backup)
        .arg("-C")
        .arg(storage_root)
        .arg("users/")
        .output()?;
    if !storage_tar.status.success() {
        return Err(anyhow::anyhow!(
            "tar storage falló: {}",
            String::from_utf8_lossy(&storage_tar.stderr)
        ));
    }

    // Combinar en archive unified
    let combined = backup_dir.join(format!("backup_{}.tar.gz", timestamp));
    let combined_tar = std::process::Command::new("tar")
        .arg("-czf")
        .arg(&combined)
        .arg("-C")
        .arg(backup_dir)
        .arg(format!("metadata_{}.db", timestamp))
        .arg(format!("storage_{}.tar.gz", timestamp))
        .output()?;
    if !combined_tar.status.success() {
        return Err(anyhow::anyhow!(
            "tar combinado falló: {}",
            String::from_utf8_lossy(&combined_tar.stderr)
        ));
    }

    let checksum = compute_sha256(&combined)?;
    let size_bytes = std::fs::metadata(&combined)?.len();

    let mut last = state.last_backup.write().unwrap();
    *last = BackupInfo {
        path: combined.to_string_lossy().to_string(),
        checksum,
        size_bytes,
    };

    tracing::info!(
        "Backup completado: {} (sha256={})",
        combined.display(),
        last.checksum
    );

    Ok(())
}

fn compute_sha256(path: &std::path::Path) -> anyhow::Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut buf = [0u8; 65536];
    let mut hasher = sha2::Sha256::new();
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}
