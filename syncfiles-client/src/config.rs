use anyhow::{Result, Context};
use std::path::PathBuf;
use dirs;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_url: String,
    pub email: String,
    pub password: String,
    pub device_id: String,
    pub sync_root: PathBuf,
    pub polling_interval_secs: u64,
}

impl Config {
    pub fn data_dir() -> PathBuf {
        dirs::data_dir()
            .expect("no data dir")
            .join("syncfiles")
    }

    pub fn load() -> Result<Self> {
        let server_url = std::env::var("SF_SERVER_URL")
            .or_else(|_| std::env::var("SYNCFILES_SERVER_URL"))
            .context("falta SF_SERVER_URL o SYNCFILES_SERVER_URL")?;
        let email = std::env::var("SF_EMAIL")
            .context("falta SF_EMAIL")?;
        let password = std::env::var("SF_PASSWORD")
            .context("falta SF_PASSWORD")?;
        let device_id = std::env::var("SF_DEVICE_ID")
            .unwrap_or_else(|_| "test-device-001".to_string());
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
        let data_dir = dirs::data_dir()
            .expect("no data dir")
            .join("syncfiles");
        std::fs::create_dir_all(&data_dir)?;

        Ok(Self {
            server_url,
            email,
            password,
            device_id,
            sync_root,
            polling_interval_secs,
        })
    }
}