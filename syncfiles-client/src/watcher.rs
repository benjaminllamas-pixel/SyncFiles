use anyhow::Result;
use notify::{Watcher, RecommendedWatcher, RecursiveMode, Event};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
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
        let canonical_root = std::fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone());
        let root_str = canonical_root.to_str().unwrap_or_default().to_string();
        let root = self.root.clone();
        let debounce_state = Arc::new(Mutex::new(std::collections::HashMap::<String, Instant>::new()));
        let debounce_state_inner = debounce_state.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        for path in &event.paths {
                            if let Some(path_str) = path.to_str() {
                                let canonical_path = std::fs::canonicalize(path)
                                    .map(|p| p.to_string_lossy().into_owned())
                                    .unwrap_or_else(|_| path_str.to_string());
                                if !canonical_path.starts_with(&root_str) {
                                    continue;
                                }
                                let now = Instant::now();
                                let mut map = debounce_state_inner.lock().unwrap();
                                let last = map.get(path_str).copied();
                                if last.map(|t| now.duration_since(t) < Duration::from_secs(2)).unwrap_or(false) {
                                    continue;
                                }
                                map.insert(path_str.to_string(), now);
                                drop(map);
                                info!("Cambio detectado: {}", path_str);
                                let relative = canonical_path
                                    .strip_prefix(&root_str)
                                    .map(|p| p.trim_start_matches('/').to_string())
                                    .unwrap_or(canonical_path.clone());
                                (on_change)(relative);
                            }
                        }
                    }
                    Err(e) => info!("Watch error: {}", e),
                }
            },
            notify::Config::default().with_poll_interval(Duration::from_secs(2)),
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
