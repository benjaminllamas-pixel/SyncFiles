use actix_http::Request;
use actix_web::body::BoxBody;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::{test, web, App};
use base64::Engine;
use serde_json::Value;
use std::sync::Arc;
use syncfiles_server::handlers;
use syncfiles_server::state::AppState;

const EMAIL: &str = "test@syncfiles.local";
const PASSWORD: &str = "syncfiles";
const USER_ID: &str = "user-test-001";
const DEVICE_ID: &str = "device-test-001";

type AppService<S> = S;

async fn setup() -> Arc<AppState> {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.keep();
    let storage_root = db_path.join("storage");
    let config = syncfiles_server::config::Config {
        bind_address: "127.0.0.1:0".parse().unwrap(),
        database_url: format!("sqlite:{}", db_path.join("test.db").display()),
        server_url: "http://127.0.0.1:0".to_string(),
        storage_root: storage_root.to_string_lossy().into_owned(),
        users: vec![syncfiles_server::config::UserConfig {
            email: EMAIL.to_string(),
            password_hash: bcrypt::hash(PASSWORD, 4).unwrap(),
            user_id: USER_ID.to_string(),
        }],
    };
    Arc::new(AppState::new(&config).await.expect("app state"))
}

async fn make_app(
    state: Arc<AppState>,
) -> impl Service<Request, Response = ServiceResponse<actix_web::body::EitherBody<BoxBody>>, Error = actix_web::Error> {
    test::init_service(
        App::new()
            .wrap(actix_cors::Cors::permissive())
            .app_data(web::Data::new(state))
            .service(
                web::scope("/api/v1")
                    .route("/auth/login", web::post().to(handlers::login_handler))
                    .route("/session/status", web::get().to(handlers::session_status_handler))
                    .route("/sync/diff", web::post().to(handlers::diff_handler))
                    .route("/sync/upload", web::post().to(handlers::upload_handler))
                    .route("/sync/download", web::post().to(handlers::download_handler))
                    .route("/sync/delete", web::post().to(handlers::delete_handler))
                    .route("/sync/rename", web::post().to(handlers::rename_handler))
                    .route("/sync/move", web::post().to(handlers::move_handler))
                    .route("/sync/copy", web::post().to(handlers::copy_handler))
                    .route("/files/list", web::get().to(handlers::files_list_handler))
                    .route("/queue", web::get().to(handlers::queue_handler))
                    .route("/activity", web::get().to(handlers::activity_handler))
                    .route("/conflicts", web::get().to(handlers::conflicts_handler))
                    .route("/devices", web::get().to(handlers::devices_handler))
                    .route("/devices/revoke", web::post().to(handlers::revoke_device_handler))
                    .route("/storage/stats", web::get().to(handlers::storage_stats_handler))
                    .route("/conflicts/resolve", web::post().to(handlers::resolve_conflict_handler))
                    .default_service(web::route().to(handlers::not_found))
            ),
    )
    .await
}

async fn login<S>(app: &S) -> String
where
    S: Service<Request, Response = ServiceResponse<actix_web::body::EitherBody<BoxBody>>, Error = actix_web::Error>,
{
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(serde_json::json!({
            "email": EMAIL,
            "password": PASSWORD,
            "device_id": DEVICE_ID,
        }))
        .to_request();
    let resp: Value = test::call_and_read_body_json(app, req).await;
    resp["session_id"].as_str().expect("session_id").to_string()
}

async fn upload<S>(app: &S, token: &str, path: &str, content: &str) -> Value
where
    S: Service<Request, Response = ServiceResponse<actix_web::body::EitherBody<BoxBody>>, Error = actix_web::Error>,
{
    let checksum = syncfiles_models::compute_checksum(content.as_bytes());
    let req = test::TestRequest::post()
        .uri("/api/v1/sync/upload")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "session_id": token,
            "device_id": DEVICE_ID,
            "file_id": "",
            "relative_path": path,
            "path_hash": syncfiles_models::compute_path_hash(path),
            "checksum": checksum,
            "size_bytes": content.len() as i64,
            "modified_at": syncfiles_models::now_ms(),
            "idempotency_key": syncfiles_models::uuid_str(),
            "content": base64::engine::general_purpose::STANDARD.encode(content.as_bytes()),
        }))
        .to_request();
    test::call_and_read_body_json(app, req).await
}

