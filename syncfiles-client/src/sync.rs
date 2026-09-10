use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::Utc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn, error};
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::network::SyncClient;
use crate::metadata::MetadataStore;
use syncfiles_models::{FileEntry, ChangeEntry, UploadRequest};

static WAKEUP_RX: std::sync::OnceLock<std::sync::Mutex<Option<std::sync::mpsc::Receiver<()>>>> = std::sync::OnceLock::new();

/// Estado compartido con la UI a través de un archivo JSON en data_dir.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct EngineStatus {
    pub last_cycle_ok: bool,
    pub last_cycle_at: i64,
    pub paused: bool,
}

fn write_engine_status(ok: bool, paused: bool) {
    let status = EngineStatus {
        last_cycle_ok: ok,
        last_cycle_at: chrono::Utc::now().timestamp_millis(),
        paused,
    };
    let dir = Config::data_dir();
    if let Ok(content) = serde_json::to_string(&status) {
        let _ = std::fs::write(dir.join("engine_status.json"), content);
    }
}

pub struct SyncEngine {
    config: Config,
    client: Arc<SyncClient>,
    store: Arc<Mutex<MetadataStore>>,
    paused: std::sync::atomic::AtomicBool,
    wakeup_rx: tokio::sync::Mutex<Option<tokio::sync::mpsc::Receiver<()>>>,
}

impl SyncEngine {
    pub fn new(config: Config, client: Arc<SyncClient>, store: Arc<Mutex<MetadataStore>>) -> Self {
        Self {
            config,
            client,
            store,
            paused: std::sync::atomic::AtomicBool::new(false),
            wakeup_rx: tokio::sync::Mutex::new(None),
        }
    }

    /// Canal para que la UI despierte el ciclo de sync inmediatamente.
    pub fn wakeup_channel() -> Arc<std::sync::mpsc::Sender<()>> {
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        // Guardamos el receiver en un static para que el motor lo tome.
        WAKEUP_RX.get_or_init(|| std::sync::Mutex::new(Some(rx)));
        Arc::new(tx)
    }

