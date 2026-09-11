use actix_web::{web, HttpRequest, HttpResponse, Result as ActixResult};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tracing::{info, warn};

use crate::state::AppState;
use crate::models::{ApiError, ApiResponse, ChangeEntry, DiffResponse, DownloadResponse, FileEntry, LoginRequest, Session, DiffRequest, UploadRequest, DeleteRequest, DownloadRequest, RenameRequest, MoveRequest, CopyRequest, ResolveConflictRequest, RevokeDeviceRequest, Conflict, uuid_str, now_ms, compute_path_hash};
use crate::auth;
use crate::storage::normalize_relative_path;

fn bad_request(code: &str, message: String) -> HttpResponse {
    HttpResponse::BadRequest().json(ApiError {
        code: code.to_string(),
        message,
        retryable: false,
        request_id: uuid_str(),
    })
}

fn unauthorized(message: String) -> HttpResponse {
    HttpResponse::Unauthorized().json(ApiError {
        code: "UNAUTHORIZED".to_string(),
        message,
        retryable: false,
        request_id: uuid_str(),
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

async fn authorize_session(
    state: &AppState,
    request: &HttpRequest,
    req_session_id: &str,
) -> std::result::Result<Session, HttpResponse> {
    let token = match extract_session(request) {
        Some(t) => t,
        None => return Err(unauthorized("Sin sesión".to_string())),
    };
    let session = auth::validate_session(state, &token)
        .await
        .map_err(|e| unauthorized(e.to_string()))?;
    if !req_session_id.is_empty() && req_session_id != token {
        return Err(unauthorized("session_id no coincide con el token de autorización".to_string()));
    }
    Ok(session)
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
    let session = match authorize_session(&state, &request, &req.session_id).await {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
    };

    let user_id = &session.user_id;
    let since = req.since;
    let changes = crate::db::get_files_modified_since(&state.pool, user_id, since).await.unwrap_or_default();

    let change_entries: Vec<ChangeEntry> = changes.iter().map(|f| {
        let operation = if f.status == "deleted" { "delete".to_string() } else { "upload".to_string() };
        ChangeEntry {
            file_id: f.file_id.clone(),
            operation,
            path_hash: f.path_hash.clone(),
            checksum: f.checksum.clone(),
            modified_at: if f.deleted_at.is_some() { f.deleted_at.unwrap_or(f.modified_at) } else { f.modified_at },
            device_id: f.device_id.clone(),
            relative_path: Some(f.relative_path.clone()),
            content: None,
            size_bytes: Some(f.size_bytes),
        }
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
    let session = match authorize_session(&state, &request, &req.session_id).await {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
    };

    if crate::db::check_idempotency(&state.pool, &req.idempotency_key).await.unwrap_or(false) {
        let server_seq = state.next_server_seq();
        info!("Upload idempotente repetido: {}", req.relative_path);
        return Ok(ok_response::<()>(None, server_seq));
    }

    let normalized_relative_path = match normalize_relative_path(&req.relative_path) {
        Ok(p) => p,
        Err(_) => return Ok(bad_request("INVALID_PATH", "Ruta relativa inválida".to_string())),
    };
    let expected_path_hash = compute_path_hash(&normalized_relative_path);
    if req.path_hash != expected_path_hash {
        return Ok(bad_request("INVALID_PATH_HASH", "path_hash no coincide con relative_path".to_string()));
    }

    let existing: Option<FileEntry> = match crate::db::get_files_by_path_hash(&state.pool, &session.user_id, &req.path_hash).await {
        Ok(Some(f)) => Some(f),
        Ok(None) => None,
        Err(e) => return Ok(bad_request("DB_ERROR", e.to_string())),
    };

    if let Some(ref existing_file) = existing {
        if existing_file.checksum != req.checksum && existing_file.status != "deleted" {
            let conflict_id = uuid_str();
            let preserved_path = format!(
                "{}.conflict_{}_{}",
                existing_file.relative_path, conflict_id, now_ms()
            );
            let _ = state.storage.copy(&session.user_id, &existing_file.relative_path, &preserved_path).await;

            let conflict = Conflict {
                conflict_id: conflict_id.clone(),
                file_id: existing_file.file_id.clone(),
                user_id: session.user_id.clone(),
                device_local: Some(existing_file.device_id.clone()),
                device_remote: Some(req.device_id.clone()),
                local_checksum: Some(existing_file.checksum.clone()),
                remote_checksum: Some(req.checksum.clone()),
                conflict_type: "checksum_mismatch".to_string(),
                strategy: "last_write_wins".to_string(),
                created_at: now_ms(),
                resolved_at: None,
                resolved_by: None,
            };
            if let Err(e) = crate::db::mark_conflict(&state.pool, &conflict).await {
                warn!("No se pudo registrar conflicto: {}", e);
            }

            return Ok(HttpResponse::Conflict().json(ApiResponse {
                accepted: false,
                status: "conflict".to_string(),
                server_seq: Some(state.next_server_seq()),
                data: Some(serde_json::json!({
                    "conflict_id": conflict_id,
                    "alternative_preserved_path": preserved_path,
                })),
                error: Some(ApiError {
                    code: "CONFLICT".to_string(),
                    message: "Conflicto de checksum detectado".to_string(),
                    retryable: false,
                    request_id: uuid_str(),
                }),
            }));
        }
    }

    let file_id = if req.file_id.is_empty() {
        uuid_str()
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

    if let Err(e) = crate::db::record_idempotency(&state.pool, &req.idempotency_key, &session.session_id).await {
        warn!("No se pudo registrar idempotency_key: {}", e);
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
    let session = match authorize_session(&state, &request, &req.session_id).await {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
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
    let session = match authorize_session(&state, &request, &req.session_id).await {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
    };

    if crate::db::check_idempotency(&state.pool, &req.idempotency_key).await.unwrap_or(false) {
        let server_seq = state.next_server_seq();
        info!("Delete idempotente repetido: {}", req.file_id);
        return Ok(ok_response::<()>(None, server_seq));
    }

    let file = match crate::db::get_file_by_id_for_user(&state.pool, &session.user_id, &req.file_id).await {
        Ok(Some(f)) => f,
        Ok(None) => return Ok(bad_request("NOT_FOUND", "Archivo no encontrado".to_string())),
        Err(e) => return Ok(bad_request("DB_ERROR", e.to_string())),
    };

    let _ = state.storage.delete(&session.user_id, &file.relative_path).await;

    if let Err(e) = crate::db::delete_file(&state.pool, &req.file_id).await {
        return Ok(bad_request("DB_ERROR", e.to_string()));
    }

    if let Err(e) = crate::db::record_idempotency(&state.pool, &req.idempotency_key, &session.session_id).await {
        warn!("No se pudo registrar idempotency_key: {}", e);
    }

    let server_seq = state.next_server_seq();
    Ok(ok_response::<()>(None, server_seq))
}

pub async fn rename_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    req: web::Json<RenameRequest>,
) -> ActixResult<HttpResponse> {
    let session = match authorize_session(&state, &request, &req.session_id).await {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
    };

    if crate::db::check_idempotency(&state.pool, &req.idempotency_key).await.unwrap_or(false) {
        let server_seq = state.next_server_seq();
        info!("Rename idempotente repetido: {}", req.file_id);
        return Ok(ok_response::<()>(None, server_seq));
    }

    let new_normalized = match normalize_relative_path(&req.new_path) {
        Ok(p) => p,
        Err(_) => return Ok(bad_request("INVALID_PATH", "Ruta relativa inválida".to_string())),
    };

    let file = match crate::db::get_file_by_id_for_user(&state.pool, &session.user_id, &req.file_id).await {
        Ok(Some(f)) => f,
        Ok(None) => return Ok(bad_request("NOT_FOUND", "Archivo no encontrado".to_string())),
        Err(e) => return Ok(bad_request("DB_ERROR", e.to_string())),
    };
    let old_path = if req.old_path.is_empty() { file.relative_path.clone() } else { req.old_path.clone() };

    if let Err(e) = state.storage.rename(&session.user_id, &old_path, &new_normalized).await {
        return Ok(bad_request("STORAGE_ERROR", e.to_string()));
    }

    if let Err(e) = crate::db::rename_file(&state.pool, &req.file_id, &new_normalized).await {
        return Ok(bad_request("DB_ERROR", e.to_string()));
    }

    if let Err(e) = crate::db::record_idempotency(&state.pool, &req.idempotency_key, &session.session_id).await {
        warn!("No se pudo registrar idempotency_key: {}", e);
    }

    let server_seq = state.next_server_seq();
    Ok(ok_response::<()>(None, server_seq))
}

pub async fn move_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    req: web::Json<MoveRequest>,
) -> ActixResult<HttpResponse> {
    let session = match authorize_session(&state, &request, &req.session_id).await {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
    };

    if crate::db::check_idempotency(&state.pool, &req.idempotency_key).await.unwrap_or(false) {
        let server_seq = state.next_server_seq();
        info!("Move idempotente repetido: {}", req.file_id);
        return Ok(ok_response::<()>(None, server_seq));
    }

    let new_normalized = match normalize_relative_path(&req.new_path) {
        Ok(p) => p,
        Err(_) => return Ok(bad_request("INVALID_PATH", "Ruta relativa inválida".to_string())),
    };

    let file = match crate::db::get_file_by_id_for_user(&state.pool, &session.user_id, &req.file_id).await {
        Ok(Some(f)) => f,
        Ok(None) => return Ok(bad_request("NOT_FOUND", "Archivo no encontrado".to_string())),
        Err(e) => return Ok(bad_request("DB_ERROR", e.to_string())),
    };
    let old_path = if req.old_path.is_empty() { file.relative_path.clone() } else { req.old_path.clone() };

    if let Err(e) = state.storage.rename(&session.user_id, &old_path, &new_normalized).await {
        return Ok(bad_request("STORAGE_ERROR", e.to_string()));
    }

    if let Err(e) = crate::db::rename_file(&state.pool, &req.file_id, &new_normalized).await {
        return Ok(bad_request("DB_ERROR", e.to_string()));
    }

    if let Err(e) = crate::db::record_idempotency(&state.pool, &req.idempotency_key, &session.session_id).await {
        warn!("No se pudo registrar idempotency_key: {}", e);
    }

    let server_seq = state.next_server_seq();
    Ok(ok_response::<()>(None, server_seq))
}

pub async fn copy_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    req: web::Json<CopyRequest>,
) -> ActixResult<HttpResponse> {
    let session = match authorize_session(&state, &request, &req.session_id).await {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
    };

    if crate::db::check_idempotency(&state.pool, &req.idempotency_key).await.unwrap_or(false) {
        let server_seq = state.next_server_seq();
        info!("Copy idempotente repetido: {}", req.file_id);
        return Ok(ok_response::<()>(None, server_seq));
    }

    let src_normalized = match normalize_relative_path(&req.source_path) {
        Ok(p) => p,
        Err(_) => return Ok(bad_request("INVALID_PATH", "Ruta de origen inválida".to_string())),
    };
    let dst_normalized = match normalize_relative_path(&req.destination_path) {
        Ok(p) => p,
        Err(_) => return Ok(bad_request("INVALID_PATH", "Ruta de destino inválida".to_string())),
    };

    if let Err(e) = state.storage.copy(&session.user_id, &src_normalized, &dst_normalized).await {
        return Ok(bad_request("STORAGE_ERROR", e.to_string()));
    }

    let content_bytes = match state.storage.read(&session.user_id, &dst_normalized).await {
        Ok(b) => b,
        Err(crate::storage::StorageError::NotFound) => return Ok(bad_request("NOT_FOUND", "Contenido no encontrado en storage".to_string())),
        Err(e) => return Ok(bad_request("STORAGE_ERROR", e.to_string())),
    };

    let checksum = format!("{:x}", Sha256::digest(&content_bytes));
    let size_bytes = content_bytes.len() as i64;
    let now = now_ms();

    let new_file_id = uuid_str();
    let new_entry = FileEntry {
        file_id: new_file_id.clone(),
        user_id: session.user_id.clone(),
        device_id: req.device_id.clone(),
        relative_path: dst_normalized.clone(),
        path_hash: compute_path_hash(&dst_normalized),
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

    if let Err(e) = crate::db::record_idempotency(&state.pool, &req.idempotency_key, &session.session_id).await {
        warn!("No se pudo registrar idempotency_key: {}", e);
    }

    let server_seq = state.next_server_seq();
    Ok(ok_response::<()>(None, server_seq))
}

pub async fn files_list_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
) -> ActixResult<HttpResponse> {
    let session = match authorize_session(&state, &request, "").await {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
    };

    let include_deleted = request
        .query_string()
        .split('&')
        .any(|pair| pair == "include_deleted=true" || pair == "include_deleted=1");

    let files = match crate::db::list_files_for_user(&state.pool, &session.user_id, include_deleted).await {
        Ok(f) => f,
        Err(e) => return Ok(bad_request("DB_ERROR", e.to_string())),
    };
    let total = files.len() as i64;
    let items: Vec<syncfiles_models::FileListItem> = files.into_iter().map(|f| syncfiles_models::FileListItem {
        file_id: f.file_id,
        relative_path: f.relative_path,
        path_hash: f.path_hash,
        checksum: f.checksum,
        size_bytes: f.size_bytes,
        modified_at: f.modified_at,
        synced_at: f.synced_at,
        status: f.status,
        deleted_at: f.deleted_at,
        device_id: f.device_id,
    }).collect();

    let server_seq = state.next_server_seq();
    info!("Files list: {} archivos para {}", total, session.user_id);
    Ok(HttpResponse::Ok().json(syncfiles_models::FilesListResponse {
        files: items,
        total,
        server_seq,
    }))
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct QueueParams {
    pub device_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
}

pub async fn queue_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    params: web::Query<QueueParams>,
) -> ActixResult<HttpResponse> {
    let session = match authorize_session(&state, &request, "").await {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
    };

    let device_filter = params.device_id.clone();
    let pending_only = params
        .status
        .as_deref()
        .map(|s| s == "pending" || s == "queued" || s == "retry")
        .unwrap_or(false);
    let limit = params.limit.unwrap_or(50).clamp(1, 200);

    let entries = if pending_only {
        crate::db::get_pending_queued_ops_for_user(&state.pool, &session.user_id, device_filter.as_deref(), limit).await
    } else {
        crate::db::get_queued_ops_for_user(&state.pool, &session.user_id, device_filter.as_deref(), limit).await
    };
    let entries = match entries {
        Ok(e) => e,
        Err(err) => return Ok(bad_request("DB_ERROR", err.to_string())),
    };

    let total = match crate::db::get_queue_total_for_user(&state.pool, &session.user_id).await {
        Ok(t) => t,
        Err(err) => return Ok(bad_request("DB_ERROR", err.to_string())),
    };

    let server_seq = state.next_server_seq();
    Ok(HttpResponse::Ok().json(syncfiles_models::QueueResponse {
        entries,
        total,
        server_seq,
    }))
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct ActivityParams {
    pub limit: Option<i64>,
}

pub async fn activity_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    params: web::Query<ActivityParams>,
) -> ActixResult<HttpResponse> {
    let session = match authorize_session(&state, &request, "").await {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
    };

    let limit = params.limit.unwrap_or(100).clamp(1, 500);

    let events = match crate::db::get_activity_for_user(&state.pool, &session.user_id, limit).await {
        Ok(e) => e,
        Err(err) => return Ok(bad_request("DB_ERROR", err.to_string())),
    };
    let total = match crate::db::get_activity_total_for_user(&state.pool, &session.user_id).await {
        Ok(t) => t,
        Err(err) => return Ok(bad_request("DB_ERROR", err.to_string())),
    };

    let server_seq = state.next_server_seq();
    Ok(HttpResponse::Ok().json(syncfiles_models::ActivityResponse {
        events,
        total,
        server_seq,
    }))
}

pub async fn conflicts_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
) -> ActixResult<HttpResponse> {
    let session = match authorize_session(&state, &request, "").await {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
    };

    let conflicts = match crate::db::get_unresolved_conflicts_for_user(&state.pool, &session.user_id).await {
        Ok(c) => c,
        Err(e) => return Ok(bad_request("DB_ERROR", e.to_string())),
    };
    let total = match crate::db::get_unresolved_conflict_count_for_user(&state.pool, &session.user_id).await {
        Ok(t) => t,
        Err(e) => return Ok(bad_request("DB_ERROR", e.to_string())),
    };

    let server_seq = state.next_server_seq();
    Ok(HttpResponse::Ok().json(syncfiles_models::ConflictsResponse {
        conflicts,
        total,
        server_seq,
    }))
}

pub async fn devices_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
) -> ActixResult<HttpResponse> {
    let session = match authorize_session(&state, &request, "").await {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
    };

    let rows = match crate::db::get_user_devices(&state.pool, &session.user_id).await {
        Ok(r) => r,
        Err(e) => return Ok(bad_request("DB_ERROR", e.to_string())),
    };
    let devices: Vec<syncfiles_models::DeviceWithSessions> = rows.into_iter().map(|(d, active)| syncfiles_models::DeviceWithSessions {
        device_id: d.device_id,
        platform: d.platform,
        device_name: d.device_name,
        last_seen_at: d.last_seen_at,
        created_at: d.created_at,
        active_sessions: active,
    }).collect();
    let total = devices.len() as i64;

    let server_seq = state.next_server_seq();
    Ok(HttpResponse::Ok().json(syncfiles_models::DevicesResponse {
        devices,
        total,
        server_seq,
    }))
}

pub async fn storage_stats_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
) -> ActixResult<HttpResponse> {
    let session = match authorize_session(&state, &request, "").await {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
    };

    let (used_bytes, file_count, last_modified_at) = match crate::db::get_storage_stats(&state.pool, &session.user_id).await {
        Ok(s) => s,
        Err(e) => return Ok(bad_request("DB_ERROR", e.to_string())),
    };

    let server_seq = state.next_server_seq();
    Ok(HttpResponse::Ok().json(syncfiles_models::StorageStats {
        used_bytes,
        file_count,
        last_modified_at,
        server_seq,
    }))
}

pub async fn resolve_conflict_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    req: web::Json<ResolveConflictRequest>,
) -> ActixResult<HttpResponse> {
    let session = match authorize_session(&state, &request, &req.session_id).await {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
    };

    let conflict = match crate::db::get_conflict_by_id(&state.pool, &req.conflict_id).await {
        Ok(Some(c)) => c,
        Ok(None) => return Ok(bad_request("NOT_FOUND", "Conflicto no encontrado".to_string())),
        Err(e) => return Ok(bad_request("DB_ERROR", e.to_string())),
    };

    let file = crate::db::get_file_by_id_for_user(&state.pool, &session.user_id, &conflict.file_id).await
        .ok()
        .flatten();

    let relative_path = file.as_ref().map(|f| f.relative_path.clone()).unwrap_or_default();
    let device_id = req.device_id.clone();

    match req.decision.to_lowercase().as_str() {
        "keep" | "keep_local" | "keep_remote" => {
            if req.preserve_alternative {
                if !relative_path.is_empty() {
                    let preserved = format!(
                        "{}.conflict_{}_{}",
                        relative_path, &req.conflict_id, now_ms()
                    );
                    if let Ok(dst) = normalize_relative_path(&preserved) {
                        if let Err(e) = state.storage.copy(&session.user_id, &relative_path, &dst).await {
                            warn!("No se pudo preservar alternativa: {}", e);
                        }
                    }
                }
            }
        }
        "rename" => {
            if let Some(new_name) = &req.new_name {
                if !relative_path.is_empty() {
                    if let Ok(dst) = normalize_relative_path(new_name) {
                        if let Err(e) = state.storage.rename(&session.user_id, &relative_path, &dst).await {
                            warn!("No se pudo renombrar en storage: {}", e);
                        }
                        if let Some(ref f) = file {
                            let _ = crate::db::rename_file(&state.pool, &f.file_id, &dst).await;
                        }
                    }
                }
            }
        }
        "copy" => {
            if let Some(new_name) = &req.new_name {
                if !relative_path.is_empty() {
                    if let Ok(dst) = normalize_relative_path(new_name) {
                        if let Err(e) = state.storage.copy(&session.user_id, &relative_path, &dst).await {
                            warn!("No se pudo copiar en storage: {}", e);
                        }
                        if let Ok(content_bytes) = state.storage.read(&session.user_id, &dst).await {
                            let checksum = format!("{:x}", Sha256::digest(&content_bytes));
                            let new_file_id = uuid_str();
                            let new_entry = FileEntry {
                                file_id: new_file_id,
                                user_id: session.user_id.clone(),
                                device_id: device_id.clone(),
                                relative_path: dst.clone(),
                                path_hash: compute_path_hash(&dst),
                                checksum,
                                size_bytes: content_bytes.len() as i64,
                                modified_at: now_ms(),
                                synced_at: Some(now_ms()),
                                status: "synced".to_string(),
                                last_sync_version: 0,
                                deleted_at: None,
                                content: None,
                            };
                            let _ = crate::db::upsert_file(&state.pool, &new_entry).await;
                        }
                    }
                }
            }
        }
        other => {
            return Ok(bad_request(
                "INVALID_DECISION",
                format!("Decisión de conflicto no reconocida: {}", other),
            ));
        }
    }

    if let Err(e) = crate::db::resolve_conflict(&state.pool, &req.conflict_id, &conflict.user_id).await {
        return Ok(bad_request("DB_ERROR", e.to_string()));
    }

    if let Err(e) = crate::db::audit_event(&state.pool, &crate::models::AuditEntry {
        audit_id: uuid_str(),
        user_id: Some(session.user_id.clone()),
        device_id: Some(device_id.clone()),
        event_name: "conflict.resolved".to_string(),
        event_type: Some("conflict".to_string()),
        payload_json: Some(format!(
            "{{\"decision\":\"{}\",\"conflict_id\":\"{}\",\"preserve_alternative\":{},\"new_name\":{:?}}}",
            req.decision, req.conflict_id, req.preserve_alternative, req.new_name
        )),
        created_at: now_ms(),
    }).await {
        warn!("No se pudo escribir audit_log: {}", e);
    }

    let server_seq = state.next_server_seq();
    Ok(ok_response::<()>(None, server_seq))
}

pub async fn revoke_device_handler(
    state: web::Data<Arc<AppState>>,
    request: HttpRequest,
    req: web::Json<RevokeDeviceRequest>,
) -> ActixResult<HttpResponse> {
    let session = match authorize_session(&state, &request, &req.session_id).await {
        Ok(s) => s,
        Err(resp) => return Ok(resp),
    };

    if req.target_device_id == req.device_id {
        return Ok(bad_request(
            "INVALID_TARGET",
            "No puedes revocar el dispositivo con el que estás conectado. Usa cerrar sesión.".to_string(),
        ));
    }

    let revoked = match crate::db::revoke_device_sessions(&state.pool, &session.user_id, &req.target_device_id).await {
        Ok(n) => n,
        Err(e) => return Ok(bad_request("DB_ERROR", e.to_string())),
    };
    if revoked == 0 {
        return Ok(bad_request(
            "NOT_FOUND",
            "Dispositivo no encontrado o sin sesiones activas".to_string(),
        ));
    }

    if let Err(e) = crate::db::audit_event(&state.pool, &crate::models::AuditEntry {
        audit_id: uuid_str(),
        user_id: Some(session.user_id.clone()),
        device_id: Some(req.device_id.clone()),
        event_name: "device.revoked".to_string(),
        event_type: Some("device".to_string()),
        payload_json: Some(format!("{{\"target_device_id\":\"{}\",\"sessions_revoked\":{}}}", req.target_device_id, revoked)),
        created_at: now_ms(),
    }).await {
        warn!("No se pudo escribir audit_log: {}", e);
    }

    let server_seq = state.next_server_seq();
    info!("Dispositivo {} revocado ({} sesiones)", req.target_device_id, revoked);
    Ok(ok_response(Some(serde_json::json!({ "sessions_revoked": revoked })), server_seq))
}

pub async fn not_found() -> ActixResult<HttpResponse> {
    Ok(HttpResponse::NotFound().json(ApiError {
        code: "NOT_FOUND".to_string(),
        message: "Endpoint no encontrado".to_string(),
        retryable: false,
        request_id: uuid_str(),
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
        let expected_hash = compute_path_hash(&normalized);

        assert_eq!(expected_hash, compute_path_hash(path));

        let wrong_hash = compute_path_hash("other.txt");
        assert_ne!(expected_hash, wrong_hash);
    }
}