use actix_files::Files;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::{from_fn, Next};
use actix_web::web;
use anyhow::Result;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

async fn no_cache_statics(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let path = req.path().to_owned();
    let mut res = next.call(req).await?;
    if !path.starts_with("/api/") {
        res.headers_mut().insert(
            actix_web::http::header::CACHE_CONTROL,
            actix_web::http::header::HeaderValue::from_static("no-cache"),
        );
    }
    Ok(res)
}

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

    actix_web::HttpServer::new(move || {
        let cors = actix_cors::Cors::permissive()
            .supports_credentials()
            .max_age(3600);

        let static_dir = static_dir.clone();

        actix_web::App::new()
            .wrap(cors)
            // Estáticos sin cache agresivo: el navegador debe revalidar (ETag) en cada carga
            .wrap(from_fn(no_cache_statics))
            .app_data(actix_web::web::Data::new(app_state.clone()))
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
                    .route("/storage/stats", web::get().to(syncfiles_server::handlers::storage_stats_handler))
                    .route("/conflicts/resolve", web::post().to(syncfiles_server::handlers::resolve_conflict_handler))
                    .default_service(web::route().to(syncfiles_server::handlers::not_found))
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
