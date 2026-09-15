//! Middleware: trazas estructuradas, validación de Content-Type y rate limiting.
//!
//! Se registran en main.rs como `from_fn` closures. Los middlewares que
//! descartan la solicitud devuelven un `HttpResponse`; los que dejan pasar
//! devuelven `ServiceResponse<BoxBody>`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use actix_web::body::{BoxBody, MessageBody};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header::{CACHE_CONTROL, HeaderValue};
use actix_web::middleware::Next;
use actix_web::web::Data;
use actix_web::{HttpMessage, HttpResponse};

/// Contexto de solicitud: correlation ID + tiempo de llegada.
///
/// Uso: `.wrap(from_fn(trace_context))` en la App.
pub async fn trace_context(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<BoxBody>, actix_web::Error> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let start = Instant::now();
    req.extensions_mut().insert::<String>(request_id.clone());
    req.extensions_mut().insert::<Instant>(start);

    let method = req.method().to_string();
    let path = req.path().to_owned();

    let res = next.call(req).await;

    let elapsed = start.elapsed();
    let status = match &res {
        Ok(r) => r.status().as_u16().to_string(),
        Err(_) => "ERR".to_string(),
    };

    tracing::info!(
        request_id = %request_id,
        method = %method,
        path = %path,
        status = %status,
        elapsed_ms = elapsed.as_millis(),
        "solicitud procesada"
    );

    res.map(|r| r.map_into_boxed_body())
}

/// Rate limiting por IP (token bucket simple en memoria).
///
/// Límite configurable vía `SF_RATE_LIMIT_RPS` (default 60) y
/// `SF_RATE_LIMIT_BURST` (default 120). Para despliegues multi-instancia
/// usar Redis; en V1 la memoria es suficiente para un único node.
pub struct RateLimiter {
    rps: u64,
    burst: u64,
    state: Mutex<HashMap<String, (u64, Instant)>>,
}

impl RateLimiter {
    pub fn new(rps: u64, burst: u64) -> Self {
        Self {
            rps,
            burst,
            state: Mutex::new(HashMap::new()),
        }
    }

    pub fn allow(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut state = self.state.lock().unwrap();
        let (count, last) = state.entry(key.to_string()).or_insert((0, now));
        let elapsed = now.duration_since(*last).as_secs_f64();
        if elapsed >= 1.0 {
            *count = 0;
            *last = now;
        }
        let tokens = (*count as f64) + elapsed * self.rps as f64;
        if tokens < self.burst as f64 {
            *count = (*count).saturating_add(1);
            true
        } else {
            false
        }
    }
}

/// Helper: construir el RateLimiter a partir de variables de entorno.
pub fn build_rate_limiter() -> Arc<RateLimiter> {
    let rps = std::env::var("SF_RATE_LIMIT_RPS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60);
    let burst = std::env::var("SF_RATE_LIMIT_BURST")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(120);
    Arc::new(RateLimiter::new(rps, burst))
}

/// Closure para `from_fn`: aplica rate limiting a /api/ y /health/.
pub async fn rate_limit_middleware(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<BoxBody>, actix_web::Error> {
    let path = req.path().to_owned();
    if !path.starts_with("/api/") && !path.starts_with("/health/") {
        return Ok(next.call(req).await?.map_into_boxed_body());
    }

    let limiter = req
        .app_data::<Data<Arc<RateLimiter>>>()
        .expect("RateLimiter debe estar en AppData")
        .as_ref()
        .clone();

    let key = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .or_else(|| req.peer_addr().map(|a| a.to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    if !limiter.allow(&key) {
        let request_id = req
            .extensions()
            .get::<String>()
            .cloned()
            .unwrap_or_default();
        let resp = HttpResponse::TooManyRequests()
            .append_header(("Retry-After", "60"))
            .json(serde_json::json!({
                "code": "RATE_LIMITED",
                "message": "Demasiadas solicitudes. Inténtalo de nuevo más tarde.",
                "retryable": true,
                "request_id": request_id,
            }));
        return Ok(req.into_response(resp));
    }

    Ok(next.call(req).await?.map_into_boxed_body())
}

/// Closure para `from_fn`: Content-Type application/json en /api/.
pub async fn content_type_middleware(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<BoxBody>, actix_web::Error> {
    if req.path().starts_with("/api/") {
        if let Some(ct) = req.headers().get("Content-Type") {
            let ct = ct.to_str().unwrap_or("");
            if !ct.starts_with("application/json") {
                let request_id = req
                    .extensions()
                    .get::<String>()
                    .cloned()
                    .unwrap_or_default();
                let resp = HttpResponse::UnsupportedMediaType()
                    .json(serde_json::json!({
                        "code": "INVALID_CONTENT_TYPE",
                        "message": "Content-Type debe ser application/json",
                        "retryable": false,
                        "request_id": request_id,
                    }));
                return Ok(req.into_response(resp));
            }
        }
    }
    Ok(next.call(req).await?.map_into_boxed_body())
}

/// Estáticos sin cache agresivo (excepto API y health).
pub async fn no_cache_statics(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<BoxBody>, actix_web::Error> {
    let path = req.path().to_owned();
    let mut res = next.call(req).await?;
    if !path.starts_with("/api/") && !path.starts_with("/health/") {
        res.headers_mut().insert(
            CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        );
    }
    Ok(res.map_into_boxed_body())
}