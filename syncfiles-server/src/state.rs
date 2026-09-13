use std::str::FromStr;
use std::sync::Arc;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;
use crate::config::Config;
use crate::db;
use crate::storage::{StorageProvider, LocalDiskStorageProvider};

pub struct AppState {
    pub pool: SqlitePool,
    pub config: Config,
    pub server_seq: Arc<std::sync::atomic::AtomicI64>,
    pub storage: Arc<dyn StorageProvider>,
}

impl AppState {
    pub async fn new(config: &Config) -> anyhow::Result<Self> {
        if let Some(path) = config.database_url.strip_prefix("sqlite:") {
            let path = path.strip_prefix("//").unwrap_or(path);
            if path != ":memory:" {
                if let Some(parent) = std::path::Path::new(path).parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent)?;
                    }
                }
            }
        }
        let mut connect_options = SqliteConnectOptions::from_str(&config.database_url)?;
        if let Some(path) = config.database_url.strip_prefix("sqlite:") {
            if path != ":memory:" && !std::path::Path::new(path).exists() {
                connect_options = connect_options.create_if_missing(true);
            }
        }
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connect_options)
            .await?;

        db::init_db(&pool).await?;

        for user in &config.users {
            if db::get_user_by_id(&pool, &user.user_id).await?.is_none() {
                db::create_user(&pool, &user.email, &user.password_hash, &user.user_id).await?;
            }
            sqlx::query(
                "INSERT OR IGNORE INTO devices (device_id, user_id, platform, device_name, last_seen_at, created_at) VALUES (?, ?, ?, ?, ?, ?)"
            )
            .bind(format!("bootstrap-{}", user.user_id))
            .bind(&user.user_id)
            .bind("server")
            .bind("bootstrap")
            .bind(crate::models::now_ms())
            .bind(crate::models::now_ms())
            .execute(&pool)
            .await?;
        }

        // Contador decorativo para handlers de solo lectura; los que mutan
        // usan el seq del change_log (fuente de verdad del diff).
        let seq = db::get_max_seq(&pool, None).await.unwrap_or(0);

        let storage = Arc::new(LocalDiskStorageProvider::new(&config.storage_root));
        storage.init()?;

        Ok(Self {
            pool,
            config: config.clone(),
            server_seq: Arc::new(std::sync::atomic::AtomicI64::new(seq)),
            storage,
        })
    }

    pub fn next_server_seq(&self) -> i64 {
        self.server_seq.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
    }
}