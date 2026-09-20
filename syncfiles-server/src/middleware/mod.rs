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
use actix_web::http::header::{HeaderValue, CACHE_CONTROL, CONTENT_LENGTH};
use actix_web::http::Method;
use actix_web::middleware::Next;
use actix_web::web::{Bytes, Data};
use actix_web::{HttpMessage, HttpResponse};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use futures_util::StreamExt;
use percent_encoding::percent_decode_str;
use serde_json::Value;

use crate::storage::normalize_relative_path;

const DEFAULT_MAX_UPLOAD_BYTES: usize = 104_857_600;

#[derive(Debug)]
struct Violation {
    code: &'static str,
    message: String,
}

fn request_id(req: &ServiceRequest) -> String {
    req.extensions()
        .get::<String>()
        .cloned()
        .unwrap_or_default()
}

fn rejection(
    status: actix_web::http::StatusCode,
    code: &'static str,
    message: String,
    req: &ServiceRequest,
) -> HttpResponse {
    HttpResponse::build(status).json(serde_json::json!({
        "code": code,
        "message": message,
        "retryable": false,
        "request_id": request_id(req),
    }))
}

fn has_request_body(method: &Method) -> bool {
    method == Method::POST || method == Method::PUT || method == Method::PATCH
}

fn is_json_content_type(req: &ServiceRequest) -> bool {
    req.headers()
        .get("Content-Type")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .map(|value| value.eq_ignore_ascii_case("application/json"))
        .unwrap_or(false)
}

pub fn body_limit_for(max_upload_bytes: usize) -> usize {
    max_upload_bytes.saturating_mul(4).saturating_add(1_048_576)
}

fn valid_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_identifier(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value.chars().all(|character| !character.is_control())
}

fn valid_path(value: &str, allow_empty: bool) -> bool {
    if allow_empty && value.is_empty() {
        return true;
    }
    if value.is_empty()
        || value.contains('\0')
        || value.starts_with('/')
        || value.starts_with('\\')
        || value.replace('\\', "/").split('/').any(|part| part == "..")
    {
        return false;
    }
    normalize_relative_path(value)
        .map(|normalized| normalized != ".")
        .unwrap_or(false)
}

fn string_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    max_len: usize,
    allow_empty: bool,
) -> Result<&'a str, Violation> {
    let value = object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| Violation {
            code: "INVALID_PAYLOAD",
            message: format!("El campo {key} debe ser una cadena"),
        })?;
    if (!allow_empty && value.is_empty())
        || value.len() > max_len
        || value.chars().any(|character| character.is_control())
    {
        return Err(Violation {
            code: "INVALID_PAYLOAD",
            message: format!("El campo {key} tiene un tamaño o formato inválido"),
        });
    }
    Ok(value)
}

fn integer_field(object: &serde_json::Map<String, Value>, key: &str) -> Result<i64, Violation> {
    let value = object.get(key).ok_or_else(|| Violation {
        code: "INVALID_PAYLOAD",
        message: format!("El campo {key} es obligatorio"),
    })?;
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
        .ok_or_else(|| Violation {
            code: "INVALID_PAYLOAD",
            message: format!("El campo {key} debe ser un entero"),
        })
}

fn boolean_field(object: &serde_json::Map<String, Value>, key: &str) -> Result<bool, Violation> {
    object
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| Violation {
            code: "INVALID_PAYLOAD",
            message: format!("El campo {key} debe ser booleano"),
        })
}

fn validate_path_field(
    object: &serde_json::Map<String, Value>,
    key: &str,
    allow_empty: bool,
) -> Result<(), Violation> {
    let value = string_field(object, key, 4096, allow_empty)?;
    if !valid_path(value, allow_empty) {
        return Err(Violation {
            code: "INVALID_PATH",
            message: format!("El campo {key} contiene una ruta inválida"),
        });
    }
    Ok(())
}

