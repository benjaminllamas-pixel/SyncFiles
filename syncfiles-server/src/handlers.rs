use actix_web::{web, HttpRequest, HttpResponse, Result as ActixResult};
use std::sync::Arc;
use tracing::{info, warn, error};

use crate::state::AppState;
use crate::models::{ApiError, ApiResponse, ChangeEntry, DiffResponse, DownloadResponse, FileEntry, LoginRequest, LoginResponse, DiffRequest, UploadRequest, DeleteRequest, ResolveConflictRequest, uuid_str as model_uuid_str, now_ms};
use crate::auth;

fn bad_request(code: &str, message: String) -> HttpResponse {
    HttpResponse::BadRequest().json(ApiError {
        code: code.to_string(),
        message,
        retryable: false,
        request_id: model_uuid_str(),
    })
}

fn unauthorized(message: String) -> HttpResponse {
    HttpResponse::Unauthorized().json(ApiError {
        code: "UNAUTHORIZED".to_string(),
        message,
        retryable: false,
        request_id: model_uuid_str(),
    })
}

fn ok_response<T: serde::Serialize>(data: Option<T>) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse {
        accepted: true,
        status: "ok".to_string(),
        server_seq: None,
        data,
        error: None,
    })
}

pub async fn login_handler(
    state: web::Data<Arc<AppState>>,
    req: web::Json<LoginRequest>,
) -> ActixResult<HttpResponse> {
    info!("Login request for: {}", req.email);
    match auth::login(&state, &req).await {
        Ok(resp) => Ok(HttpResponse::Ok().json(resp)),
        Err(e) => {
            warn!("Login failed: {}", e);
            Ok(bad_request("AUTH_FAILED", e.to_string()))
        }
    }
}

pub async fn logout_handler(
    state: web::Data<Arc<AppState>>,
    req: HttpRequest,
) -> ActixResult<HttpResponse> {
    if let Some(token) = extract_session(&req) {
        let _ = auth::logout(&state, &token).await;
    }
    Ok(ok_response::<()>(None))
}

pub async fn session_status_handler(
    state: web::Data<Arc<AppState>>,
    req: HttpRequest,
) -> ActixResult<HttpResponse> {
    let token = match extract_session(&req) {
        Some(t) => t,
        None => return Ok(HttpResponse::Unauthorized().finish()),
    };
    match auth::validate_session(&state, &token).await {
        Ok(session) => Ok(HttpResponse::Ok().json(session)),
        Err(_) => Ok(HttpResponse::Unauthorized().finish()),
    }
}

pub async fn diff_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    req: web::Json<DiffRequest>,
) -> ActixResult<HttpResponse> {
    let token = match extract_session(&request).or_else(|| Some(req.session_id.clone())) {
        Some(t) => t,
        None => return Ok(HttpResponse::Unauthorized().finish()),
    };
    let session = match auth::validate_session(&state, &token).await {
        Ok(s) => s,
        Err(e) => return Ok(unauthorized(e.to_string())),
    };

    let user_id = &session.user_id;
    let since = req.since;
    let changes = crate::db::get_files_modified_since(&state.pool, user_id, since).await.unwrap_or_default();

    let change_entries: Vec<ChangeEntry> = changes.iter().map(|f| ChangeEntry {
        file_id: f.file_id.clone(),
        operation: "upload".to_string(),
        path_hash: f.path_hash.clone(),
        checksum: f.checksum.clone(),
        modified_at: f.modified_at,
        device_id: f.device_id.clone(),
        relative_path: Some(f.relative_path.clone()),
        content: None,
        size_bytes: Some(f.size_bytes),
    }).collect();

    let server_seq = state.next_server_seq();
    let _ = crate::db::set_last_server_seq(&state.pool, server_seq).await;

    info!("Diff: {} changes for user {}", change_entries.len(), user_id);

    Ok(HttpResponse::Ok().json(DiffResponse {
        changes: change_entries,
        server_seq,
    }))
}

pub async fn upload_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    req: web::Json<UploadRequest>,
) -> ActixResult<HttpResponse> {
    let token = match extract_session(&request).or_else(|| Some(req.session_id.clone())) {
        Some(t) => t,
        None => return Ok(unauthorized("Sin sesión".to_string())),
    };
    let session = match auth::validate_session(&state, &token).await {
        Ok(s) => s,
        Err(e) => return Ok(unauthorized(e.to_string())),
    };

    let file_id = if req.file_id.is_empty() {
        model_uuid_str()
    } else {
        req.file_id.clone()
    };

    let file_entry = FileEntry {
        file_id,
        user_id: session.user_id,
        device_id: req.device_id.clone(),
        relative_path: req.relative_path.clone(),
        path_hash: req.path_hash.clone(),
        checksum: req.checksum.clone(),
        size_bytes: req.size_bytes,
        modified_at: req.modified_at,
        synced_at: Some(now_ms()),
        status: "synced".to_string(),
        last_sync_version: 0,
        deleted_at: None,
        content: req.content.clone(),
    };

    if let Err(e) = crate::db::upsert_file(&state.pool, &file_entry).await {
        return Ok(bad_request("DB_ERROR", e.to_string()));
    }

    let server_seq = state.next_server_seq();
    info!("Upload aceptado: {} ({} bytes)", req.relative_path, req.size_bytes);

    Ok(ok_response::<()>(None))
}

