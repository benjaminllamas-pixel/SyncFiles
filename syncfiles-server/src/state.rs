use std::sync::Arc;
use sqlx::SqlitePool;
use crate::config::Config;
use crate::db;

pub struct AppState {
    pub pool: SqlitePool,
    pub config: Config,
    pub server_seq: Arc<std::sync::atomic::AtomicI64>,
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
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&config.database_url)
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

        let seq = sqlx::query_scalar::<_, String>(
            "SELECT COALESCE((SELECT value FROM metadata WHERE key = 'last_server_seq'), '0')"
        )
        .fetch_optional(&pool)
        .await?
        .unwrap_or("0".to_string());
        let seq: i64 = seq.parse().unwrap_or(0);

        Ok(Self {
            pool,
            config: config.clone(),
            server_seq: Arc::new(std::sync::atomic::AtomicI64::new(seq)),
        })
    }

    pub fn next_server_seq(&self) -> i64 {
        self.server_seq.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
    }
}