fn validate_payload_structure(
    path: &str,
    value: &Value,
    max_upload_bytes: usize,
) -> Result<(), Violation> {
    let object = value.as_object().ok_or_else(|| Violation {
        code: "INVALID_PAYLOAD",
        message: "El payload JSON debe ser un objeto".to_string(),
    })?;

    match path {
        "/api/v1/auth/login" => {
            string_field(object, "email", 320, false)?;
            string_field(object, "password", 1024, false)?;
            string_field(object, "device_id", 256, false)?;
        }
        "/api/v1/sync/diff" => {
            string_field(object, "session_id", 256, false)?;
            string_field(object, "device_id", 256, false)?;
            string_field(object, "request_id", 256, false)?;
            integer_field(object, "since")?;
        }
        "/api/v1/sync/upload" => {
            string_field(object, "session_id", 256, false)?;
            string_field(object, "device_id", 256, false)?;
            string_field(object, "file_id", 256, true)?;
            let relative_path = string_field(object, "relative_path", 4096, false)?;
            let path_hash = string_field(object, "path_hash", 128, false)?;
            let checksum = string_field(object, "checksum", 128, false)?;
            let size_bytes = integer_field(object, "size_bytes")?;
            integer_field(object, "modified_at")?;
            let idempotency_key = string_field(object, "idempotency_key", 256, false)?;
            let content = string_field(
                object,
                "content",
                max_upload_bytes.saturating_mul(4).saturating_add(1_048_576),
                false,
            )?;

            if !valid_path(relative_path, false) {
                return Err(Violation {
                    code: "INVALID_PATH",
                    message: "relative_path contiene una ruta inválida".to_string(),
                });
            }
            if !valid_hex(path_hash)
                || path_hash != syncfiles_models::compute_path_hash(relative_path)
            {
                return Err(Violation {
                    code: "INVALID_PATH_HASH",
                    message: "path_hash no coincide con relative_path".to_string(),
                });
            }
            if !valid_hex(checksum) {
                return Err(Violation {
                    code: "INVALID_CHECKSUM",
                    message: "checksum debe ser SHA-256 hexadecimal".to_string(),
                });
            }
            if size_bytes < 0 || size_bytes as usize > max_upload_bytes {
                return Err(Violation {
                    code: "PAYLOAD_TOO_LARGE",
                    message: "El archivo excede SF_MAX_UPLOAD_BYTES".to_string(),
                });
            }
            let decoded = BASE64.decode(content).map_err(|_| Violation {
                code: "INVALID_CONTENT",
                message: "content debe ser base64 válido".to_string(),
            })?;
            if decoded.len() != size_bytes as usize {
                return Err(Violation {
                    code: "INVALID_CONTENT",
                    message: "size_bytes no coincide con content".to_string(),
                });
            }
            if !valid_identifier(idempotency_key, 256) {
                return Err(Violation {
                    code: "INVALID_PAYLOAD",
                    message: "idempotency_key tiene un formato inválido".to_string(),
                });
            }
        }
        "/api/v1/sync/download" | "/api/v1/sync/delete" => {
            string_field(object, "session_id", 256, false)?;
            string_field(object, "device_id", 256, false)?;
            string_field(object, "file_id", 256, false)?;
            string_field(object, "path_hash", 128, true)?;
            string_field(object, "idempotency_key", 256, false)?;
        }
        "/api/v1/sync/rename" | "/api/v1/sync/move" => {
            string_field(object, "session_id", 256, false)?;
            string_field(object, "device_id", 256, false)?;
            string_field(object, "file_id", 256, false)?;
            validate_path_field(object, "old_path", true)?;
            validate_path_field(object, "new_path", false)?;
            string_field(object, "idempotency_key", 256, false)?;
        }
        "/api/v1/sync/copy" => {
            string_field(object, "session_id", 256, false)?;
            string_field(object, "device_id", 256, false)?;
            string_field(object, "file_id", 256, false)?;
            validate_path_field(object, "source_path", false)?;
            validate_path_field(object, "destination_path", false)?;
            string_field(object, "idempotency_key", 256, false)?;
        }
        "/api/v1/conflicts/resolve" => {
            string_field(object, "session_id", 256, false)?;
            string_field(object, "device_id", 256, false)?;
            string_field(object, "conflict_id", 256, false)?;
            string_field(object, "decision", 64, false)?;
            boolean_field(object, "preserve_alternative")?;
            if let Some(new_name) = object.get("new_name") {
                let new_name = new_name.as_str().ok_or_else(|| Violation {
                    code: "INVALID_PAYLOAD",
                    message: "new_name debe ser una cadena o null".to_string(),
                })?;
                if !new_name.is_empty() && !valid_path(new_name, false) {
                    return Err(Violation {
                        code: "INVALID_PATH",
                        message: "new_name contiene una ruta inválida".to_string(),
                    });
                }
            }
        }
        "/api/v1/devices/revoke" => {
            string_field(object, "session_id", 256, false)?;
            string_field(object, "device_id", 256, false)?;
            string_field(object, "target_device_id", 256, false)?;
        }
        _ => {}
    }

    Ok(())
}

fn request_target_is_safe(path: &str) -> bool {
    let decoded = percent_decode_str(path).decode_utf8_lossy();
    !decoded.contains('\0')
        && !decoded.contains('\\')
        && !decoded.split('/').any(|segment| segment == "..")
}

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
pub trait RateLimiter: Send + Sync {
    fn allow(&self, key: &str) -> bool;
}

pub struct InMemoryRateLimiter {
    rps: u64,
    burst: u64,
    state: Mutex<HashMap<String, (u64, Instant)>>,
}

impl InMemoryRateLimiter {
    pub fn new(rps: u64, burst: u64) -> Self {
        Self {
            rps,
            burst,
            state: Mutex::new(HashMap::new()),
        }
    }
}

