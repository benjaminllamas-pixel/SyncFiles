//! SyncFiles server — Actix-web REST API + dashboard web.
//!
//! Middlewares (registrados en orden):
//! 1. `no_cache_statics` — headers no-cache para assets estáticos.
//! 2. `trace_context` — correlation ID + logging estructurado por solicitud.
//! 3. `content_type_middleware` — Content-Type application/json en /api/.
//! 4. `rate_limit_middleware` — token bucket por IP (60 rps, burst 120).
//!
//! Health checks: GET /health/live (liveness) y /health/ready (readiness DB+storage).
//! Migraciones: sistema versionado en `syncfiles-server/migrations/` al arrancar.

use actix_files::Files;
use actix_web::middleware::from_fn;
use actix_web::web;
use actix_web::{App, HttpServer};
use anyhow::Result;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use syncfiles_server::middleware::{build_rate_limiter, no_cache_statics, content_type_middleware, rate_limit_middleware, trace_context};
use syncfiles_server::state::AppState;

#[actix_web::main]
async fn main() -> Result<()> {
    // Logging JSON estructurado (para correlacionar con request_id).
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout).json())
        .init();

    let app_config = syncfiles_server::config::Config::load()?;
    let app_state = Arc::new(AppState::new(&app_config).await?);

    // Ejecutar migraciones versionadas (idempotentes).
    let migrations_dir = std::path::Path::new("./syncfiles-server/migrations");
    if migrations_dir.is_dir() {
        if let Err(e) = syncfiles_server::migrations::run_migrations(&app_state.pool, migrations_dir).await {
            tracing::warn!("Migraciones fallaron (se continua con schema actual): {}", e);
        }
    }

    let rate_limiter = build_rate_limiter();

    tracing::info!("Servidor SyncFiles iniciado en {}", app_config.bind_address);

    let bind = app_config.bind_address;
    let static_dir = std::env::var("SF_STATIC_DIR")
        .ok()
        .filter(|p| std::path::Path::new(p).is_dir())
        .or_else(|| {
            ["./static", "./syncfiles-server/static"]
                .iter()
                .find(|p| std::path::Path::new(p).is_dir())
                .map(|p| p.to_string())
        })
        .unwrap_or_else(|| {
            tracing::warn!("Carpeta static/ no encontrada: el dashboard web no se servirá");
            "./static".to_string()
        });
    tracing::info!("Sirviendo dashboard web desde {}", static_dir);

    let max_upload_bytes: usize = std::env::var("SF_MAX_UPLOAD_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(104_857_600); // 100 MiB

    HttpServer::new(move || {
        let cors = actix_cors::Cors::permissive()
            .supports_credentials()
            .max_age(3600);

        let static_dir = static_dir.clone();
        let rate_limiter = rate_limiter.clone();

        App::new()
            .wrap(cors)
            .wrap(from_fn(no_cache_statics))
            .wrap(from_fn(trace_context))
            .wrap(from_fn(content_type_middleware))
            .wrap(from_fn(rate_limit_middleware))
            .app_data(web::Data::new(rate_limiter))
            .app_data(web::Data::new(app_state.clone()))
            .app_data(web::Data::new(max_upload_bytes))
            .service(
                web::scope("/api/v1")
                    .route("/auth/login", web::post().to(syncfiles_server::handlers::login_handler))
                    .route("/auth/logout", web::post().to(syncfiles_server::handlers::logout_handler))
                    .route("/session/status", web::get().to(syncfiles_server::handlers::session_status_handler))
                    .route("/sync/diff", web::post().to(syncfiles_server::handlers::diff_handler))
                    .route("/sync/upload", web::post().to(syncfiles_server::handlers::upload_handler))
                    .route("/sync/download", web::post().to(syncfiles_server::handlers::download_handler))
                    .route("/sync/delete", web::post().to(syncfiles_server::handlers::delete_handler))
                    .route("/sync/rename", web::post().to(syncfiles_server::handlers::rename_handler))
                    .route("/sync/move", web::post().to(syncfiles_server::handlers::move_handler))
                    .route("/sync/copy", web::post().to(syncfiles_server::handlers::copy_handler))
                    .route("/files/list", web::get().to(syncfiles_server::handlers::files_list_handler))
                    .route("/queue", web::get().to(syncfiles_server::handlers::queue_handler))
                    .route("/activity", web::get().to(syncfiles_server::handlers::activity_handler))
                    .route("/conflicts", web::get().to(syncfiles_server::handlers::conflicts_handler))
                    .route("/devices", web::get().to(syncfiles_server::handlers::devices_handler))
                    .route("/devices/revoke", web::post().to(syncfiles_server::handlers::revoke_device_handler))
                    .route("/storage/stats", web::get().to(syncfiles_server::handlers::storage_stats_handler))
                    .route("/conflicts/resolve", web::post().to(syncfiles_server::handlers::resolve_conflict_handler))
                    .route("/openapi", web::get().to(|| async {
                        let content = include_str!("../docs/openapi.md");
                        actix_web::HttpResponse::Ok()
                            .content_type("text/markdown; charset=utf-8")
                            .body(content)
                    }))
                    .default_service(web::route().to(syncfiles_server::handlers::not_found))
            )
            .service(
                web::scope("/health")
                    .service(syncfiles_server::health::liveness)
                    .service(syncfiles_server::health::readiness)
            )
            .service(web::redirect("/ui", "/"))
            .service(
                Files::new("/", &static_dir)
                    .index_file("index.html")
                    .redirect_to_slash_directory(),
            )
    })
    .bind(&bind)?
    .run()
    .await?;

    Ok(())
}