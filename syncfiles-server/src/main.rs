use anyhow::Result;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[actix_web::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app_config = syncfiles_server::config::Config::load()?;
    let app_state = Arc::new(syncfiles_server::state::AppState::new(&app_config).await?);

    tracing::info!("Servidor SyncFiles iniciado en {}", app_config.bind_address);

    let bind = app_config.bind_address;

    actix_web::HttpServer::new(move || {
        actix_web::App::new()
            .app_data(actix_web::web::Data::new(app_state.clone()))
            .service(
                actix_web::web::scope("/api/v1")
                    .route("/auth/login", actix_web::web::post().to(syncfiles_server::handlers::login_handler))
                    .route("/auth/logout", actix_web::web::post().to(syncfiles_server::handlers::logout_handler))
                    .route("/session/status", actix_web::web::get().to(syncfiles_server::handlers::session_status_handler))
                    .route("/sync/diff", actix_web::web::post().to(syncfiles_server::handlers::diff_handler))
                    .route("/sync/upload", actix_web::web::post().to(syncfiles_server::handlers::upload_handler))
                    .route("/sync/download", actix_web::web::post().to(syncfiles_server::handlers::download_handler))
                    .route("/sync/delete", actix_web::web::post().to(syncfiles_server::handlers::delete_handler))
                    .route("/sync/rename", actix_web::web::post().to(syncfiles_server::handlers::rename_handler))
                    .route("/sync/move", actix_web::web::post().to(syncfiles_server::handlers::move_handler))
                    .route("/sync/copy", actix_web::web::post().to(syncfiles_server::handlers::copy_handler))
                    .route("/conflicts/resolve", actix_web::web::post().to(syncfiles_server::handlers::resolve_conflict_handler))
                    .default_service(actix_web::web::route().to(syncfiles_server::handlers::not_found))
            )
    })
    .bind(&bind)?
    .run()
    .await?;

    Ok(())
}
