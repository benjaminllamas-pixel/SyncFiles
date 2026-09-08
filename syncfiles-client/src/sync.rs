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

pub struct SyncEngine {
    config: Config,
    client: Arc<SyncClient>,
    store: Arc<Mutex<MetadataStore>>,
}

impl SyncEngine {
    pub fn new(config: Config, client: Arc<SyncClient>, store: Arc<Mutex<MetadataStore>>) -> Self {
        Self { config, client, store }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Motor de sincronización iniciado");
        self.resume_queued_ops().await?;

        loop {
            match self.sync_cycle().await {
                Ok(_) => {
                    info!("Ciclo de sincronización completado");
                }
                Err(e) => {
                    error!("Error en ciclo de sincronización: {}", e);
                }
            }
            info!("Próximo poll en {} segundos", self.config.polling_interval_secs);
            sleep(Duration::from_secs(self.config.polling_interval_secs)).await;
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
            match change.operation.as_str() {
                "upload" => {
                    if let Some(content) = &change.content {
                        self.apply_download(&change, content).await?;
                    }
                }
                "delete" => {
                    self.apply_delete(&change).await?;
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
            let content_b64 = BASE64.encode(&content_bytes);
            let payload = UploadRequest {
                session_id: session.session_id.clone(),
                device_id: self.config.device_id.clone(),
                file_id: file.file_id.clone(),
                relative_path: file.relative_path.clone(),
                path_hash: file.path_hash.clone(),
                checksum: file.checksum.clone(),
                size_bytes: file.size_bytes,
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
                            checksum: file.checksum.clone(),
                            size_bytes: file.size_bytes,
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
                    error!("Error subiendo {}: {}", file.relative_path, e);
                    let store = self.store.lock().unwrap();
                    store.update_queue_status(&queue_id, "retry", Some(&e.to_string()))?;
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

    async fn resume_queued_ops(&self) -> Result<()> {
        let queued = {
            let store = self.store.lock().unwrap();
            store.get_queued_ops()?
        };
        if !queued.is_empty() {
            info!("Reanudando {} operaciones en cola", queued.len());
            let store = self.store.lock().unwrap();
            for op in queued {
                store.update_queue_status(&op.queue_id, "queued", None)?;
            }
        }
        Ok(())
    }
}