fn auth_get(token: &str, uri: &str) -> test::TestRequest {
    test::TestRequest::get()
        .uri(uri)
        .insert_header(("Authorization", format!("Bearer {}", token)))
}

#[actix_web::test]
async fn files_list_returns_uploaded_files() {
    let state = setup().await;
    let app = make_app(state.clone()).await;
    let token = login(&app).await;

    upload(&app, &token, "docs/readme.txt", "hola").await;
    upload(&app, &token, "docs/notes.md", "mundo").await;

    let req = auth_get(&token, "/api/v1/files/list").to_request();
    let resp: Value = test::call_and_read_body_json(&app, req).await;

    assert_eq!(resp["total"], 2);
    let paths: Vec<&str> = resp["files"].as_array().unwrap().iter()
        .map(|f| f["relative_path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"docs/readme.txt"));
    assert!(paths.contains(&"docs/notes.md"));
}

#[actix_web::test]
async fn files_list_requires_auth() {
    let state = setup().await;
    let app = make_app(state).await;

    let req = test::TestRequest::get().uri("/api/v1/files/list").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_web::test]
async fn queue_returns_pending_ops() {
    let state = setup().await;
    let app = make_app(state.clone()).await;
    let token = login(&app).await;

    syncfiles_server::db::queue_op(&state.pool, &syncfiles_models::SyncQueueEntry {
        queue_id: syncfiles_models::uuid_str(),
        file_id: "file-1".to_string(),
        user_id: USER_ID.to_string(),
        device_id: DEVICE_ID.to_string(),
        operation: "upload".to_string(),
        status: "queued".to_string(),
        attempts: 0,
        idempotency_key: syncfiles_models::uuid_str(),
        payload_json: None,
        created_at: syncfiles_models::now_ms(),
        updated_at: syncfiles_models::now_ms(),
        last_error: None,
    }).await.unwrap();

    let req = auth_get(&token, "/api/v1/queue").to_request();
    let resp: Value = test::call_and_read_body_json(&app, req).await;

    assert_eq!(resp["total"], 1);
    assert_eq!(resp["entries"][0]["operation"], "upload");
    assert_eq!(resp["entries"][0]["status"], "queued");
}

#[actix_web::test]
async fn queue_filters_by_device_and_pending_status() {
    let state = setup().await;
    let app = make_app(state.clone()).await;
    let token = login(&app).await;

    for (device, status) in [("device-a", "queued"), ("device-b", "done")] {
        syncfiles_server::db::queue_op(&state.pool, &syncfiles_models::SyncQueueEntry {
            queue_id: syncfiles_models::uuid_str(),
            file_id: format!("file-{}", device),
            user_id: USER_ID.to_string(),
            device_id: device.to_string(),
            operation: "upload".to_string(),
            status: status.to_string(),
            attempts: 0,
            idempotency_key: syncfiles_models::uuid_str(),
            payload_json: None,
            created_at: syncfiles_models::now_ms(),
            updated_at: syncfiles_models::now_ms(),
            last_error: None,
        }).await.unwrap();
    }

    let req = auth_get(&token, "/api/v1/queue?device_id=device-a").to_request();
    let resp: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(resp["entries"].as_array().unwrap().len(), 1);
    assert_eq!(resp["entries"][0]["device_id"], "device-a");

    let req = auth_get(&token, "/api/v1/queue?status=pending").to_request();
    let resp: Value = test::call_and_read_body_json(&app, req).await;
    let entries = resp["entries"].as_array().unwrap();
    assert!(!entries.is_empty());
    assert!(entries.iter().all(|e| e["status"] == "queued" || e["status"] == "retry"));
}

#[actix_web::test]
async fn activity_returns_audit_events() {
    let state = setup().await;
    let app = make_app(state).await;
    let token = login(&app).await;

    upload(&app, &token, "docs/act.txt", "x").await;

    let req = auth_get(&token, "/api/v1/activity").to_request();
    let resp: Value = test::call_and_read_body_json(&app, req).await;

    assert!(resp["total"].as_i64().unwrap() > 0);
    let events = resp["events"].as_array().unwrap();
    assert!(events.iter().any(|e| e["event_name"] == "auth.login_success"));
}

#[actix_web::test]
async fn conflicts_returns_unresolved_only() {
    let state = setup().await;
    let app = make_app(state.clone()).await;
    let token = login(&app).await;

    let mk_conflict = |resolved: bool| syncfiles_models::Conflict {
        conflict_id: syncfiles_models::uuid_str(),
        file_id: syncfiles_models::uuid_str(),
        user_id: USER_ID.to_string(),
        device_local: Some("device-a".to_string()),
        device_remote: Some("device-b".to_string()),
        local_checksum: Some("aaa".to_string()),
        remote_checksum: Some("bbb".to_string()),
        conflict_type: "checksum_mismatch".to_string(),
        strategy: "last_write_wins".to_string(),
        created_at: syncfiles_models::now_ms(),
        resolved_at: if resolved { Some(syncfiles_models::now_ms()) } else { None },
        resolved_by: None,
    };
    syncfiles_server::db::mark_conflict(&state.pool, &mk_conflict(false)).await.unwrap();
    syncfiles_server::db::mark_conflict(&state.pool, &mk_conflict(true)).await.unwrap();

    let req = auth_get(&token, "/api/v1/conflicts").to_request();
    let resp: Value = test::call_and_read_body_json(&app, req).await;

    assert_eq!(resp["total"], 1);
    assert!(resp["conflicts"][0]["resolved_at"].is_null());
}

#[actix_web::test]
async fn devices_lists_user_devices_with_sessions() {
    let state = setup().await;
    let app = make_app(state).await;
    let token = login(&app).await;

    let req = auth_get(&token, "/api/v1/devices").to_request();
    let resp: Value = test::call_and_read_body_json(&app, req).await;

    assert!(resp["total"].as_i64().unwrap() >= 1);
    let devices = resp["devices"].as_array().unwrap();
    assert!(devices.iter().any(|d| d["device_id"] == DEVICE_ID && d["active_sessions"].as_i64().unwrap() >= 1));
}

#[actix_web::test]
async fn storage_stats_sums_user_files() {
    let state = setup().await;
    let app = make_app(state).await;
    let token = login(&app).await;

    upload(&app, &token, "a.txt", "hola").await;
    upload(&app, &token, "b.txt", "mundo!").await;

    let req = auth_get(&token, "/api/v1/storage/stats").to_request();
    let resp: Value = test::call_and_read_body_json(&app, req).await;

    assert_eq!(resp["file_count"], 2);
    assert_eq!(resp["used_bytes"], ("hola".len() + "mundo!".len()) as i64);
    assert!(resp["last_modified_at"].as_i64().unwrap() > 0);
}

#[actix_web::test]
async fn new_endpoints_reject_bad_tokens() {
    let state = setup().await;
    let app = make_app(state).await;

    for uri in ["/api/v1/files/list", "/api/v1/queue", "/api/v1/activity", "/api/v1/conflicts", "/api/v1/devices", "/api/v1/storage/stats"] {
        let req = auth_get("token-invalido", uri).to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401, "esperaba 401 para {}", uri);
    }

    let req = test::TestRequest::post()
        .uri("/api/v1/devices/revoke")
        .insert_header(("Authorization", "Bearer token-invalido"))
        .set_json(serde_json::json!({
            "session_id": "token-invalido",
            "device_id": DEVICE_ID,
            "target_device_id": "otro",
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);
}

#[actix_web::test]
async fn upload_binary_content_round_trips_via_download() {
    let state = setup().await;
    let app = make_app(state).await;
    let token = login(&app).await;

    // Contenido binario no-UTF8 seguro: bytes 0..255
    let content: Vec<u8> = (0..=255u8).collect();
    let checksum = syncfiles_models::compute_checksum(&content);
    let req = test::TestRequest::post()
        .uri("/api/v1/sync/upload")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "session_id": token,
            "device_id": DEVICE_ID,
            "file_id": "",
            "relative_path": "bin/blob.dat",
            "path_hash": syncfiles_models::compute_path_hash("bin/blob.dat"),
            "checksum": checksum,
            "size_bytes": content.len() as i64,
            "modified_at": syncfiles_models::now_ms(),
            "idempotency_key": syncfiles_models::uuid_str(),
            "content": base64::engine::general_purpose::STANDARD.encode(&content),
        }))
        .to_request();
    let resp: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(resp["status"], "ok");

    let files_req = auth_get(&token, "/api/v1/files/list").to_request();
    let files: Value = test::call_and_read_body_json(&app, files_req).await;
    let entry = files["files"].as_array().unwrap().iter()
        .find(|f| f["relative_path"] == "bin/blob.dat")
        .expect("archivo subido debe aparecer en files/list");
    let file_id = entry["file_id"].as_str().unwrap().to_string();
    let path_hash = entry["path_hash"].as_str().unwrap().to_string();

    let dl_req = test::TestRequest::post()
        .uri("/api/v1/sync/download")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "session_id": token,
            "device_id": DEVICE_ID,
            "file_id": file_id,
            "path_hash": path_hash,
            "idempotency_key": syncfiles_models::uuid_str(),
        }))
        .to_request();
    let dl: Value = test::call_and_read_body_json(&app, dl_req).await;
    let b64 = dl["content"].as_str().unwrap();
    let decoded = base64::engine::general_purpose::STANDARD.decode(b64).unwrap();
    assert_eq!(decoded, content, "el binario debe sobrevivir upload→download");
}

