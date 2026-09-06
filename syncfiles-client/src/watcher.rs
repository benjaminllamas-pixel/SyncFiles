use anyhow::Result;
use notify::{Watcher, RecommendedWatcher, RecursiveMode, Event};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;
use sha2::{Sha256, Digest};

pub struct FileWatcher {
    root: PathBuf,
    on_change: Arc<dyn Fn(String) + Send + Sync>,
}

impl FileWatcher {
    pub fn new(root: PathBuf, on_change: Arc<dyn Fn(String) + Send + Sync>) -> Self {
        Self { root, on_change }
    }

    pub fn start(&self) -> Result<RecommendedWatcher> {
        let on_change = self.on_change.clone();
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        for path in &event.paths {
                            if let Some(path_str) = path.to_str() {
                                info!("Cambio detectado: {}", path_str);
                                (on_change)(path_str.to_string());
                            }
                        }
                    }
                    Err(e) => info!("Watch error: {}", e),
                }
            },
            notify::Config::default().with_poll_interval(Duration::from_secs(1)),
        )?;

        watcher.watch(&self.root, RecursiveMode::Recursive)?;
        info!("File watcher iniciado en: {:?}", self.root);
        Ok(watcher)
    }
}

pub fn compute_checksum(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

pub fn compute_path_hash(path: &str) -> String {
    let normalized = path.trim_start_matches('/');
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn get_file_mod_time(path: &std::path::Path) -> Result<i64> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?.duration_since(std::time::SystemTime::UNIX_EPOCH)?.as_millis() as i64;
    Ok(modified)
}