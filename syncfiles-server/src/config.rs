use anyhow::{Context, Result};
use dotenvy::dotenv;
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_address: SocketAddr,
    pub database_url: String,
    pub server_url: String,
    pub users: Vec<UserConfig>,
}

#[derive(Debug, Clone)]
pub struct UserConfig {
    pub email: String,
    pub password_hash: String,
    pub user_id: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        dotenv().ok();

        let bind_str = std::env::var("SF_BIND_ADDRESS")
            .unwrap_or_else(|_| "127.0.0.1:8080".to_string());
        let bind_address: SocketAddr = bind_str.parse()
            .context("SF_BIND_ADDRESS no es una dirección válida")?;

        let database_url = std::env::var("SF_DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:data/syncfiles.db".to_string());

        let server_url = std::env::var("SF_SERVER_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

        let mut users = Vec::new();
        let mut i = 0;
        loop {
            let email = std::env::var(format!("SF_USER_{}_EMAIL", i)).ok();
            let pwd = std::env::var(format!("SF_USER_{}_PASSWORD", i)).ok();
            let uid = std::env::var(format!("SF_USER_{}_ID", i)).ok();
            if let (Some(e), Some(p), Some(u)) = (email, pwd, uid) {
                let hash = bcrypt::hash(p, 12)?;
                users.push(UserConfig {
                    email: e,
                    password_hash: hash,
                    user_id: u,
                });
            } else {
                break;
            }
            i += 1;
        }

        if users.is_empty() {
            let hash = bcrypt::hash("syncfiles", 12)?;
            users.push(UserConfig {
                email: "admin@syncfiles.local".to_string(),
                password_hash: hash,
                user_id: "user-001".to_string(),
            });
            tracing::info!("Cuenta demo creada: admin@syncfiles.local / syncfiles");
        }

        Ok(Self {
            bind_address,
            database_url,
            server_url,
            users,
        })
    }
}