#[actix_web::test]
async fn rename_moves_file_path() {
    let state = setup().await;
    let app = make_app(state).await;
    let token = login(&app).await;
    let up = upload(&app, &token, "docs/viejo.txt", "contenido").await;
    assert_eq!(up["status"], "ok");

    let files: Value = {
        let req = auth_get(&token, "/api/v1/files/list").to_request();
        test::call_and_read_body_json(&app, req).await
    };
    let file_id = files["files"].as_array().unwrap().iter()
        .find(|f| f["relative_path"] == "docs/viejo.txt")
        .expect("archivo")
        ["file_id"].as_str().unwrap().to_string();

    let req = test::TestRequest::post()
        .uri("/api/v1/sync/rename")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "session_id": token,
            "device_id": DEVICE_ID,
            "file_id": file_id,
            "old_path": "docs/viejo.txt",
            "new_path": "docs/nuevo.txt",
            "idempotency_key": syncfiles_models::uuid_str(),
        }))
        .to_request();
    let resp: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(resp["status"], "ok");

    let files: Value = {
        let req = auth_get(&token, "/api/v1/files/list").to_request();
        test::call_and_read_body_json(&app, req).await
    };
    let paths: Vec<&str> = files["files"].as_array().unwrap().iter()
        .map(|f| f["relative_path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"docs/nuevo.txt"), "debe existir la ruta nueva: {:?}", paths);
    assert!(!paths.contains(&"docs/viejo.txt"), "no debe existir la ruta vieja: {:?}", paths);
}

#[actix_web::test]
async fn move_relocates_file_path() {
    let state = setup().await;
    let app = make_app(state).await;
    let token = login(&app).await;
    upload(&app, &token, "orig/a.txt", "data").await;

    let files: Value = {
        let req = auth_get(&token, "/api/v1/files/list").to_request();
        test::call_and_read_body_json(&app, req).await
    };
    let file_id = files["files"].as_array().unwrap().iter()
        .find(|f| f["relative_path"] == "orig/a.txt")
        .expect("archivo")
        ["file_id"].as_str().unwrap().to_string();

    let req = test::TestRequest::post()
        .uri("/api/v1/sync/move")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "session_id": token,
            "device_id": DEVICE_ID,
            "file_id": file_id,
            "old_path": "orig/a.txt",
            "new_path": "dest/deep/b.txt",
            "idempotency_key": syncfiles_models::uuid_str(),
        }))
        .to_request();
    let resp: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(resp["status"], "ok");

    let files: Value = {
        let req = auth_get(&token, "/api/v1/files/list").to_request();
        test::call_and_read_body_json(&app, req).await
    };
    let paths: Vec<&str> = files["files"].as_array().unwrap().iter()
        .map(|f| f["relative_path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&"dest/deep/b.txt"), "debe existir la ruta movida: {:?}", paths);
}

#[actix_web::test]
async fn copy_creates_new_file_with_content() {
    let state = setup().await;
    let app = make_app(state).await;
    let token = login(&app).await;
    upload(&app, &token, "orig/copy-src.txt", "mundo").await;

    let files: Value = {
        let req = auth_get(&token, "/api/v1/files/list").to_request();
        test::call_and_read_body_json(&app, req).await
    };
    let file_id = files["files"].as_array().unwrap().iter()
        .find(|f| f["relative_path"] == "orig/copy-src.txt")
        .expect("archivo")
        ["file_id"].as_str().unwrap().to_string();

    let req = test::TestRequest::post()
        .uri("/api/v1/sync/copy")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "session_id": token,
            "device_id": DEVICE_ID,
            "file_id": file_id,
            "source_path": "orig/copy-src.txt",
            "destination_path": "copia/copy-dst.txt",
            "idempotency_key": syncfiles_models::uuid_str(),
        }))
        .to_request();
    let resp: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(resp["status"], "ok");

    let files: Value = {
        let req = auth_get(&token, "/api/v1/files/list").to_request();
        test::call_and_read_body_json(&app, req).await
    };
    let dst = files["files"].as_array().unwrap().iter()
        .find(|f| f["relative_path"] == "copia/copy-dst.txt")
        .expect("copia creada");
    assert_eq!(dst["size_bytes"], "mundo".len() as i64);
    assert_ne!(dst["file_id"].as_str().unwrap(), file_id, "la copia tiene file_id propio");
}

