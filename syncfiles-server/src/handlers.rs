use actix_web::{web, HttpRequest, HttpResponse, Result as ActixResult};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tracing::{info, warn};

use crate::state::AppState;
use crate::models::{ApiError, ApiResponse, ChangeEntry, DiffResponse, DownloadResponse, FileEntry, LoginRequest, DiffRequest, UploadRequest, DeleteRequest, DownloadRequest, RenameRequest, MoveRequest, CopyRequest, ResolveConflictRequest, uuid_str as model_uuid_str, now_ms};
use crate::auth;
use crate::storage::normalize_relative_path;

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

fn ok_response<T: serde::Serialize>(data: Option<T>, server_seq: i64) -> HttpResponse {
    HttpResponse::Ok().json(ApiResponse {
        accepted: true,
        status: "ok".to_string(),
        server_seq: Some(server_seq),
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
    Ok(ok_response::<()>(None, 0))
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

    let normalized_relative_path = match normalize_relative_path(&req.relative_path) {
        Ok(p) => p,
        Err(_) => return Ok(bad_request("INVALID_PATH", "Ruta relativa inválida".to_string())),
    };
    let expected_path_hash = format!("{:x}", Sha256::digest(normalized_relative_path.as_bytes()));
    if req.path_hash != expected_path_hash {
        return Ok(bad_request("INVALID_PATH_HASH", "path_hash no coincide con relative_path".to_string()));
    }

    let file_id = if req.file_id.is_empty() {
        model_uuid_str()
    } else {
        req.file_id.clone()
    };

    let content_bytes = match BASE64.decode(&req.content) {
        Ok(b) => b,
        Err(_) => return Ok(bad_request("INVALID_CONTENT", "Content base64 inválido".to_string())),
    };

    if let Err(e) = state.storage.write(&session.user_id, &req.relative_path, &content_bytes).await {
        return Ok(bad_request("STORAGE_ERROR", e.to_string()));
    }

    let file_entry = FileEntry {
        file_id: file_id.clone(),
        user_id: session.user_id.clone(),
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
        content: None,
    };

    if let Err(e) = crate::db::upsert_file(&state.pool, &file_entry).await {
        return Ok(bad_request("DB_ERROR", e.to_string()));
    }

    let server_seq = state.next_server_seq();
    info!("Upload aceptado: {} ({} bytes)", req.relative_path, req.size_bytes);

    Ok(ok_response::<()>(None, server_seq))
}

pub async fn download_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    req: web::Json<DownloadRequest>,
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

    let content_bytes = match state.storage.read(&session.user_id, &file.relative_path).await {
        Ok(b) => b,
        Err(crate::storage::StorageError::NotFound) => return Ok(bad_request("NOT_FOUND", "Contenido no encontrado en storage".to_string())),
        Err(e) => return Ok(bad_request("STORAGE_ERROR", e.to_string())),
    };

    let content_b64 = BASE64.encode(content_bytes);
    let server_seq = state.next_server_seq();

    Ok(HttpResponse::Ok().json(DownloadResponse {
        accepted: true,
        status: "ready".to_string(),
        checksum: file.checksum,
        content: content_b64,
        file_id: file.file_id,
        server_seq,
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
    let session = match auth::validate_session(&state, &token).await {
        Ok(s) => s,
        Err(e) => return Ok(unauthorized(e.to_string())),
    };

    let file = match crate::db::get_file_by_id_for_user(&state.pool, &session.user_id, &req.file_id).await {
        Ok(Some(f)) => f,
        Ok(None) => return Ok(bad_request("NOT_FOUND", "Archivo no encontrado".to_string())),
        Err(e) => return Ok(bad_request("DB_ERROR", e.to_string())),
    };

    let _ = state.storage.delete(&session.user_id, &file.relative_path).await;

    if let Err(e) = crate::db::delete_file(&state.pool, &req.file_id).await {
        return Ok(bad_request("DB_ERROR", e.to_string()));
    }

    let server_seq = state.next_server_seq();
    Ok(ok_response::<()>(None, server_seq))
}

pub async fn rename_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    req: web::Json<RenameRequest>,
) -> ActixResult<HttpResponse> {
    let token = match extract_session(&request).or_else(|| Some(req.session_id.clone())) {
        Some(t) => t,
        None => return Ok(unauthorized("Sin sesión".to_string())),
    };
    let session = match auth::validate_session(&state, &token).await {
        Ok(s) => s,
        Err(e) => return Ok(unauthorized(e.to_string())),
    };

    let new_normalized = match normalize_relative_path(&req.new_path) {
        Ok(p) => p,
        Err(_) => return Ok(bad_request("INVALID_PATH", "Ruta relativa inválida".to_string())),
    };

    if let Err(e) = state.storage.rename(&session.user_id, &req.old_path, &new_normalized).await {
        return Ok(bad_request("STORAGE_ERROR", e.to_string()));
    }

    if let Err(e) = crate::db::rename_file(&state.pool, &req.file_id, &new_normalized).await {
        return Ok(bad_request("DB_ERROR", e.to_string()));
    }

    let server_seq = state.next_server_seq();
    Ok(ok_response::<()>(None, server_seq))
}

pub async fn move_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    req: web::Json<MoveRequest>,
) -> ActixResult<HttpResponse> {
    let token = match extract_session(&request).or_else(|| Some(req.session_id.clone())) {
        Some(t) => t,
        None => return Ok(unauthorized("Sin sesión".to_string())),
    };
    let session = match auth::validate_session(&state, &token).await {
        Ok(s) => s,
        Err(e) => return Ok(unauthorized(e.to_string())),
    };

    if let Err(e) = state.storage.rename(&session.user_id, &req.old_path, &req.new_path).await {
        return Ok(bad_request("STORAGE_ERROR", e.to_string()));
    }

    if let Err(e) = crate::db::rename_file(&state.pool, &req.file_id, &req.new_path).await {
        return Ok(bad_request("DB_ERROR", e.to_string()));
    }

    let server_seq = state.next_server_seq();
    Ok(ok_response::<()>(None, server_seq))
}

pub async fn copy_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    req: web::Json<CopyRequest>,
) -> ActixResult<HttpResponse> {
    let token = match extract_session(&request).or_else(|| Some(req.session_id.clone())) {
        Some(t) => t,
        None => return Ok(unauthorized("Sin sesión".to_string())),
    };
    let session = match auth::validate_session(&state, &token).await {
        Ok(s) => s,
        Err(e) => return Ok(unauthorized(e.to_string())),
    };

    if let Err(e) = state.storage.copy(&session.user_id, &req.source_path, &req.destination_path).await {
        return Ok(bad_request("STORAGE_ERROR", e.to_string()));
    }

    let content_bytes = match state.storage.read(&session.user_id, &req.destination_path).await {
        Ok(b) => b,
        Err(e) => return Ok(bad_request("STORAGE_ERROR", e.to_string())),
    };

    let checksum = format!("{:x}", Sha256::digest(&content_bytes));
    let size_bytes = content_bytes.len() as i64;
    let now = now_ms();

    let new_file_id = model_uuid_str();
    let new_entry = FileEntry {
        file_id: new_file_id.clone(),
        user_id: session.user_id.clone(),
        device_id: req.device_id.clone(),
        relative_path: req.destination_path.clone(),
        path_hash: crate::models::uuid_str(),
        checksum,
        size_bytes,
        modified_at: now,
        synced_at: Some(now),
        status: "synced".to_string(),
        last_sync_version: 0,
        deleted_at: None,
        content: None,
    };

    if let Err(e) = crate::db::upsert_file(&state.pool, &new_entry).await {
        return Ok(bad_request("DB_ERROR", e.to_string()));
    }

    let server_seq = state.next_server_seq();
    Ok(ok_response::<()>(None, server_seq))
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

    let server_seq = state.next_server_seq();
    Ok(ok_response::<()>(None, server_seq))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::normalize_relative_path;

    #[test]
    fn normalize_relative_path_removes_dot_segments() {
        assert_eq!(normalize_relative_path("a/b/../c/f.txt").unwrap(), "a/c/f.txt");
        assert_eq!(normalize_relative_path("./a/./b/").unwrap(), "a/b");
        assert_eq!(normalize_relative_path("a//b").unwrap(), "a/b");
    }

    #[test]
    fn normalize_relative_path_rejects_parent_escape() {
        assert!(normalize_relative_path("../etc/passwd").is_err());
        assert!(normalize_relative_path("a/../../b").is_err());
    }

    #[test]
    fn path_hash_must_match_relative_path() {
        let path = "docs/readme.txt";
        let normalized = normalize_relative_path(path).unwrap();
        let expected_hash = format!("{:x}", Sha256::digest(normalized.as_bytes()));

        assert_eq!(expected_hash, format!("{:x}", Sha256::digest(path.as_bytes())));

        let wrong_hash = format!("{:x}", Sha256::digest(b"other.txt"));
        assert_ne!(expected_hash, wrong_hash);
    }
}