    pub fn set_wakeup(&self, _tx: Arc<std::sync::mpsc::Sender<()>>) {
        // El receiver está en el global WAKEUP_RX; nada que hacer aquí.
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, std::sync::atomic::Ordering::Relaxed);
    }

    pub async fn run(&self) -> Result<()> {
        info!("Motor de sincronización iniciado");

        loop {
            if self.paused.load(std::sync::atomic::Ordering::Relaxed) {
                info!("Sincronización pausada; esperando...");
                sleep(Duration::from_secs(1)).await;
                continue;
            }
            match self.sync_cycle().await {
                Ok(_) => {
                    info!("Ciclo de sincronización completado");
                    write_engine_status(true, self.paused.load(std::sync::atomic::Ordering::Relaxed));
                }
                Err(e) => {
                    error!("Error en ciclo de sincronización: {}", e);
                    write_engine_status(false, self.paused.load(std::sync::atomic::Ordering::Relaxed));
                }
            }
            info!("Próximo poll en {} segundos", self.config.polling_interval_secs);
            // Espera interrumpible: "Sincronizar ahora" acorta la espera
            let mut waited = 0u64;
            while waited < self.config.polling_interval_secs * 1000 {
                if self.paused.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                if let Some(rx) = WAKEUP_RX.get() {
                    if let Ok(mut guard) = rx.try_lock() {
                        if let Some(receiver) = guard.as_mut() {
                            if receiver.try_recv().is_ok() {
                                info!("Wakeup recibido; sincronizando ya");
                                break;
                            }
                        }
                    }
                }
                sleep(Duration::from_millis(100)).await;
                waited += 100;
            }
        }
    }

    async fn sync_cycle(&self) -> Result<()> {
        let session_rec = {
            let store = self.store.lock().unwrap();
            store.get_active_session()?.expect("No hay sesión activa")
        };
        let session = crate::metadata::Session {
            session_id: session_rec.session_id.clone(),
            user_id: session_rec.user_id.clone(),
            device_id: session_rec.device_id.clone(),
            expires_at: session_rec.expires_at,
            token_hash: session_rec.token_hash,
        };

        info!("Reprocesando cola persistente...");
        self.retry_queued_ops(&session).await?;

        info!("Pull remoto (polling)...");
        self.pull_remote(&session).await?;

        info!("Push de cambios locales...");
        self.push_local(&session).await?;

        Ok(())
    }

    async fn pull_remote(&self, session: &crate::metadata::Session) -> Result<()> {
        let last_seq = {
            let store = self.store.lock().unwrap();
            store.get_last_server_seq()?
        };
        let diff = self.client.get_diff(&session.session_id, last_seq).await?;

        for change in &diff.changes {
            // Ignorar cambios originados por este dispositivo
            if change.device_id == self.config.device_id {
                continue;
            }
            match change.operation.as_str() {
                "upload" => {
                    if let Some(content) = &change.content {
                        self.apply_download(&change, content).await?;
                    } else {
                        self.download_remote_file(&session, change).await?;
                    }
                }
                "delete" => {
                    self.apply_delete(change).await?;
                }
                "rename" | "move" => {
                    self.apply_renaming(change).await?;
                }
                "copy" => {
                    // Copiar no invalida la ruta original; descargar el contenido
                    // destino exige conocer el nuevo path, que la diff no incluye.
                    // Se descarga por path_hash como fallback.
                    self.download_remote_file(&session, change).await?;
                }
                _ => {
                    info!("Operación remota no implementada: {}", change.operation);
                }
            }
        }

        {
            let store = self.store.lock().unwrap();
            store.set_last_server_seq(diff.server_seq)?;
        }
        info!("Pull completado: {} cambios, server_seq={}", diff.changes.len(), diff.server_seq);
        Ok(())
    }

    async fn download_remote_file(&self, session: &crate::metadata::Session, change: &ChangeEntry) -> Result<()> {
        let resp = self.client
            .download(&session.session_id, &change.file_id, &change.path_hash)
            .await?;
        self.apply_download(change, &resp.content).await
    }

    async fn apply_renaming(&self, change: &ChangeEntry) -> Result<()> {
        // El servidor registra renames actualizando relative_path; la diff no
        // incluye el path nuevo, así que se consulta el estado actual del archivo.
        let local_path_old = {
            let store = self.store.lock().unwrap();
            let existing = store.get_file_by_id(&change.file_id)?;
            existing.map(|f| f.relative_path)
        };

        let remote_relative = change.relative_path.clone().unwrap_or_default();
        if remote_relative.is_empty() {
            info!("Rename remoto sin ruta nueva conocida (file_id={}), ignorando", change.file_id);
            return Ok(());
        }

        let new_local = self.config.sync_root.join(&remote_relative);
        if let Some(old_relative) = local_path_old {
            let old_local = self.config.sync_root.join(&old_relative);
            if old_local.exists() && old_local != new_local {
                if let Some(parent) = new_local.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                if let Err(e) = std::fs::rename(&old_local, &new_local) {
                    warn!("Fallo renombrando local {:?} -> {:?}: {}", old_local, new_local, e);
                } else {
                    info!("Renombrado local: {:?} -> {:?}", old_local, new_local);
                }
            }
        }

        {
            let store = self.store.lock().unwrap();
            store.update_file_path(&change.file_id, &remote_relative, &change.path_hash)?;
        }
        Ok(())
    }

    async fn push_local(&self, session: &crate::metadata::Session) -> Result<()> {
        let pending = {
            let store = self.store.lock().unwrap();
            store.get_files_by_status("pending")?
        };

        for file in pending {
            if file.status == "deleted" {
                let _ = self.client.delete(&session.session_id, &file.file_id, &file.path_hash).await;
                let store = self.store.lock().unwrap();
                let _ = store.update_queue_status(&file.file_id, "done", None);
                continue;
            }

            let local_path = self.config.sync_root.join(&file.relative_path);
            if !local_path.exists() {
                warn!("Archivo no existe localmente: {:?}", local_path);
                continue;
            }

            let content_bytes = std::fs::read(&local_path).unwrap_or_default();
            let checksum = crate::metadata::MetadataStore::hash_file_at(&local_path)
                .unwrap_or_else(|_| file.checksum.clone());
            let content_b64 = BASE64.encode(&content_bytes);
            let payload = UploadRequest {
                session_id: session.session_id.clone(),
                device_id: self.config.device_id.clone(),
                file_id: file.file_id.clone(),
                relative_path: file.relative_path.clone(),
                path_hash: file.path_hash.clone(),
                checksum: checksum.clone(),
                size_bytes: content_bytes.len() as i64,
                modified_at: file.modified_at,
                idempotency_key: uuid::Uuid::new_v4().to_string(),
                content: content_b64,
            };

            let queue_id = uuid::Uuid::new_v4().to_string();
            {
                let store = self.store.lock().unwrap();
                store.queue_op(
                    &queue_id,
                    &file.file_id,
                    "upload",
                    &payload.idempotency_key,
                    Some(serde_json::to_string(&payload)?),
                )?;
            }

            match self.client.upload(&payload).await {
                Ok(resp) => {
                    if resp.accepted {
                        let store = self.store.lock().unwrap();
                        store.update_queue_status(&queue_id, "done", None)?;
                        store.upsert_file(&FileEntry {
                            file_id: file.file_id.clone(),
                            user_id: file.user_id.clone(),
                            device_id: file.device_id.clone(),
                            relative_path: file.relative_path.clone(),
                            path_hash: file.path_hash.clone(),
                            checksum: checksum.clone(),
                            size_bytes: content_bytes.len() as i64,
                            modified_at: file.modified_at,
                            synced_at: Some(Utc::now().timestamp_millis()),
                            status: "synced".to_string(),
                            last_sync_version: file.last_sync_version,
                            deleted_at: file.deleted_at,
                            content: None,
                        })?;
                        info!("Upload aceptado: {}", file.relative_path);
                    }
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.starts_with("CONFLICT:") {
                        let conflict_id = uuid::Uuid::new_v4().to_string();
                        let store = self.store.lock().unwrap();
                        store.mark_conflict(&conflict_id, &file.file_id, &file.checksum, &payload.checksum)?;
                        store.upsert_file(&FileEntry {
                            file_id: file.file_id.clone(),
                            user_id: file.user_id.clone(),
                            device_id: file.device_id.clone(),
                            relative_path: file.relative_path.clone(),
                            path_hash: file.path_hash.clone(),
                            checksum: payload.checksum.clone(),
                            size_bytes: content_bytes.len() as i64,
                            modified_at: file.modified_at,
                            synced_at: None,
                            status: "conflict".to_string(),
                            last_sync_version: file.last_sync_version,
                            deleted_at: file.deleted_at,
                            content: None,
                        })?;
                        store.update_queue_status(&queue_id, "retry", Some(&err_str))?;
                        warn!("Conflicto detectado en {}: {}", file.relative_path, err_str);
                    } else {
                        error!("Error subiendo {}: {}", file.relative_path, e);
                        let store = self.store.lock().unwrap();
                        store.update_queue_status(&queue_id, "retry", Some(&err_str))?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn apply_download(&self, change: &ChangeEntry, content: &str) -> Result<()> {
        let local_path = self.config.sync_root.join(change.relative_path.as_deref().unwrap_or(&change.path_hash));
        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content_bytes = BASE64.decode(content)?;
        std::fs::write(&local_path, content_bytes)?;

        {
            let store = self.store.lock().unwrap();
            store.upsert_file(&FileEntry {
                file_id: change.file_id.clone(),
                user_id: "local".to_string(),
                device_id: self.config.device_id.clone(),
                relative_path: change.relative_path.clone().unwrap_or_default(),
                path_hash: change.path_hash.clone(),
                checksum: change.checksum.clone(),
                size_bytes: change.size_bytes.unwrap_or(content.len() as i64),
                modified_at: change.modified_at,
                synced_at: Some(Utc::now().timestamp_millis()),
                status: "synced".to_string(),
                last_sync_version: 0,
                deleted_at: None,
                content: None,
            })?;
        }

        info!("Descargado: {:?}", local_path);
        Ok(())
    }

    async fn apply_delete(&self, change: &ChangeEntry) -> Result<()> {
        let local_path = self.config.sync_root.join(change.relative_path.as_deref().unwrap_or(&change.path_hash));
        if local_path.exists() {
            std::fs::remove_file(&local_path)?;
            info!("Borrado local: {:?}", local_path);
        }
        {
            let store = self.store.lock().unwrap();
            store.upsert_file(&FileEntry {
                file_id: change.file_id.clone(),
                user_id: "local".to_string(),
                device_id: self.config.device_id.clone(),
                relative_path: change.relative_path.clone().unwrap_or_default(),
                path_hash: change.path_hash.clone(),
                checksum: change.checksum.clone(),
                size_bytes: 0,
                modified_at: change.modified_at,
                synced_at: Some(Utc::now().timestamp_millis()),
                status: "deleted".to_string(),
                last_sync_version: 0,
                deleted_at: Some(Utc::now().timestamp_millis()),
                content: None,
            })?;
        }
        Ok(())
    }

    /// Reprocesa operaciones persistidas en la cola SQLite (supervivencia a reinicios).
    async fn retry_queued_ops(&self, session: &crate::metadata::Session) -> Result<()> {
        let queued = {
            let store = self.store.lock().unwrap();
            store.get_queued_ops()?
        };
        if queued.is_empty() {
            return Ok(());
        }
        info!("Reanudando {} operaciones en cola", queued.len());
        for op in queued {
            // Solo reprocesar payloads upload persistidos; los 'pending' del
            // watcher sin payload se manejan via push_local.
            let Some(payload_json) = &op.payload_json else {
                continue;
            };
            let Ok(payload) = serde_json::from_str::<UploadRequest>(payload_json) else {
                let store = self.store.lock().unwrap();
                let _ = store.update_queue_status(&op.queue_id, "done", Some("payload inválido"));
                continue;
            };
            {
                let store = self.store.lock().unwrap();
                store.update_queue_status(&op.queue_id, "in_progress", None)?;
            }
            match self.client.upload(&payload).await {
                Ok(resp) => {
                    if resp.accepted {
                        let store = self.store.lock().unwrap();
                        store.update_queue_status(&op.queue_id, "done", None)?;
                        store.upsert_file(&FileEntry {
                            file_id: payload.file_id.clone(),
                            user_id: session.user_id.clone(),
                            device_id: payload.device_id.clone(),
                            relative_path: payload.relative_path.clone(),
                            path_hash: payload.path_hash.clone(),
                            checksum: payload.checksum.clone(),
                            size_bytes: payload.size_bytes,
                            modified_at: payload.modified_at,
                            synced_at: Some(Utc::now().timestamp_millis()),
                            status: "synced".to_string(),
                            last_sync_version: 0,
                            deleted_at: None,
                            content: None,
                        })?;
                    }
                }
                Err(e) => {
                    let err_str = e.to_string();
                    warn!("Reintento fallido ({}): {}", op.operation, err_str);
                    let store = self.store.lock().unwrap();
                    store.increment_attempts(&op.queue_id)?;
                    let next_status = if err_str.starts_with("CONFLICT:") { "conflict" } else { "retry" };
                    store.update_queue_status(&op.queue_id, next_status, Some(&err_str))?;
                }
            }
        }
        Ok(())
    }
}