#[actix_web::test]
async fn diff_returns_changes_since_cursor() {
    let state = setup().await;
    let app = make_app(state).await;
    let token = login(&app).await;
    upload(&app, &token, "diff/a.txt", "x").await;

    // Sesión de otro dispositivo: como un segundo cliente sincronizando
    let token_b = {
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(serde_json::json!({
                "email": EMAIL, "password": PASSWORD, "device_id": "device-otro",
            }))
            .to_request();
        let resp: Value = test::call_and_read_body_json(&app, req).await;
        resp["session_id"].as_str().expect("session_id").to_string()
    };

    let diff_req = |since: i64, device: &str| {
        test::TestRequest::post()
            .uri("/api/v1/sync/diff")
            .insert_header(("Authorization", format!("Bearer {}", token_b)))
            .set_json(serde_json::json!({
                "session_id": token_b,
                "device_id": device,
                "since": since,
                "request_id": syncfiles_models::uuid_str(),
            }))
            .to_request()
    };

    // Desde 0, otro dispositivo ve el upload
    let resp: Value = test::call_and_read_body_json(&app, diff_req(0, "device-otro")).await;
    let changes = resp["changes"].as_array().unwrap();
    assert!(changes.iter().any(|c| c["relative_path"] == "diff/a.txt" && c["operation"] == "upload"));
    let server_seq = resp["server_seq"].as_i64().unwrap();
    assert!(server_seq > 0);

    // Con cursor al día: no repite entradas ya vistas
    let resp: Value = test::call_and_read_body_json(&app, diff_req(server_seq, "device-otro")).await;
    assert!(resp["changes"].as_array().unwrap().is_empty());
    assert_eq!(resp["server_seq"].as_i64().unwrap(), server_seq);

    // El dispositivo que originó el cambio no lo recibe de vuelta
    let resp: Value = test::call_and_read_body_json(&app, diff_req(0, DEVICE_ID)).await;
    assert!(resp["changes"].as_array().unwrap().is_empty());
}

