mod config;
mod auth;
mod network;
mod metadata;
mod sync;
mod watcher;

use anyhow::Result;
use std::sync::Arc;
use tokio::runtime::Builder;
use tokio::task::LocalSet;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let rt = Builder::new_current_thread()
        .enable_all()
        .build()?;

    let local = LocalSet::new();
    local.block_on(&rt, async {
        let metadata_store = Arc::new(metadata::MetadataStore::init()?);
        let client = Arc::new(network::SyncClient::new(&metadata_store.config.server_url, &metadata_store.config.device_id)?);

        let on_change = Arc::new(|path: String| {
            tracing::info!("Cambio detectado: {}", path);
        });
        let watcher = watcher::FileWatcher::new(
            metadata_store.config.sync_root.clone(),
            on_change,
        );
        let _watcher = watcher.start()?;

        let auth = auth::AuthService::new(client.clone(), metadata_store.clone());
        let auth_handle = tokio::task::spawn_local(async move {
            if let Err(e) = auth.ensure_session().await {
                tracing::error!("Auth error: {}", e);
            }
        });

        let sync_engine = sync::SyncEngine::new(metadata_store.config.clone(), client, metadata_store.clone());

        let sync_handle = tokio::task::spawn_local(async move {
            if let Err(e) = sync_engine.run().await {
                tracing::error!("Sync engine error: {}", e);
            }
        });

        let _ = auth_handle.await;
        let _ = sync_handle.await;
        tokio::signal::ctrl_c().await?;
        tracing::info!("Cliente detenido");
        Ok(())
    })
}