impl RateLimiter for InMemoryRateLimiter {
    fn allow(&self, key: &str) -> bool {
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

/// Stub de Redis rate limiter. En V1 no conecta a Redis real; simula
/// allow-all para que el server arranque sin infraestructura externa.
pub struct RedisRateLimiter;

impl Default for RedisRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RedisRateLimiter {
    pub fn new() -> Self {
        Self
    }
}

impl RateLimiter for RedisRateLimiter {
    fn allow(&self, _key: &str) -> bool {
        tracing::warn!("RedisRateLimiter es un stub: allow-all");
        true
    }
}

/// Helper: construir el RateLimiter a partir de variables de entorno.
/// `SF_RATELIMITER_BACKEND=redis|memory` (default: memory).
pub fn build_rate_limiter() -> Arc<dyn RateLimiter> {
    let backend = std::env::var("SF_RATELIMITER_BACKEND").unwrap_or_else(|_| "memory".to_string());
    match backend.as_str() {
        "redis" => {
            tracing::info!("RateLimiter: usando backend Redis (stub)");
            Arc::new(RedisRateLimiter::new())
        }
        _ => {
            let rps = std::env::var("SF_RATE_LIMIT_RPS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(60);
            let burst = std::env::var("SF_RATE_LIMIT_BURST")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(120);
            Arc::new(InMemoryRateLimiter::new(rps, burst))
        }
    }
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
        .app_data::<Data<Arc<dyn RateLimiter>>>()
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

/// Closure para `from_fn`: Content-Type application/json en solicitudes con cuerpo.
pub async fn content_type_middleware(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<BoxBody>, actix_web::Error> {
    if req.path().starts_with("/api/")
        && has_request_body(req.method())
        && !is_json_content_type(&req)
    {
        let resp = rejection(
            actix_web::http::StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "INVALID_CONTENT_TYPE",
            "Content-Type debe ser application/json".to_string(),
            &req,
        );
        return Ok(req.into_response(resp));
    }
    Ok(next.call(req).await?.map_into_boxed_body())
}

/// Closure para `from_fn`: valida segmentos de ruta codificados o con traversal.
pub async fn path_validation_middleware(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<BoxBody>, actix_web::Error> {
    if req.path().starts_with("/api/") && !request_target_is_safe(req.path()) {
        let resp = rejection(
            actix_web::http::StatusCode::BAD_REQUEST,
            "INVALID_PATH",
            "La ruta de la solicitud contiene traversal".to_string(),
            &req,
        );
        return Ok(req.into_response(resp));
    }
    Ok(next.call(req).await?.map_into_boxed_body())
}

/// Closure para `from_fn`: limita el cuerpo, valida JSON y restaura el payload.
pub async fn body_limit_middleware(
    mut req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<BoxBody>, actix_web::Error> {
    if !req.path().starts_with("/api/") || !has_request_body(req.method()) {
        return Ok(next.call(req).await?.map_into_boxed_body());
    }

    let max_upload_bytes = req
        .app_data::<Data<usize>>()
        .map(|value| *value.get_ref())
        .unwrap_or(DEFAULT_MAX_UPLOAD_BYTES);
    let max_body_bytes = body_limit_for(max_upload_bytes);

    if let Some(content_length) = req.headers().get(CONTENT_LENGTH) {
        let content_length = content_length
            .to_str()
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .ok_or_else(|| actix_web::error::ErrorBadRequest("Content-Length inválido"))?;
        if content_length > max_body_bytes {
            let resp = rejection(
                actix_web::http::StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                format!("El cuerpo excede el límite de {max_body_bytes} bytes"),
                &req,
            );
            return Ok(req.into_response(resp));
        }
    }

    let mut payload = req.take_payload();
    let mut body = Vec::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk.map_err(|error| actix_web::error::ErrorBadRequest(error.to_string()))?;
        if body.len().saturating_add(chunk.len()) > max_body_bytes {
            let resp = rejection(
                actix_web::http::StatusCode::PAYLOAD_TOO_LARGE,
                "PAYLOAD_TOO_LARGE",
                format!("El cuerpo excede el límite de {max_body_bytes} bytes"),
                &req,
            );
            return Ok(req.into_response(resp));
        }
        body.extend_from_slice(&chunk);
    }

    let json: Value = serde_json::from_slice(&body)
        .map_err(|error| actix_web::error::ErrorBadRequest(error.to_string()))?;
    if let Err(violation) = validate_payload_structure(req.path(), &json, max_upload_bytes) {
        let status = if violation.code == "PAYLOAD_TOO_LARGE" {
            actix_web::http::StatusCode::PAYLOAD_TOO_LARGE
        } else {
            actix_web::http::StatusCode::BAD_REQUEST
        };
        let resp = rejection(status, violation.code, violation.message, &req);
        return Ok(req.into_response(resp));
    }

    req.set_payload(Bytes::from(body).into());
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
        res.headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    }
    Ok(res.map_into_boxed_body())
}