#[actix_web::test]
async fn rename_and_copy_appear_in_diff_for_other_devices() {
    let state = setup().await;
    let app = make_app(state).await;
    let token = login(&app).await;
    upload(&app, &token, "docs/viejo.txt", "contenido").await;

    let files: Value = {
        let req = auth_get(&token, "/api/v1/files/list").to_request();
        test::call_and_read_body_json(&app, req).await
    };
    let file_id = files["files"].as_array().unwrap().iter()
        .find(|f| f["relative_path"] == "docs/viejo.txt")
        .expect("archivo")
        ["file_id"].as_str().unwrap().to_string();

    let rename_req = test::TestRequest::post()
        .uri("/api/v1/sync/rename")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "session_id": token,
            "device_id": DEVICE_ID,
            "file_id": file_id,
            "old_path": "docs/viejo.txt",
            "new_path": "docs/nuevo.txt",
            "idempotency_key": syncfiles_models::uuid_str(),
        }))
        .to_request();
    let resp: Value = test::call_and_read_body_json(&app, rename_req).await;
    assert_eq!(resp["status"], "ok");

    let copy_req = test::TestRequest::post()
        .uri("/api/v1/sync/copy")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "session_id": token,
            "device_id": DEVICE_ID,
            "file_id": file_id,
            "source_path": "docs/nuevo.txt",
            "destination_path": "docs/copia.txt",
            "idempotency_key": syncfiles_models::uuid_str(),
        }))
        .to_request();
    let resp: Value = test::call_and_read_body_json(&app, copy_req).await;
    assert_eq!(resp["status"], "ok");

    // Otro dispositivo consulta el diff y ve rename (con old_path) y copy
    let token_b = {
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(serde_json::json!({
                "email": EMAIL, "password": PASSWORD, "device_id": "device-otro",
            }))
            .to_request();
        let resp: Value = test::call_and_read_body_json(&app, req).await;
        resp["session_id"].as_str().expect("session_id").to_string()
    };
    let req = test::TestRequest::post()
        .uri("/api/v1/sync/diff")
        .insert_header(("Authorization", format!("Bearer {}", token_b)))
        .set_json(serde_json::json!({
            "session_id": token_b,
            "device_id": "device-otro",
            "since": 0,
            "request_id": syncfiles_models::uuid_str(),
        }))
        .to_request();
    let resp: Value = test::call_and_read_body_json(&app, req).await;
    let changes = resp["changes"].as_array().unwrap();

    let rename = changes.iter().find(|c| c["operation"] == "rename")
        .expect("diff incluye rename");
    assert_eq!(rename["old_path"], "docs/viejo.txt");
    assert_eq!(rename["relative_path"], "docs/nuevo.txt");
    assert_eq!(rename["file_id"], file_id.as_str());

    let copy = changes.iter().find(|c| c["operation"] == "copy")
        .expect("diff incluye copy");
    assert_eq!(copy["relative_path"], "docs/copia.txt");
    assert_ne!(copy["file_id"].as_str().unwrap(), file_id, "copy reporta el file_id nuevo");
}

