//! Health check endpoints: liveness + readiness.
//!
//! - GET /health/live  → 200 siempre (proceso vivo)
//! - GET /health/ready → 200 si la DB y storage están disponibles; 503 si no.

use std::sync::Arc;

use actix_web::{get, web, HttpMessage, HttpRequest, HttpResponse, Result as ActixResult};

use crate::state::AppState;

#[get("/live")]
pub async fn liveness() -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "alive",
        "service": "syncfiles-server",
        "version": env!("CARGO_PKG_VERSION"),
    })))
}

#[get("/ready")]
pub async fn readiness(
    state: web::Data<Arc<AppState>>,
    req: HttpRequest,
) -> ActixResult<HttpResponse> {
    let request_id = req
        .extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_default();

    // Chequeo DB: pool vivo?
    let db_ok = state.pool.acquire().await.is_ok();

    // Chequeo storage: escribir y borrar un archivo de healthcheck.
    // Opcional via SF_HEALTH_PROBE_WRITE=0 (para CI donde storage puede no
    // tener permisos de escritura).
    let write_probe = std::env::var("SF_HEALTH_PROBE_WRITE")
        .ok()
        .map(|v| v != "0" && v.to_lowercase() != "false")
        .unwrap_or(true);
    let storage_ok = if write_probe {
        match state
            .storage
            .write("__healthcheck__", "ping.txt", b"ping")
            .await
        {
            Ok(()) => {
                let _ = state.storage.delete("__healthcheck__", "ping.txt").await;
                true
            }
            Err(_) => false,
        }
    } else {
        true
    };

    if db_ok && storage_ok {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "status": "ready",
            "checks": {
                "database": "ok",
                "storage": "ok",
            },
            "request_id": request_id,
        })))
    } else {
        Ok(HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "not_ready",
            "checks": {
                "database": if db_ok { "ok" } else { "fail" },
                "storage": if storage_ok { "ok" } else { "fail" },
            },
            "request_id": request_id,
        })))
    }
}
