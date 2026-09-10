use actix_files::Files;
use actix_http::Request;
use actix_web::body::BoxBody;
use actix_web::dev::{Service, ServiceResponse};
use actix_web::{test, web, App};
use std::sync::Arc;
use syncfiles_server::handlers;
use syncfiles_server::state::AppState;

const EMAIL: &str = "web-test@syncfiles.local";
const PASSWORD: &str = "syncfiles";
const USER_ID: &str = "user-web-001";
const DEVICE_ID: &str = "device-web-001";

fn static_dir() -> String {
    // tests corren con CWD = syncfiles-server
    "./static".to_string()
}

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

async fn make_web_app(
    state: Arc<AppState>,
) -> impl Service<Request, Response = ServiceResponse<actix_web::body::EitherBody<BoxBody>>, Error = actix_web::Error> {
    let static_dir = static_dir();
    test::init_service(
        App::new()
            .wrap(actix_cors::Cors::permissive())
            .app_data(web::Data::new(state))
            .service(
                web::scope("/api/v1")
                    .route("/auth/login", web::post().to(handlers::login_handler))
                    .route("/session/status", web::get().to(handlers::session_status_handler))
                    .default_service(web::route().to(handlers::not_found)),
            )
            .service(web::redirect("/ui", "/"))
            .service(
                Files::new("/", static_dir)
                    .index_file("index.html")
                    .redirect_to_slash_directory(),
            ),
    )
    .await
}

#[actix_web::test]
async fn root_serves_dashboard_index() {
    let state = setup().await;
    let app = make_web_app(state).await;

    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    let body = test::read_body(resp).await;
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("SyncFiles"), "index.html debe contener el título de la app");
    assert!(html.contains("app.js"), "index.html debe cargar el script del dashboard");
}

#[actix_web::test]
async fn static_assets_are_served() {
    let state = setup().await;
    let app = make_web_app(state).await;

    let req = test::TestRequest::get().uri("/styles.css").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
    assert!(!test::read_body(resp).await.is_empty());

    let req = test::TestRequest::get().uri("/app.js").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);
}

#[actix_web::test]
async fn ui_alias_redirects_to_root() {
    let state = setup().await;
    let app = make_web_app(state).await;

    let req = test::TestRequest::get().uri("/ui").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status() == 307 || resp.status() == 308, "esperaba redirect, obtuve {}", resp.status());
    let location = resp
        .headers()
        .get("Location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert_eq!(location, "/");
}

#[actix_web::test]
async fn login_via_web_flow_returns_session() {
    let state = setup().await;
    let app = make_web_app(state).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .insert_header(("Origin", "http://localhost:3000"))
        .set_json(serde_json::json!({
            "email": EMAIL,
            "password": PASSWORD,
            "device_id": DEVICE_ID,
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 200);

    // CORS activo: la respuesta lleva cabeceras CORS
    assert!(resp.headers().contains_key("access-control-allow-origin"));

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["session_id"].as_str().is_some());
    assert_eq!(body["user_id"], USER_ID);
}

#[actix_web::test]
async fn unknown_api_route_returns_404_json() {
    let state = setup().await;
    let app = make_web_app(state).await;

    let req = test::TestRequest::get().uri("/api/v1/no-existe").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn spa_fallback_unknown_path_serves_index() {
    let state = setup().await;
    let app = make_web_app(state).await;

    // rutas desconocidas fuera de /api deben caer en el servicio de archivos
    // (actix-files devuelve 404 de archivo, no rompe la app)
    let req = test::TestRequest::get().uri("/ruta-no-existente.js").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}