#[actix_web::test]
async fn revoke_device_revokes_other_device_sessions() {
    let state = setup().await;
    let app = make_app(state).await;

    // Sesión A: el dispositivo web actual
    let token_a = {
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(serde_json::json!({
                "email": EMAIL, "password": PASSWORD, "device_id": "web-actual",
            }))
            .to_request();
        let resp: Value = test::call_and_read_body_json(&app, req).await;
        resp["session_id"].as_str().unwrap().to_string()
    };
    // Sesión B: otro dispositivo a revocar
    let token_b = {
        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .set_json(serde_json::json!({
                "email": EMAIL, "password": PASSWORD, "device_id": "telefono-viejo",
            }))
            .to_request();
        let resp: Value = test::call_and_read_body_json(&app, req).await;
        resp["session_id"].as_str().unwrap().to_string()
    };
    assert_ne!(token_a, token_b);

    // Ambas sesiones activas antes de revocar
    let devices: Value = {
        let req = auth_get(&token_a, "/api/v1/devices").to_request();
        test::call_and_read_body_json(&app, req).await
    };
    let dev_b = devices["devices"].as_array().unwrap().iter()
        .find(|d| d["device_id"] == "telefono-viejo")
        .expect("dispositivo B registrado");
    assert!(dev_b["active_sessions"].as_i64().unwrap() >= 1);

    // Revocar el dispositivo B desde la sesión A
    let req = test::TestRequest::post()
        .uri("/api/v1/devices/revoke")
        .insert_header(("Authorization", format!("Bearer {}", token_a)))
        .set_json(serde_json::json!({
            "session_id": token_a,
            "device_id": "web-actual",
            "target_device_id": "telefono-viejo",
        }))
        .to_request();
    let resp: Value = test::call_and_read_body_json(&app, req).await;
    assert_eq!(resp["status"], "ok");
    assert_eq!(resp["data"]["sessions_revoked"], 1);

    // La sesión B ya no es válida
    let req = auth_get(&token_b, "/api/v1/files/list").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401, "la sesión revocada debe fallar");

    // La sesión A sigue viva
    let req = auth_get(&token_a, "/api/v1/files/list").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200, "la sesión que revoca debe seguir activa");

    // El evento queda en la auditoría
    let req = auth_get(&token_a, "/api/v1/activity").to_request();
    let activity: Value = test::call_and_read_body_json(&app, req).await;
    assert!(activity["events"].as_array().unwrap().iter().any(|e| e["event_name"] == "device.revoked"));
}

