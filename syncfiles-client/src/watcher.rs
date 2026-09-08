use anyhow::Result;
use notify::{Watcher, RecommendedWatcher, RecursiveMode, Event};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

use syncfiles_models::{compute_checksum, compute_path_hash};

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

pub fn get_file_mod_time(path: &std::path::Path) -> Result<i64> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?.duration_since(std::time::SystemTime::UNIX_EPOCH)?.as_millis() as i64;
    Ok(modified)
}