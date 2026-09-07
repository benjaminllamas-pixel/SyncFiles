pub mod config;
pub mod models;
pub mod db;
pub mod auth;
pub mod handlers;
pub mod state;
pub mod storage;

use anyhow::Result;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[actix_web::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app_config = crate::config::Config::load()?;
    let app_state = Arc::new(crate::state::AppState::new(&app_config).await?);

    tracing::info!("Servidor SyncFiles iniciado en {}", app_config.bind_address);

    let bind = app_config.bind_address.clone();

    actix_web::HttpServer::new(move || {
        actix_web::App::new()
            .app_data(actix_web::web::Data::new(app_state.clone()))
            .service(
                actix_web::web::scope("/api/v1")
                    .route("/auth/login", actix_web::web::post().to(crate::handlers::login_handler))
                    .route("/auth/logout", actix_web::web::post().to(crate::handlers::logout_handler))
                    .route("/session/status", actix_web::web::get().to(crate::handlers::session_status_handler))
                    .route("/sync/diff", actix_web::web::post().to(crate::handlers::diff_handler))
                    .route("/sync/upload", actix_web::web::post().to(crate::handlers::upload_handler))
                    .route("/sync/download", actix_web::web::post().to(crate::handlers::download_handler))
                    .route("/sync/delete", actix_web::web::post().to(crate::handlers::delete_handler))
                    .route("/sync/rename", actix_web::web::post().to(crate::handlers::rename_handler))
                    .route("/sync/move", actix_web::web::post().to(crate::handlers::move_handler))
                    .route("/sync/copy", actix_web::web::post().to(crate::handlers::copy_handler))
                    .route("/conflicts/resolve", actix_web::web::post().to(crate::handlers::resolve_conflict_handler))
                    .default_service(actix_web::web::route().to(crate::handlers::not_found))
            )
    })
    .bind(&bind)?
    .run()
    .await?;

    Ok(())
}