#[actix_web::test]
async fn revoke_rejects_own_device_and_unknown_device() {
    let state = setup().await;
    let app = make_app(state).await;
    let token = login(&app).await;

    // No puedes revocar tu propio dispositivo actual
    let req = test::TestRequest::post()
        .uri("/api/v1/devices/revoke")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "session_id": token,
            "device_id": DEVICE_ID,
            "target_device_id": DEVICE_ID,
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    // Dispositivo inexistente → 400 NOT_FOUND
    let req = test::TestRequest::post()
        .uri("/api/v1/devices/revoke")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "session_id": token,
            "device_id": DEVICE_ID,
            "target_device_id": "no-existe",
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], "NOT_FOUND");
}

// ===== Tests de seguridad (hardening V1, plan 1789441050663) =====

/// Expira una sesión directamente en la DB (simula el paso del TTL de 24h).
async fn expire_session(state: &Arc<AppState>, token: &str) {
    sqlx::query("UPDATE sessions SET expires_at = ? WHERE token_hash = ?")
        .bind(syncfiles_models::now_ms() - 1)
        .bind(token)
        .execute(&state.pool)
        .await
        .expect("expirar sesión");
}

#[actix_web::test]
async fn expired_session_gets_401_everywhere() {
    let state = setup().await;
    let app = make_app(state.clone()).await;
    let token = login(&app).await;

    // Antes de expirar: la sesión es válida
    let req = auth_get(&token, "/api/v1/session/status").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    expire_session(&state, &token).await;

    // session/status → 401
    let req = auth_get(&token, "/api/v1/session/status").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401, "status con sesión expirada");

    // Mutaciones con sesión expirada → 401
    let req = test::TestRequest::post()
        .uri("/api/v1/sync/upload")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "session_id": token,
            "device_id": DEVICE_ID,
            "file_id": "",
            "relative_path": "a.txt",
            "path_hash": syncfiles_models::compute_path_hash("a.txt"),
            "checksum": syncfiles_models::compute_checksum(b"x"),
            "size_bytes": 1,
            "modified_at": syncfiles_models::now_ms(),
            "idempotency_key": syncfiles_models::uuid_str(),
            "content": base64::engine::general_purpose::STANDARD.encode(b"x"),
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401, "upload con sesión expirada");

    // GET protegidos → 401
    for uri in ["/api/v1/files/list", "/api/v1/conflicts", "/api/v1/devices"] {
        let req = auth_get(&token, uri).to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401, "GET {} con sesión expirada", uri);
    }
}

#[actix_web::test]
async fn upload_rejects_invalid_path_hash() {
    let state = setup().await;
    let app = make_app(state).await;
    let token = login(&app).await;

    // path_hash ≠ SHA-256 de relative_path → 400 INVALID_PATH_HASH
    let req = test::TestRequest::post()
        .uri("/api/v1/sync/upload")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "session_id": token,
            "device_id": DEVICE_ID,
            "file_id": "",
            "relative_path": "docs/a.txt",
            "path_hash": "hash-falso-que-no-corresponde",
            "checksum": syncfiles_models::compute_checksum(b"hola"),
            "size_bytes": 4,
            "modified_at": syncfiles_models::now_ms(),
            "idempotency_key": syncfiles_models::uuid_str(),
            "content": base64::engine::general_purpose::STANDARD.encode(b"hola"),
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["code"], "INVALID_PATH_HASH",
        "debe rechazar path_hash incoherente: {body}"
    );
}

