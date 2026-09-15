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

#[cfg(test)]
mod tests {
    use super::*;

    fn isolated_data_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sf-config-test-{}-{}", name, std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    struct EnvGuard(std::sync::Mutex<Vec<&'static str>>);
    impl EnvGuard {
        #[allow(dead_code)]
        fn set(keys: Vec<&'static str>) -> Self {
            for k in &keys {
                unsafe { std::env::set_var(k, "") };
            }
            EnvGuard(std::sync::Mutex::new(keys))
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            let keys = self.0.lock().unwrap().clone();
            for k in &keys {
                unsafe { std::env::remove_var(k) };
            }
        }
    }

    #[test]
    fn config_save_load_roundtrip() {
        let dir = isolated_data_dir("roundtrip");
        unsafe { std::env::set_var("SF_DATA_DIR", &dir) };
        let cfg = Config {
            server_url: "http://example.com:9000".into(),
            email: "user@example.com".into(),
            password: "secreto".into(),
            device_id: "dev-42".into(),
            sync_root: dir.join("sync"),
            polling_interval_secs: 15,
        };
        cfg.save().unwrap();

        // load() lee del config.json persistido (ignora env defaults)
        let loaded = Config::load().unwrap();
        assert_eq!(loaded.server_url, cfg.server_url);
        assert_eq!(loaded.email, cfg.email);
        assert_eq!(loaded.password, cfg.password);
        assert_eq!(loaded.device_id, cfg.device_id);
        assert_eq!(loaded.polling_interval_secs, 15);
        assert_eq!(loaded.sync_root, dir.join("sync"));

        // data_dir respeta SF_DATA_DIR
        assert_eq!(Config::data_dir(), dir);
        assert!(Config::config_path().starts_with(&dir));
        unsafe { std::env::remove_var("SF_DATA_DIR") };
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn config_load_defaults_when_no_file() {
        let dir = isolated_data_dir("defaults");
        unsafe { std::env::set_var("SF_DATA_DIR", &dir) };
        unsafe { std::env::remove_var("SF_CONFIG_EXISTS_MARKER") };
        // Sin config.json: usa defaults de env
        unsafe { std::env::set_var("SF_SERVER_URL", "http://default-url:1234") };
        unsafe { std::env::set_var("SF_POLLING_INTERVAL", "7") };
        let cfg = Config::load().unwrap();
        assert_eq!(cfg.server_url, "http://default-url:1234");
        assert_eq!(cfg.polling_interval_secs, 7);
        assert!(!cfg.device_id.is_empty());
        assert!(cfg.device_id.starts_with("desktop-"));

        unsafe { std::env::remove_var("SF_SERVER_URL") };
        unsafe { std::env::remove_var("SF_POLLING_INTERVAL") };
        unsafe { std::env::remove_var("SF_DATA_DIR") };
        std::fs::remove_dir_all(dir).ok();
    }
}
