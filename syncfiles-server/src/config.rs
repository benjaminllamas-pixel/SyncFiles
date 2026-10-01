use anyhow::{Context, Result};
use dotenvy::dotenv;
use std::net::SocketAddr;
use std::path::Path;

/// Directorio base contra el que se resuelven las rutas relativas de configuración.
///
/// Antes se resolvía contra el directorio del binario (`current_exe()`), lo que producía
/// una base de datos distinta por cada forma de lanzar el servidor (`cargo run`, el binario
/// en `target/debug`, el release, Docker) y hacía que los archivos "desaparecieran".
/// Ahora se resuelve contra el directorio de trabajo, que es el comportamiento estándar y
/// estable; las rutas absolutas en `SF_DATABASE_URL` / `SF_STORAGE_ROOT` se respetan tal cual.
fn base_dir() -> std::path::PathBuf {
    std::env::current_dir().unwrap_or_else(|_| {
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.to_path_buf()))
            .unwrap_or_default()
    })
}

fn resolve_abs(path: &str) -> String {
    let p = Path::new(path);
    if p.is_absolute() {
        return path.to_string();
    }
    base_dir().join(p).to_string_lossy().to_string()
}

fn resolve_db_url(url: &str) -> String {
    if let Some(path_part) = url.strip_prefix("sqlite:") {
        if path_part == ":memory:" {
            return url.to_string();
        }
        let path_part = path_part.trim_start_matches("./");
        if Path::new(path_part).is_absolute() {
            return url.to_string();
        }
        let abs = base_dir().join(path_part);
        return format!("sqlite:{}", abs.to_string_lossy());
    }
    url.to_string()
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_address: SocketAddr,
    pub database_url: String,
    pub server_url: String,
    pub storage_root: String,
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

        let bind_str =
            std::env::var("SF_BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
        let bind_address: SocketAddr = bind_str
            .parse()
            .context("SF_BIND_ADDRESS no es una dirección válida")?;

        let database_url = resolve_db_url(
            &std::env::var("SF_DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:data/syncfiles.db".to_string()),
        );

        let server_url =
            std::env::var("SF_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

        // Consistente con el default de SF_DATABASE_URL ("sqlite:data/syncfiles.db"):
        // ambos relativos al directorio del binario para que DB y storage compartan
        // la misma raiz "data/". Cambiar CWD no corrompe la BD existente.
        let storage_root = resolve_abs(
            &std::env::var("SF_STORAGE_ROOT").unwrap_or_else(|_| "data/storage".to_string()),
        );

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
            storage_root,
            users,
        })
    }
}