#[actix_web::test]
async fn upload_rejects_traversal_and_absolute_paths() {
    let state = setup().await;
    let app = make_app(state).await;
    let token = login(&app).await;

    // `..` que escapa de la raíz del usuario → 400
    let evil = "../../etc/passwd";
    let req = test::TestRequest::post()
        .uri("/api/v1/sync/upload")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "session_id": token,
            "device_id": DEVICE_ID,
            "file_id": "",
            "relative_path": evil,
            "path_hash": syncfiles_models::compute_path_hash(evil),
            "checksum": syncfiles_models::compute_checksum(b"mal"),
            "size_bytes": 3,
            "modified_at": syncfiles_models::now_ms(),
            "idempotency_key": syncfiles_models::uuid_str(),
            "content": base64::engine::general_purpose::STANDARD.encode(b"mal"),
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400, "path traversal debe rechazarse");

    // Path absoluta → 400
    let abs = "/etc/passwd";
    let req = test::TestRequest::post()
        .uri("/api/v1/sync/upload")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(serde_json::json!({
            "session_id": token,
            "device_id": DEVICE_ID,
            "file_id": "",
            "relative_path": abs,
            "path_hash": syncfiles_models::compute_path_hash(abs.trim_start_matches('/')),
            "checksum": syncfiles_models::compute_checksum(b"mal"),
            "size_bytes": 3,
            "modified_at": syncfiles_models::now_ms(),
            "idempotency_key": syncfiles_models::uuid_str(),
            "content": base64::engine::general_purpose::STANDARD.encode(b"mal"),
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400, "path absoluta debe rechazarse");

    // Verificación: ningún archivo salió del storage root del usuario
    let listing: Value = {
        let req = auth_get(&token, "/api/v1/files/list").to_request();
        test::call_and_read_body_json(&app, req).await
    };
    for f in listing["files"].as_array().unwrap() {
        let path = f["relative_path"].as_str().unwrap_or("");
        assert!(!path.contains(".."), "ningún path con .. en el listado: {path}");
    }
}

#[actix_web::test]
async fn rate_limiter_blocks_excess_requests() {
    use std::time::Duration;

    // Token bucket directo: rps=2, burst=3 (sin HTTP, unit test del limiter)
    let limiter = syncfiles_server::middleware::RateLimiter::new(2, 3);
    let key = "1.2.3.4:9999";
    // Las primeras 3 (burst) pasan
    assert!(limiter.allow(key), "burst 1");
    assert!(limiter.allow(key), "burst 2");
    assert!(limiter.allow(key), "burst 3");
    // La 4ª excede el burst → bloqueada
    assert!(!limiter.allow(key), "excede burst");

    // Otra IP no se ve afectada
    assert!(limiter.allow("5.6.7.8:1234"), "clave distinta no se afecta");

    // Tras 1s la ventana se renueva (tokens se recargan)
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert!(limiter.allow(key), "ventana renovada tras 1s");
}

#[actix_web::test]
async fn content_type_middleware_rejects_non_json() {
    // El middleware exige application/json en /api/ con body
    use actix_web::middleware::from_fn;
    use syncfiles_server::middleware::content_type_middleware;

    let state = setup().await;
    let app = test::init_service(
        App::new()
            .wrap(from_fn(content_type_middleware))
            .app_data(web::Data::new(state))
            .route("/api/v1/ping", web::post().to(|| async { "pong" })),
    )
    .await;

    // Content-Type text/plain → 415
    let req = test::TestRequest::post()
        .uri("/api/v1/ping")
        .insert_header(("Content-Type", "text/plain"))
        .set_payload("hola")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 415);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["code"], "INVALID_CONTENT_TYPE");

    // Content-Type application/json → pasa
    let req = test::TestRequest::post()
        .uri("/api/v1/ping")
        .insert_header(("Content-Type", "application/json"))
        .set_json(serde_json::json!({}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}