pub async fn download_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    req: web::Json<DeleteRequest>,
) -> ActixResult<HttpResponse> {
    let token = match extract_session(&request).or_else(|| Some(req.session_id.clone())) {
        Some(t) => t,
        None => return Ok(unauthorized("Sin sesión".to_string())),
    };
    let session = match auth::validate_session(&state, &token).await {
        Ok(s) => s,
        Err(e) => return Ok(unauthorized(e.to_string())),
    };

    let file = match crate::db::get_file_by_id_for_user(&state.pool, &session.user_id, &req.file_id).await {
        Ok(Some(f)) => f,
        Ok(None) => return Ok(bad_request("NOT_FOUND", "Archivo no encontrado".to_string())),
        Err(e) => return Ok(bad_request("DB_ERROR", e.to_string())),
    };

    Ok(HttpResponse::Ok().json(DownloadResponse {
        accepted: true,
        status: "ready".to_string(),
        checksum: file.checksum,
        content: file.content,
        file_id: file.file_id,
    }))
}

pub async fn delete_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    req: web::Json<DeleteRequest>,
) -> ActixResult<HttpResponse> {
    let token = match extract_session(&request).or_else(|| Some(req.session_id.clone())) {
        Some(t) => t,
        None => return Ok(unauthorized("Sin sesión".to_string())),
    };
    let _session = match auth::validate_session(&state, &token).await {
        Ok(s) => s,
        Err(e) => return Ok(unauthorized(e.to_string())),
    };

    if let Err(e) = crate::db::delete_file(&state.pool, &req.file_id).await {
        return Ok(bad_request("DB_ERROR", e.to_string()));
    }

    Ok(ok_response::<()>(None))
}

pub async fn rename_handler(
    state: web::Data<Arc<AppState>>,
    req: HttpRequest,
) -> ActixResult<HttpResponse> {
    let token = match extract_session(&req) {
        Some(t) => t,
        None => return Ok(unauthorized("Sin sesión".to_string())),
    };
    if let Err(e) = auth::validate_session(&state, &token).await {
        return Ok(unauthorized(e.to_string()));
    }
    Ok(ok_response::<()>(None))
}

pub async fn move_handler(
    state: web::Data<Arc<AppState>>,
    req: HttpRequest,
) -> ActixResult<HttpResponse> {
    let token = match extract_session(&req) {
        Some(t) => t,
        None => return Ok(unauthorized("Sin sesión".to_string())),
    };
    if let Err(e) = auth::validate_session(&state, &token).await {
        return Ok(unauthorized(e.to_string()));
    }
    Ok(ok_response::<()>(None))
}

pub async fn copy_handler(
    state: web::Data<Arc<AppState>>,
    req: HttpRequest,
) -> ActixResult<HttpResponse> {
    let token = match extract_session(&req) {
        Some(t) => t,
        None => return Ok(unauthorized("Sin sesión".to_string())),
    };
    if let Err(e) = auth::validate_session(&state, &token).await {
        return Ok(unauthorized(e.to_string()));
    }
    Ok(ok_response::<()>(None))
}

pub async fn resolve_conflict_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    req: web::Json<ResolveConflictRequest>,
) -> ActixResult<HttpResponse> {
    let token = match extract_session(&request).or_else(|| Some(req.session_id.clone())) {
        Some(t) => t,
        None => return Ok(unauthorized("Sin sesión".to_string())),
    };
    let session = match auth::validate_session(&state, &token).await {
        Ok(s) => s,
        Err(e) => return Ok(unauthorized(e.to_string())),
    };

    if let Err(e) = crate::db::resolve_conflict(&state.pool, &req.conflict_id, &session.user_id).await {
        return Ok(bad_request("DB_ERROR", e.to_string()));
    }

    Ok(ok_response::<()>(None))
}

pub async fn not_found() -> ActixResult<HttpResponse> {
    Ok(HttpResponse::NotFound().json(ApiError {
        code: "NOT_FOUND".to_string(),
        message: "Endpoint no encontrado".to_string(),
        retryable: false,
        request_id: model_uuid_str(),
    }))
}

fn extract_session(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}
