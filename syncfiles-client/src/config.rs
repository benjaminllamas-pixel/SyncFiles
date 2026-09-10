use anyhow::Result;
use std::path::PathBuf;
use dirs;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server_url: String,
    pub email: String,
    pub password: String,
    pub device_id: String,
    pub sync_root: PathBuf,
    pub polling_interval_secs: u64,
}

impl Config {
    /// Directorio de datos del cliente. Sobreescrible con `SF_DATA_DIR`
    /// (útil para pruebas E2E aisladas y multi-instancia).
    pub fn data_dir() -> PathBuf {
        if let Ok(dir) = std::env::var("SF_DATA_DIR") {
            return PathBuf::from(dir);
        }
        dirs::data_dir()
            .expect("no data dir")
            .join("syncfiles")
    }

    pub fn config_path() -> PathBuf {
        Self::data_dir().join("config.json")
    }

    pub fn load() -> Result<Self> {
        let data_dir = Self::data_dir();
        std::fs::create_dir_all(&data_dir)?;

        if let Ok(content) = std::fs::read_to_string(Self::config_path()) {
            if let Ok(config) = serde_json::from_str::<Config>(&content) {
                return Ok(config);
            }
        }

        let server_url = std::env::var("SF_SERVER_URL")
            .or_else(|_| std::env::var("SYNCFILES_SERVER_URL"))
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
        let email = std::env::var("SF_EMAIL").unwrap_or_default();
        let password = std::env::var("SF_PASSWORD").unwrap_or_default();
        let device_id = std::env::var("SF_DEVICE_ID")
            .unwrap_or_else(|_| format!("desktop-{}", uuid::Uuid::new_v4()));
        let sync_root: PathBuf = std::env::var("SF_SYNC_ROOT")
            .map(|p| p.into())
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .expect("no home dir")
                    .join("SyncFiles")
            });
        let polling_interval_secs: u64 = std::env::var("SF_POLLING_INTERVAL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        std::fs::create_dir_all(&sync_root)?;

        Ok(Self {
            server_url,
            email,
            password,
            device_id,
            sync_root,
            polling_interval_secs,
        })
    }

    pub fn save(&self) -> Result<()> {
        let data_dir = Self::data_dir();
        std::fs::create_dir_all(&data_dir)?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(Self::config_path(), content)?;
        Ok(())
    }
}
