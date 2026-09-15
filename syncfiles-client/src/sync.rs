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

        info!("Reconciliando carpeta local con la base de metadatos...");
        self.reconcile_local_folder().await;

        info!("Reprocesando cola persistente...");
        self.retry_queued_ops(&session).await?;

        info!("Pull remoto (polling)...");
        self.pull_remote(&session).await?;

        info!("Push de cambios locales...");
        self.push_local(&session).await?;

        Ok(())
    }

    /// Escanea la carpeta local y:
    /// - encola como 'pending' los archivos nuevos o con checksum cambiado;
    /// - marca como 'deleted' las filas synced/pending cuyo archivo ya no está
    ///   en disco (propagación de deletes hechos fuera de la UI, p.ej. Finder).
    ///   Los archivos cuyo último segmento empieza con '.' se ignoran (scan
    ///   no los ve; evita borrar dotfiles y descargas .conflict).
    ///   No toca filas 'conflict' ni 'deleted'.
    async fn reconcile_local_folder(&self) {
        let mut disk_files = std::collections::HashMap::new();
        if let Err(e) = scan_files(&self.config.sync_root, &mut disk_files) {
            warn!("Error escaneando {:?}: {}", self.config.sync_root, e);
            return;
        }

        let known: Vec<FileEntry> = {
            let store = self.store.lock().unwrap();
            let mut acc = Vec::new();
            for status in ["synced", "pending"] {
                match store.get_files_by_status(status) {
                    Ok(mut entries) => acc.append(&mut entries),
                    Err(e) => {
                        warn!("Error leyendo archivos sincronizados: {}", e);
                        return;
                    }
                }
            }
            acc
        };

        for (relative, checksum) in &disk_files {
            let existing = known.iter().find(|f| f.relative_path == *relative);
            match existing {
                None => {
                    // Archivo nuevo (cliente apagado o primera ejecución)
                    let store = self.store.lock().unwrap();
                    if store.get_file_by_relative_path(relative).ok().flatten().is_none() {
                        if let Err(e) = store.enqueue(relative, "pending", None) {
                            warn!("Error encolando nuevo archivo {}: {}", relative, e);
                        } else {
                            info!("Archivo local nuevo detectado por escaneo: {}", relative);
                        }
                    }
                }
                Some(entry) => {
                    if entry.checksum != *checksum {
                        // Contenido cambió desde la última sincronización
                        let store = self.store.lock().unwrap();
                        if let Ok(Some(_)) = store.get_file_by_relative_path(relative) {
                            let _ = store.update_file_status(relative, "pending");
                        }
                        info!("Cambio local detectado por escaneo: {}", relative);
                    }
                }
            }
        }

        // Deletes locales: filas synced/pending sin archivo en disco
        for entry in &known {
            if disk_files.contains_key(&entry.relative_path) {
                continue;
            }
            let last_segment = entry.relative_path.rsplit('/').next().unwrap_or("");
            if last_segment.starts_with('.') {
                continue;
            }
            let store = self.store.lock().unwrap();
            match store.update_file_status(&entry.relative_path, "deleted") {
                Ok(_) => info!("Delete local detectado por escaneo: {}", entry.relative_path),
                Err(e) => warn!("Error marcando delete local {}: {}", entry.relative_path, e),
            }
        }
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
        // Prioridad: ruta local conocida por file_id; fallback a old_path del
        // diff (p.ej. replay tras reinstalación cuando la fila local no existe).
        let local_path_old = {
            let store = self.store.lock().unwrap();
            let existing = store.get_file_by_id(&change.file_id)?;
            existing.map(|f| f.relative_path)
        }
        .or_else(|| change.old_path.clone());

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
        // Deletes pendientes de propagación (detectados por reconcile o la UI)
        let deleted = {
            let store = self.store.lock().unwrap();
            store.get_deleted_unsynced()?
        };
        for file in deleted {
            // Si el archivo reapareció en disco, volver a pending (subirá de nuevo)
            let local_path = self.config.sync_root.join(&file.relative_path);
            if local_path.exists() {
                let store = self.store.lock().unwrap();
                let _ = store.restore_to_pending(&file.file_id);
                info!("Archivo reapareció en disco; vuelve a pending: {}", file.relative_path);
                continue;
            }

            match self.client.delete(&session.session_id, &file.file_id, &file.path_hash).await {
                Ok(_) => {
                    let store = self.store.lock().unwrap();
                    store.mark_delete_synced(&file.file_id)?;
                    info!("Delete propagado al server: {}", file.relative_path);
                }
                Err(e) => {
                    // NOT_FOUND = ya inexistente server-side; cuenta como éxito
                    let err_str = e.to_string();
                    if err_str.contains("NOT_FOUND") {
                        let store = self.store.lock().unwrap();
                        store.mark_delete_synced(&file.file_id)?;
                        info!("Delete ya aplicado server-side (NOT_FOUND): {}", file.relative_path);
                    } else {
                        warn!("Error propagando delete de {}: {}", file.relative_path, err_str);
                    }
                }
            }
        }

        let pending = {
            let store = self.store.lock().unwrap();
            store.get_files_by_status("pending")?
        };

        for file in pending {
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
                // Sin payload no hay nada que reprocesar; cerrar para no volver a verla
                let store = self.store.lock().unwrap();
                let _ = store.update_queue_status(&op.queue_id, "done", Some("sin payload"));
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

/// Escanea recursivamente un directorio devolviendo ruta relativa → checksum.
/// Ignora archivos ocultos y de metadatos (`.DS_Store`, `.conflict_*` no: los
/// conflictos preservados SÍ se sincronizan como archivos normales).
fn scan_files(root: &std::path::Path, out: &mut std::collections::HashMap<String, String>) -> anyhow::Result<()> {
    scan_files_rel(root, "", out)
}

/// Recursión interna: acumula el prefijo relativo para que las claves de
/// subdirectorios incluyan su carpeta (`sub/b.txt`, no `b.txt`).
fn scan_files_rel(
    dir: &std::path::Path,
    prefix: &str,
    out: &mut std::collections::HashMap<String, String>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let relative = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };
        if path.is_dir() {
            scan_files_rel(&path, &relative, out)?;
        } else if path.is_file() {
            let checksum = crate::metadata::MetadataStore::hash_file_at(&path).unwrap_or_default();
            out.insert(relative, checksum);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::MetadataStore;
    use syncfiles_models::compute_checksum;

    fn tmp_dir(name: &str) -> std::path::PathBuf {
        let tmp = std::env::temp_dir().join(format!("sf-sync-test-{}-{}", name, std::process::id()));
        std::fs::remove_dir_all(&tmp).ok();
        std::fs::create_dir_all(&tmp).unwrap();
        tmp
    }

    fn test_config(root: &std::path::Path) -> Config {
        Config {
            server_url: "http://127.0.0.1:8080".into(),
            email: "test@syncfiles.local".into(),
            password: "secret".into(),
            device_id: "device-test".into(),
            sync_root: root.to_path_buf(),
            polling_interval_secs: 30,
        }
    }

    fn engine_with(root: &std::path::Path) -> (SyncEngine, std::path::PathBuf) {
        let config = test_config(root);
        let client = Arc::new(SyncClient::new("http://127.0.0.1:1", "device-test").unwrap());
        let db_path = root.join(".sf-test").join("syncfiles.db");
        let store = MetadataStore::with_path(config.clone(), &db_path).unwrap();
        let engine = SyncEngine::new(config, client, Arc::new(Mutex::new(store)));
        (engine, root.to_path_buf())
    }

    #[test]
    fn scan_files_maps_relative_paths_and_checksums() {
        let tmp = tmp_dir(&format!("scan-{}", uuid::Uuid::new_v4()));
        std::fs::write(tmp.join("a.txt"), b"uno").unwrap();
        std::fs::create_dir(tmp.join("sub")).unwrap();
        std::fs::write(tmp.join("sub").join("b.txt"), b"dos").unwrap();
        std::fs::write(tmp.join(".hidden"), b"ignorame").unwrap();

        let mut out = std::collections::HashMap::new();
        scan_files(&tmp, &mut out).unwrap();
        assert_eq!(out.len(), 2, "los dotfiles se ignoran: {:?}", out.keys());
        assert_eq!(out.get("a.txt").unwrap(), &compute_checksum(b"uno"));
        let sub_key = out.keys().find(|k| k.ends_with("b.txt")).expect("b.txt escaneado");
        assert_eq!(out.get(sub_key).unwrap(), &compute_checksum(b"dos"));
        assert!(sub_key.contains("sub"));
        std::fs::remove_dir_all(tmp).ok();
    }

    #[tokio::test]
    async fn reconcile_detects_new_modified_and_deleted() {
        let tmp = tmp_dir("reconcile");
        std::fs::write(tmp.join("nuevo.txt"), b"contenido nuevo").unwrap();
        std::fs::write(tmp.join("cambiado.txt"), b"v2").unwrap();
        std::fs::write(tmp.join("borrado.txt"), b"chau").unwrap();

        let (engine, _) = engine_with(&tmp);
        // Estado inicial: 'cambiado.txt' synced con checksum viejo,
        // 'borrado.txt' synced (después lo eliminamos del disco).
        {
            let store = engine.store.lock().unwrap();
            store.upsert_file(&FileEntry {
                file_id: "f-cambiado".into(),
                user_id: "u".into(),
                device_id: "device-test".into(),
                relative_path: "cambiado.txt".into(),
                path_hash: syncfiles_models::compute_path_hash("cambiado.txt"),
                checksum: compute_checksum(b"v1"),
                size_bytes: 2,
                modified_at: 1,
                synced_at: Some(1),
                status: "synced".into(),
                last_sync_version: 0,
                deleted_at: None,
                content: None,
            }).unwrap();
            store.upsert_file(&FileEntry {
                file_id: "f-borrado".into(),
                user_id: "u".into(),
                device_id: "device-test".into(),
                relative_path: "borrado.txt".into(),
                path_hash: syncfiles_models::compute_path_hash("borrado.txt"),
                checksum: compute_checksum(b"chau"),
                size_bytes: 4,
                modified_at: 1,
                synced_at: Some(1),
                status: "synced".into(),
                last_sync_version: 0,
                deleted_at: None,
                content: None,
            }).unwrap();
        }

        // Borrar del disco ANTES del reconcile → debe detectarse como delete local
        std::fs::remove_file(tmp.join("borrado.txt")).unwrap();

        engine.reconcile_local_folder().await;

        {
            let store = engine.store.lock().unwrap();
            // Nuevo archivo detectado → pending
            let nuevo = store.get_file_by_relative_path("nuevo.txt").unwrap().unwrap();
            assert_eq!(nuevo.status, "pending");
            // Checksum cambió → vuelve a pending
            let cambiado = store.get_file_by_relative_path("cambiado.txt").unwrap().unwrap();
            assert_eq!(cambiado.status, "pending");
            // Archivo desaparecido → deleted
            let borrado = store.get_file_by_relative_path("borrado.txt").unwrap().unwrap();
            assert_eq!(borrado.status, "deleted");
            // Dotfiles no se marcan deleted aunque no estén en el scan
        }
        std::fs::remove_dir_all(tmp).ok();
    }

    #[tokio::test]
    async fn reconcile_ignores_dotfiles_for_deletes() {
        let tmp = tmp_dir("reconcile-dots");
        let (engine, _) = engine_with(&tmp);
        {
            let store = engine.store.lock().unwrap();
            store.upsert_file(&FileEntry {
                file_id: "f-conf".into(),
                user_id: "u".into(),
                device_id: "device-test".into(),
                relative_path: ".conflict_preservado.txt".into(),
                path_hash: syncfiles_models::compute_path_hash(".conflict_preservado.txt"),
                checksum: compute_checksum(b"x"),
                size_bytes: 1,
                modified_at: 1,
                synced_at: Some(1),
                status: "synced".into(),
                last_sync_version: 0,
                deleted_at: None,
                content: None,
            }).unwrap();
        }
        // El archivo existe pero empieza con '.' → el scan no lo ve y NO debe marcarse deleted
        std::fs::write(tmp.join(".conflict_preservado.txt"), b"x").unwrap();
        engine.reconcile_local_folder().await;
        {
            let store = engine.store.lock().unwrap();
            let entry = store.get_file_by_relative_path(".conflict_preservado.txt").unwrap().unwrap();
            assert_eq!(entry.status, "synced");
        }
        std::fs::remove_dir_all(tmp).ok();
    }

    #[tokio::test]
    async fn apply_renaming_uses_old_path_fallback() {
        let tmp = tmp_dir(&format!("rename-fallback-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(tmp.join("docs")).unwrap();
        std::fs::write(tmp.join("docs").join("viejo.txt"), b"contenido").unwrap();
        let (engine, _) = engine_with(&tmp);

        // Sin fila local (replay tras reinstalación): old_path del diff es el fallback
        let change = ChangeEntry {
            operation: "rename".into(),
            file_id: "f-rename".into(),
            path_hash: syncfiles_models::compute_path_hash("docs/nuevo.txt"),
            relative_path: Some("docs/nuevo.txt".into()),
            old_path: Some("docs/viejo.txt".into()),
            checksum: compute_checksum(b"contenido"),
            size_bytes: Some(8),
            modified_at: 1,
            device_id: "otro-device".into(),
            content: None,
        };
        engine.apply_renaming(&change).await.unwrap();

        // El archivo físico se movió usando old_path (no había fila local)
        assert!(!tmp.join("docs").join("viejo.txt").exists());
        assert!(tmp.join("docs").join("nuevo.txt").exists());

        // Con fila local existente: la ruta se actualiza en la DB
        {
            let store = engine.store.lock().unwrap();
            store.upsert_file(&FileEntry {
                file_id: "f-rename-2".into(),
                user_id: "u".into(),
                device_id: "otro-device".into(),
                relative_path: "docs/viejo2.txt".into(),
                path_hash: syncfiles_models::compute_path_hash("docs/viejo2.txt"),
                checksum: compute_checksum(b"contenido"),
                size_bytes: 8,
                modified_at: 1,
                synced_at: Some(1),
                status: "synced".into(),
                last_sync_version: 0,
                deleted_at: None,
                content: None,
            }).unwrap();
        }
        std::fs::write(tmp.join("docs").join("viejo2.txt"), b"contenido").unwrap();
        let change2 = ChangeEntry {
            operation: "rename".into(),
            file_id: "f-rename-2".into(),
            path_hash: syncfiles_models::compute_path_hash("docs/nuevo2.txt"),
            relative_path: Some("docs/nuevo2.txt".into()),
            old_path: Some("docs/viejo2.txt".into()),
            checksum: compute_checksum(b"contenido"),
            size_bytes: Some(8),
            modified_at: 1,
            device_id: "otro-device".into(),
            content: None,
        };
        engine.apply_renaming(&change2).await.unwrap();
        assert!(tmp.join("docs").join("nuevo2.txt").exists());
        let renamed = {
            let store = engine.store.lock().unwrap();
            store.get_file_by_id("f-rename-2").unwrap().unwrap()
        };
        assert_eq!(renamed.relative_path, "docs/nuevo2.txt");
        std::fs::remove_dir_all(tmp).ok();
    }

    #[tokio::test]
    async fn apply_download_writes_file_and_upserts_metadata() {
        let tmp = tmp_dir("apply-download");
        let (engine, _) = engine_with(&tmp);
        let content_b64 = base64::engine::general_purpose::STANDARD.encode(b"contenido bajo");
        let change = ChangeEntry {
            operation: "upload".into(),
            file_id: "f-dl".into(),
            path_hash: syncfiles_models::compute_path_hash("carpeta/descargado.txt"),
            relative_path: Some("carpeta/descargado.txt".into()),
            old_path: None,
            checksum: compute_checksum(b"contenido bajo"),
            size_bytes: Some(13),
            modified_at: 1,
            device_id: "otro-device".into(),
            content: Some(content_b64),
        };
        engine.apply_download(&change, change.content.as_deref().unwrap()).await.unwrap();

        let written = std::fs::read(tmp.join("carpeta").join("descargado.txt")).unwrap();
        assert_eq!(written, b"contenido bajo");
        let entry = {
            let store = engine.store.lock().unwrap();
            store.get_file_by_id("f-dl").unwrap().unwrap()
        };
        assert_eq!(entry.status, "synced");
        assert_eq!(entry.checksum, compute_checksum(b"contenido bajo"));
        std::fs::remove_dir_all(tmp).ok();
    }

    #[tokio::test]
    async fn apply_delete_removes_local_file() {
        let tmp = tmp_dir("apply-delete");
        std::fs::write(tmp.join("muere.txt"), b"x").unwrap();
        let (engine, _) = engine_with(&tmp);
        let change = ChangeEntry {
            operation: "delete".into(),
            file_id: "f-del".into(),
            path_hash: syncfiles_models::compute_path_hash("muere.txt"),
            relative_path: Some("muere.txt".into()),
            old_path: None,
            checksum: compute_checksum(b"x"),
            size_bytes: Some(1),
            modified_at: 1,
            device_id: "otro-device".into(),
            content: None,
        };
        engine.apply_delete(&change).await.unwrap();
        assert!(!tmp.join("muere.txt").exists());
        let entry = {
            let store = engine.store.lock().unwrap();
            store.get_file_by_id("f-del").unwrap().unwrap()
        };
        assert_eq!(entry.status, "deleted");
        assert!(entry.deleted_at.is_some());
        std::fs::remove_dir_all(tmp).ok();
    }
}