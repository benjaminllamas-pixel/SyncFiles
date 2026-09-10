pub mod config;
pub mod auth;
pub mod network;
pub mod metadata;
pub mod sync;
pub mod watcher;

use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use eframe::egui;

fn main() -> eframe::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Modo headless para pruebas E2E: SF_HEADLESS=1 ejecuta el SyncEngine sin GUI.
    if std::env::var("SF_HEADLESS").ok().as_deref() == Some("1") {
        headless_main();
        return Ok(());
    }

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = rt.enter();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([920.0, 620.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "SyncFiles",
        options,
        Box::new(|_cc| Ok(Box::new(SyncFilesUi::new()))),
    )
}

struct SyncFilesUi {
    config: crate::config::Config,
    metadata: Arc<std::sync::Mutex<crate::metadata::MetadataStore>>,
    sync_client: Arc<crate::network::SyncClient>,

    logged_in: bool,
    login_error: Option<String>,
    connecting: bool,
    login_result: Arc<std::sync::Mutex<Option<Result<(), String>>>>,

    login_email: String,
    login_password: String,
    login_server_url: String,

    connected: bool,
    syncing: bool,
    last_sync: Option<std::time::Instant>,
    paused: bool,
    sync_wakeup: Arc<std::sync::mpsc::Sender<()>>,

    watcher: Option<crate::watcher::FileWatcher>,
    watcher_handle: Option<notify::RecommendedWatcher>,

    files: Vec<UiFile>,
    queue_entries: Vec<crate::metadata::SyncQueueEntry>,
    conflicts: Vec<UiConflict>,
    devices: Vec<UiDevice>,
    activity: Vec<String>,

    view: View,
    mobile_navigation: bool,
    scan_error: Option<String>,

    notifications: Vec<Notification>,

    selected_file: Option<UiFile>,
    context_menu_file: Option<UiFile>,
    context_menu_pos: Option<egui::Pos2>,

    user_email: String,
    user_id: Option<String>,
    session_id: Option<String>,
    device_name: String,

    modal: Modal,

    dashboard_stats: Option<syncfiles_models::StorageStats>,
    stats_error: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum View {
    Dashboard,
    Files,
    Queue,
    Conflicts,
    Devices,
    Activity,
    Settings,
}

#[derive(Clone)]
struct UiFile {
    name: String,
    path: String,
    status: FileState,
    size: String,
    is_dir: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum FileState {
    Local,
    Synced,
    Pending,
    Conflict,
    Error,
    Deleted,
}

#[derive(Debug, Clone)]
struct UiConflict {
    conflict_id: String,
    file_id: String,
    path: String,
    local_checksum: String,
    remote_checksum: String,
    strategy: String,
    created_at: i64,
    resolved: bool,
}

#[derive(Debug, Clone)]
struct UiDevice {
    device_id: String,
    platform: String,
    device_name: Option<String>,
    last_seen_at: Option<i64>,
    status: String,
}

#[derive(Debug, Clone)]
struct Notification {
    id: uuid::Uuid,
    message: String,
    kind: NotificationKind,
    created_at: std::time::Instant,
}

#[derive(Debug, Clone, PartialEq)]
enum Modal {
    None,
    Rename { file_id: String, old_path: String, new_path: String },
    Move { file_id: String, old_path: String, new_path: String },
    Copy { file_id: String, source_path: String, destination_path: String },
    Delete { file_id: String, path: String },
    ConflictResolve { conflict_id: String, file_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum NotificationKind {
    Info,
    Success,
    Warning,
    Error,
}

impl SyncFilesUi {
    fn new() -> Self {
        let config = crate::config::Config::load().expect("Failed to load config");
        let sync_client = Arc::new(crate::network::SyncClient::new(&config.server_url, &config.device_id).expect("Failed to create client"));

        let metadata = Arc::new(std::sync::Mutex::new(crate::metadata::MetadataStore::new(config.clone()).expect("Failed to init metadata")));

        let login_server_url = config.server_url.clone();
        let login_email = config.email.clone();

        let mut ui = Self {
            config: config.clone(),
            metadata,
            sync_client,
            logged_in: false,
            login_error: None,
            connecting: false,
            login_result: Arc::new(std::sync::Mutex::new(None)),
            login_email,
            login_password: String::new(),
            login_server_url,
            connected: false,
            syncing: false,
            last_sync: None,
            paused: false,
            sync_wakeup: crate::sync::SyncEngine::wakeup_channel(),
            files: Vec::new(),
            queue_entries: Vec::new(),
            conflicts: Vec::new(),
            devices: Vec::new(),
            activity: vec!["Cliente iniciado".to_string()],
            view: View::Dashboard,
            mobile_navigation: false,
            scan_error: None,
            notifications: Vec::new(),
            selected_file: None,
            context_menu_file: None,
            context_menu_pos: None,
            user_email: config.email.clone(),
            user_id: None,
            session_id: None,
            device_name: "Mi dispositivo".to_string(),
            watcher: None,
            watcher_handle: None,
            modal: Modal::None,
            dashboard_stats: None,
            stats_error: None,
        };

        ui.try_auto_login();
        ui
    }

    fn try_auto_login(&mut self) {
        let metadata = self.metadata.lock().unwrap();
        if let Ok(Some(session)) = metadata.get_active_session() {
            self.user_id = Some(session.user_id.clone());
            self.session_id = Some(session.session_id.clone());
            self.logged_in = true;
            drop(metadata);
            self.add_activity("Sesion activa recuperada".to_string());
            self.notify(NotificationKind::Info, "Sesion activa recuperada");
            self.refresh_all();
            self.start_sync_engine();
            self.start_watcher();
        }
    }

    fn start_sync_engine(&self) {
        if self.session_id.is_none() {
            return;
        }
        let config = self.config.clone();
        let sync_client = self.sync_client.clone();
        let metadata = self.metadata.clone();
        let wakeup = self.sync_wakeup.clone();
        let paused = self.paused;
        std::thread::spawn(move || {
            let engine = crate::sync::SyncEngine::new(
                config.clone(),
                sync_client.clone(),
                metadata.clone(),
            );
            engine.set_paused(paused);
            engine.set_wakeup(wakeup);
            let rt = tokio::runtime::Runtime::new().expect("sync runtime");
            rt.block_on(async move {
                if let Err(e) = engine.run().await {
                    tracing::error!("Sync engine error: {}", e);
                }
            });
        });
    }

    fn start_watcher(&mut self) {
        if self.watcher.is_some() {
            return;
        }
        if self.config.sync_root.is_dir() {
            let metadata = self.metadata.clone();
            let root = self.config.sync_root.clone();
            let watcher = crate::watcher::FileWatcher::new(root, Arc::new(move |path_str| {
                let mut store = metadata.lock().unwrap();
                if let Err(e) = store.enqueue(&path_str, "pending", None) {
                    tracing::warn!("Failed to enqueue watcher change: {}", e);
                }
            }));
            if let Ok(watcher_handle) = watcher.start() {
                self.watcher_handle = Some(watcher_handle);
                self.watcher = Some(watcher);
                self.add_activity("File watcher iniciado".to_string());
            }
        }
    }

    fn login(&mut self) {
        if self.login_email.is_empty() || self.login_password.is_empty() {
            self.login_error = Some("Ingresa email y contraseña".to_string());
            return;
        }
        if self.connecting {
            return;
        }
        self.connecting = true;
        self.login_error = None;
        *self.login_result.lock().unwrap() = None;

        // Persistir URL servidor y email para futuros inicios
        let server_changed = self.login_server_url != self.config.server_url;
        let email = self.login_email.clone();
        let password = self.login_password.clone();
        let device_id = self.config.device_id.clone();
        let server_url = self.login_server_url.trim_end_matches('/').to_string();
        self.config.server_url = server_url.clone();
        self.config.email = email.clone();
        self.config.password = password.clone();
        let _ = self.config.save();
        if server_changed {
            // Recrear cliente con la nueva URL
            if let Ok(client) = crate::network::SyncClient::new(&server_url, &device_id) {
                self.sync_client = Arc::new(client);
            }
        }

        let client = self.sync_client.clone();
        let metadata = self.metadata.clone();
        let login_result = self.login_result.clone();

        std::thread::spawn(move || {
            let auth = crate::auth::AuthService::new(client, metadata.clone());
            let rt = tokio::runtime::Runtime::new().expect("auth runtime");
            let result = rt.block_on(async move {
                auth.login_raw(&email, &password, &device_id).await
            });
            *login_result.lock().unwrap() = Some(result.map(|_| ()).map_err(|e| e.to_string()));
        });
    }

    fn logout(&mut self) {
        let session_id = self.session_id.clone();
        if let Some(session_id) = session_id {
            let client = self.sync_client.clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("logout runtime");
                rt.block_on(async move {
                    let _ = client.logout(&session_id).await;
                });
            });
        }
        let metadata = self.metadata.lock().unwrap();
        let _ = metadata.revoke_session();
        drop(metadata);
        self.logged_in = false;
        self.user_id = None;
        self.session_id = None;
        self.syncing = false;
        self.connected = false;
        self.login_password.clear();
        self.view = View::Dashboard;
        self.add_activity("Sesion cerrada".to_string());
        self.notify(NotificationKind::Info, "Sesion cerrada");
    }

    fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        let _ = self.sync_wakeup.send(());
        let msg = if self.paused { "Sincronizacion pausada".to_string() } else { "Sincronizacion reanudada".to_string() };
        self.add_activity(msg);
        self.notify(NotificationKind::Info, if self.paused { "Sincronizacion pausada" } else { "Sincronizacion reanudada" });
    }

    fn sync_now(&mut self) {
        if self.paused {
            self.notify(NotificationKind::Warning, "Sincronizacion pausada");
            return;
        }
        self.syncing = true;
        let _ = self.sync_wakeup.send(());
        self.add_activity("Sincronizacion solicitada...".to_string());
        self.notify(NotificationKind::Info, "Sincronizando...");
    }

    fn refresh_all(&mut self) {
        self.refresh_files();
        self.refresh_queue();
        self.refresh_conflicts();
        self.refresh_devices();
        self.refresh_activity();
        self.refresh_stats();
    }

    fn refresh_files(&mut self) {
        if !self.config.sync_root.is_dir() {
            let message = format!("La carpeta no existe: {}", self.config.sync_root.display());
            self.scan_error = Some(message.clone());
            self.files.clear();
            self.activity.insert(0, message);
            return;
        }

        let mut files = Vec::new();
        collect_files(&self.config.sync_root, &self.config.sync_root, &mut files);

        let metadata = self.metadata.lock().unwrap();
        let db_entries = metadata.get_files_by_status("pending")
            .or_else(|_| metadata.get_files_by_status("synced"))
            .unwrap_or_default();
        drop(metadata);

        let mut db_map = std::collections::HashMap::new();
        for entry in &db_entries {
            db_map.insert(entry.relative_path.clone(), entry.status.clone());
        }

        for file in &mut files {
            let state = db_map.get(&file.path).map(|s| match s.as_str() {
                "synced" => FileState::Synced,
                "pending" => FileState::Pending,
                "conflict" => FileState::Conflict,
                "quarantined" => FileState::Error,
                "deleted" => FileState::Deleted,
                _ => FileState::Local,
            }).unwrap_or(FileState::Local);
            file.status = state;
        }

        files.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
        self.files = files;
        if self.files.is_empty() {
            self.activity.insert(0, "Carpeta vacia".to_string());
        }
    }

    fn refresh_queue(&mut self) {
        let metadata = self.metadata.lock().unwrap();
        match metadata.get_queued_ops() {
            Ok(entries) => {
                self.queue_entries = entries;
            }
            Err(e) => {
                tracing::warn!("Failed to load queue: {}", e);
                self.queue_entries.clear();
            }
        }
    }

    fn refresh_conflicts(&mut self) {
        let metadata = self.metadata.lock().unwrap();
        match metadata.get_conflicts() {
            Ok(entries) => {
                self.conflicts = entries.into_iter().map(|e| {
                    // Buscar la ruta real del archivo en la BD local
                    let path = metadata.get_file_by_id(&e.file_id)
                        .ok()
                        .flatten()
                        .map(|f| f.relative_path)
                        .unwrap_or_else(|| e.file_id.clone());
                    UiConflict {
                        conflict_id: e.conflict_id,
                        file_id: e.file_id,
                        path,
                        local_checksum: e.local_checksum.unwrap_or_default(),
                        remote_checksum: e.remote_checksum.unwrap_or_default(),
                        strategy: e.strategy,
                        created_at: e.created_at,
                        resolved: e.resolved_at.is_some(),
                    }
                }).collect();
            }
            Err(e) => {
                tracing::warn!("Failed to load conflicts: {}", e);
                self.conflicts.clear();
            }
        }
    }

    fn refresh_devices(&mut self) {
        let metadata = self.metadata.lock().unwrap();
        match metadata.get_devices() {
            Ok(entries) => {
                self.devices = entries.into_iter().map(|d| UiDevice {
                    device_id: d.device_id,
                    platform: d.platform,
                    device_name: d.device_name,
                    last_seen_at: Some(d.last_seen_at),
                    status: "active".to_string(),
                }).collect();
            }
            Err(e) => {
                tracing::warn!("Failed to load devices: {}", e);
                self.devices.clear();
            }
        }
    }

    fn refresh_stats(&mut self) {
        self.dashboard_stats = None;
        self.stats_error = None;
        let Some(session_id) = self.session_id.clone() else { return };
        let client = self.sync_client.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("stats runtime");
            let result: Result<syncfiles_models::StorageStats, String> = rt.block_on(async move {
                let url = format!("{}/api/v1/storage/stats", client.base_url());
                let resp = client.http_get_bearer(&url, &session_id).await.map_err(|e| e.to_string())?;
                serde_json::from_str::<syncfiles_models::StorageStats>(&resp)
                    .map_err(|e| e.to_string())
            });
            if let Ok(stats) = result {
                let dir = crate::config::Config::data_dir();
                let _ = std::fs::create_dir_all(&dir);
                let path = dir.join("stats_cache.json");
                if let Ok(json) = serde_json::to_string(&stats) {
                    let _ = std::fs::write(path, json);
                }
            }
        });
    }

    fn load_stats_cache(&mut self) {
        let path = crate::config::Config::data_dir().join("stats_cache.json");
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(stats) = serde_json::from_str::<syncfiles_models::StorageStats>(&content) {
                self.dashboard_stats = Some(stats);
            }
        }
    }

    fn refresh_activity(&mut self) {
        let metadata = self.metadata.lock().unwrap();
        match metadata.get_audit_log() {
            Ok(entries) => {
                self.activity = entries.into_iter().map(|e| {
                    format!("{} - {}", e.event_name, e.payload_json.unwrap_or_default())
                }).collect();
            }
            Err(e) => {
                tracing::warn!("Failed to load activity: {}", e);
            }
        }
    }

    fn add_activity(&mut self, msg: String) {
        self.activity.insert(0, msg);
        if self.activity.len() > 200 {
            self.activity.truncate(200);
        }
    }

    fn notify(&mut self, kind: NotificationKind, message: impl Into<String>) {
        self.notifications.push(Notification {
            id: uuid::Uuid::new_v4(),
            message: message.into(),
            kind,
            created_at: std::time::Instant::now(),
        });
        if self.notifications.len() > 20 {
            self.notifications.remove(0);
        }
    }

    fn prune_notifications(&mut self) {
        let cutoff = std::time::Instant::now() - Duration::from_secs(8);
        self.notifications.retain(|n| n.created_at > cutoff);
    }

    // ---------- Operaciones remotas (modales) ----------

    fn do_rename(&mut self, file_id: &str, old_path: &str, new_path: &str) {
        let Some(session_id) = self.session_id.clone() else { return };
        let client = self.sync_client.clone();
        let metadata = self.metadata.clone();
        let sync_root = self.config.sync_root.clone();
        let file_id = file_id.to_string();
        let old_path = old_path.to_string();
        let new_path = new_path.to_string();
        let old_path_for_remote = old_path.clone();
        let new_path_for_remote = new_path.clone();
        let old_path_for_log = old_path.clone();
        let new_path_for_log = new_path.clone();

        // Renombrar localmente primero
        let old_local = sync_root.join(&old_path);
        let new_local = sync_root.join(&new_path);
        if old_local.exists() {
            if let Some(parent) = new_local.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::fs::rename(&old_local, &new_local).is_err() {
                self.notify(NotificationKind::Error, "No se pudo renombrar localmente");
                return;
            }
            {
                let store = metadata.lock().unwrap();
                let _ = store.rename_local_path(&old_path, &new_path);
            }
        }

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("rename runtime");
            rt.block_on(async move {
                if let Err(e) = client.rename(&session_id, &file_id, &old_path_for_remote, &new_path_for_remote).await {
                    tracing::error!("Error al renombrar remoto: {}", e);
                }
            });
        });
        self.add_activity(format!("Renombrado: {} -> {}", old_path_for_log, new_path_for_log));
        self.notify(NotificationKind::Success, "Archivo renombrado");
        self.refresh_files();
    }

    fn do_move_file(&mut self, file_id: &str, old_path: &str, new_path: &str) {
        let Some(session_id) = self.session_id.clone() else { return };
        let client = self.sync_client.clone();
        let metadata = self.metadata.clone();
        let sync_root = self.config.sync_root.clone();
        let file_id = file_id.to_string();
        let old_path = old_path.to_string();
        let new_path = new_path.to_string();
        let old_path_for_remote = old_path.clone();
        let new_path_for_remote = new_path.clone();
        let old_path_for_log = old_path.clone();
        let new_path_for_log = new_path.clone();

        let old_local = sync_root.join(&old_path);
        let new_local = sync_root.join(&new_path);
        if old_local.exists() {
            if let Some(parent) = new_local.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::fs::rename(&old_local, &new_local).is_err() {
                self.notify(NotificationKind::Error, "No se pudo mover localmente");
                return;
            }
            {
                let store = metadata.lock().unwrap();
                let _ = store.rename_local_path(&old_path, &new_path);
            }
        }

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("move runtime");
            rt.block_on(async move {
                if let Err(e) = client.move_file(&session_id, &file_id, &old_path_for_remote, &new_path_for_remote).await {
                    tracing::error!("Error al mover remoto: {}", e);
                }
            });
        });
        self.add_activity(format!("Movido: {} -> {}", old_path_for_log, new_path_for_log));
        self.notify(NotificationKind::Success, "Archivo movido");
        self.refresh_files();
    }

    fn do_copy_file(&mut self, file_id: &str, source_path: &str, destination_path: &str) {
        let Some(session_id) = self.session_id.clone() else { return };
        let client = self.sync_client.clone();
        let sync_root = self.config.sync_root.clone();
        let file_id = file_id.to_string();
        let source_path = source_path.to_string();
        let destination_path = destination_path.to_string();
        let source_for_log = source_path.clone();
        let destination_for_log = destination_path.clone();

        let src_local = sync_root.join(&source_path);
        let dst_local = sync_root.join(&destination_path);
        if src_local.exists() {
            if let Some(parent) = dst_local.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::fs::copy(&src_local, &dst_local).is_err() {
                self.notify(NotificationKind::Error, "No se pudo copiar localmente");
                return;
            }
        }

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("copy runtime");
            rt.block_on(async move {
                if let Err(e) = client.copy_file(&session_id, &file_id, &source_path, &destination_path).await {
                    tracing::error!("Error al copiar remoto: {}", e);
                }
            });
        });
        self.add_activity(format!("Copiado: {} -> {}", source_for_log, destination_for_log));
        self.notify(NotificationKind::Success, "Archivo copiado");
        self.refresh_files();
    }

    fn do_delete_file(&mut self, file_id: &str, path: &str) {
        let Some(session_id) = self.session_id.clone() else { return };
        let client = self.sync_client.clone();
        let sync_root = self.config.sync_root.clone();
        let file_id = file_id.to_string();
        let path = path.to_string();
        let path_for_log = path.clone();

        let local = sync_root.join(&path);
        if local.exists() {
            if std::fs::remove_file(&local).is_err() {
                self.notify(NotificationKind::Error, "No se pudo borrar localmente");
                return;
            }
        }

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("delete runtime");
            rt.block_on(async move {
                let path_hash = syncfiles_models::compute_path_hash(&path);
                if let Err(e) = client.delete(&session_id, &file_id, &path_hash).await {
                    tracing::error!("Error al borrar remoto: {}", e);
                }
            });
        });
        self.add_activity(format!("Eliminado: {}", path_for_log));
        self.notify(NotificationKind::Success, "Archivo eliminado");
        self.refresh_files();
        self.refresh_queue();
    }

    fn draw_login_screen(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.heading("SyncFiles");
            ui.label("Inicia sesión para sincronizar");
            ui.add_space(24.0);

            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width().min(380.0));
                egui::Grid::new("login").num_columns(2).show(ui, |ui| {
                    ui.label("Servidor");
                    ui.text_edit_singleline(&mut self.login_server_url);
                    ui.end_row();
                    ui.label("Email");
                    ui.text_edit_singleline(&mut self.login_email);
                    ui.end_row();
                    ui.label("Contraseña");
                    ui.add(egui::TextEdit::singleline(&mut self.login_password).password(true));
                    ui.end_row();
                });
                ui.add_space(12.0);
                ui.vertical_centered(|ui| {
                    let label = if self.connecting { "Conectando..." } else { "Iniciar sesión" };
                    if ui.add_enabled(!self.connecting, egui::Button::new(label)).clicked() {
                        self.login();
                    }
                });
            });

            if self.connecting {
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Conectando...").color(egui::Color32::from_rgb(230, 170, 45)));
            }
            if let Some(error) = &self.login_error {
                ui.add_space(8.0);
                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), format!("⚠ {}", error));
            }
        });
    }

    fn draw_content(&mut self, ui: &mut egui::Ui) {
        match self.view {
            View::Files => self.draw_files(ui),
            View::Queue => self.draw_queue(ui),
            View::Conflicts => self.draw_conflicts(ui),
            View::Activity => self.draw_activity(ui),
            View::Settings => self.draw_settings(ui),
            View::Dashboard => self.draw_dashboard(ui),
            View::Devices => self.draw_devices(ui),
        }
    }

    fn draw_dashboard(&mut self, ui: &mut egui::Ui) {
        ui.heading("Panel");
        ui.label("Estado general de la sincronización.");
        ui.add_space(16.0);

        egui::Grid::new("dashboard").num_columns(2).show(ui, |ui| {
            ui.strong("Estado");
            let (label, color) = if self.paused {
                ("Pausado", egui::Color32::from_rgb(230, 170, 45))
            } else if self.syncing {
                ("Sincronizando...", egui::Color32::from_rgb(230, 170, 45))
            } else if self.connected {
                ("Conectado", egui::Color32::from_rgb(45, 180, 110))
            } else {
                ("Desconectado", egui::Color32::from_rgb(220, 80, 80))
            };
            ui.colored_label(color, label);
            ui.end_row();

            ui.strong("Usuario");
            ui.label(&self.user_email);
            ui.end_row();

            ui.strong("Carpeta local");
            ui.label(self.config.sync_root.display().to_string());
            ui.end_row();

            ui.strong("Archivos locales");
            ui.label(format!("{}", self.files.len()));
            ui.end_row();

            ui.strong("Operaciones en cola");
            ui.label(format!("{}", self.queue_entries.len()));
            ui.end_row();

            ui.strong("Conflictos pendientes");
            ui.label(format!("{}", self.conflicts.iter().filter(|c| !c.resolved).count()));
            ui.end_row();

            ui.strong("Última sincronización");
            ui.label(match self.last_sync {
                Some(t) => format!("hace {} s", t.elapsed().as_secs()),
                None => "nunca".to_string(),
            });
            ui.end_row();
        });

        if let Some(stats) = &self.dashboard_stats {
            ui.add_space(12.0);
            ui.separator();
            ui.heading("Almacenamiento en servidor");
            egui::Grid::new("stats").num_columns(2).show(ui, |ui| {
                ui.strong("Espacio usado");
                ui.label(format_size(stats.used_bytes.max(0) as u64));
                ui.end_row();
                ui.strong("Archivos remotos");
                ui.label(format!("{}", stats.file_count));
                ui.end_row();
                ui.strong("Última modificación remota");
                ui.label(match stats.last_modified_at {
                    Some(ms) => format!("hace {} s", chrono_ms_ago(ms)),
                    None => "-".to_string(),
                });
                ui.end_row();
            });
        } else if self.stats_error.is_some() {
            ui.add_space(8.0);
            ui.colored_label(egui::Color32::from_rgb(220, 80, 80), "No se pudieron cargar las estadísticas del servidor");
        }

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("Actualizar datos").clicked() {
                self.refresh_all();
                self.load_stats_cache();
            }
            if ui.button("Sincronizar ahora").clicked() {
                self.sync_now();
            }
        });
    }

    fn draw_devices(&mut self, ui: &mut egui::Ui) {
        ui.heading("Dispositivos");
        ui.label("Dispositivos conocidos de esta cuenta.");
        ui.add_space(16.0);
        if self.devices.is_empty() {
            ui.label("No hay dispositivos registrados.");
            return;
        }
        egui::Grid::new("devices").striped(true).show(ui, |ui| {
            ui.strong("Dispositivo");
            ui.strong("Plataforma");
            ui.strong("Nombre");
            ui.strong("Última conexión");
            ui.end_row();
            for device in &self.devices {
                ui.label(&device.device_id);
                ui.label(&device.platform);
                ui.label(device.device_name.as_deref().unwrap_or("-"));
                ui.label(match device.last_seen_at {
                    Some(ms) => format!("hace {} s", chrono_ms_ago(ms)),
                    None => "-".to_string(),
                });
                ui.end_row();
            }
        });
        ui.add_space(16.0);
        if ui.button("Refrescar dispositivos").clicked() {
            self.refresh_devices();
        }
    }

    fn draw_conflicts(&mut self, ui: &mut egui::Ui) {
        ui.heading("Conflictos");
        ui.label("Resuelve conflictos de sincronización.");
        ui.add_space(16.0);
        if self.conflicts.is_empty() {
            ui.label("No hay conflictos pendientes.");
            return;
        }
        let conflicts: Vec<_> = self.conflicts.iter().cloned().collect();
        egui::Frame::group(ui.style()).show(ui, |ui| {
            egui::Grid::new("conflicts").striped(true).show(ui, |ui| {
                ui.strong("Archivo");
                ui.strong("Local");
                ui.strong("Remoto");
                ui.strong("Estado");
                ui.end_row();
                for conflict in &conflicts {
                    ui.label(&conflict.path);
                    ui.label(&conflict.local_checksum[..8.min(conflict.local_checksum.len())]);
                    ui.label(&conflict.remote_checksum[..8.min(conflict.remote_checksum.len())]);
                    ui.label(if conflict.resolved { "Resuelto" } else { "Pendiente" });
                    ui.end_row();
                    if !conflict.resolved {
                        let conflict_id = conflict.conflict_id.clone();
                        ui.horizontal(|ui| {
                            if ui.button("Mantener local").clicked() {
                                self.resolve_conflict(&conflict_id, "keep_local");
                            }
                            if ui.button("Mantener remoto").clicked() {
                                self.resolve_conflict(&conflict_id, "keep_remote");
                            }
                        });
                        ui.end_row();
                    }
                }
            });
        });
        ui.add_space(16.0);
        if ui.button("Refrescar conflictos").clicked() {
            self.refresh_conflicts();
        }
    }

    fn resolve_conflict(&mut self, conflict_id: &str, decision: &str) {
        let session_id = match &self.session_id {
            Some(s) => s.clone(),
            None => return,
        };
        let device_id = self.config.device_id.clone();
        let client = self.sync_client.clone();
        let metadata = self.metadata.clone();
        let conflict_id = conflict_id.to_string();
        let decision = decision.to_string();
        let conflict_id_remote = conflict_id.clone();
        let conflict_id_store = conflict_id.clone();
        let decision_for_log = decision.clone();
        let conflict_id_for_log = conflict_id.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("resolve runtime");
            rt.block_on(async move {
                let req = syncfiles_models::ResolveConflictRequest {
                    session_id: session_id.clone(),
                    device_id: device_id.clone(),
                    conflict_id: conflict_id_remote.clone(),
                    decision: decision.clone(),
                    preserve_alternative: true,
                    new_name: None,
                };
                if let Err(e) = client.resolve_conflict(&session_id, &req).await {
                    tracing::error!("Error al resolver conflicto: {}", e);
                } else {
                    let store = metadata.lock().unwrap();
                    let _ = store.resolve_conflict(&conflict_id_store, "ui");
                }
            });
        });
        self.add_activity(format!("Conflicto resuelto: {} -> {}", conflict_id_for_log, decision_for_log));
        self.notify(NotificationKind::Success, "Conflicto resuelto");
        self.refresh_conflicts();
    }

    fn draw_files(&mut self, ui: &mut egui::Ui) {
        ui.heading("Mis archivos");
        ui.label("Clic derecho sobre un archivo para renombrar, mover, copiar o eliminar.");
        ui.add_space(16.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Carpeta sincronizada");
                ui.separator();
                ui.label(self.config.sync_root.display().to_string());
            });
            ui.add_space(8.0);
            let compact = ui.available_width() < 600.0;
            let files: Vec<UiFile> = self.files.clone();
            egui::Grid::new("files").striped(true).show(ui, |ui| {
                ui.strong("Nombre");
                if !compact {
                    ui.strong("Ruta");
                    ui.strong("Tamaño");
                }
                ui.strong("Estado");
                ui.end_row();
                for file in &files {
                    let (label, color) = Self::status_label(file.status);
                    let response = ui.label(if file.is_dir {
                        format!("📁 {}", file.name)
                    } else {
                        format!("📄 {}", file.name)
                    });
                    let file = file.clone();
                    response.context_menu(|ui| {
                        self.context_menu_file = Some(file.clone());
                        self.draw_file_context_menu(ui);
                    });
                    if response.clicked() {
                        self.selected_file = Some(file.clone());
                    }
                    if !compact {
                        ui.label(&file.path);
                        ui.label(&file.size);
                    }
                    ui.colored_label(color, label);
                    ui.end_row();
                }
            });
        });
        ui.add_space(16.0);
        if ui.button("Actualizar vista").clicked() {
            self.refresh_files();
            self.refresh_queue();
        }
        ui.small(format!("Última lectura local: {}", match self.last_sync {
            Some(t) => format!("hace {} s", t.elapsed().as_secs()),
            None => "nunca".to_string(),
        }));
        if let Some(error) = &self.scan_error {
            ui.colored_label(egui::Color32::from_rgb(220, 80, 80), error);
        }
        ui.horizontal(|ui| {
            ui.heading("Resumen");
            ui.separator();
            ui.label(format!("{} archivos", self.files.len()));
            ui.label(format!("En cola: {}", self.queue_entries.len()));
        });
    }

    fn draw_file_context_menu(&mut self, ui: &mut egui::Ui) {
        if ui.button("Renombrar").clicked() {
            if let Some(file) = self.context_menu_file.clone() {
                let file_id = self.resolve_file_id(&file.path);
                self.modal = Modal::Rename { file_id, old_path: file.path.clone(), new_path: file.path.clone() };
            }
            ui.close_menu();
        }
        if ui.button("Mover").clicked() {
            if let Some(file) = self.context_menu_file.clone() {
                let file_id = self.resolve_file_id(&file.path);
                self.modal = Modal::Move { file_id, old_path: file.path.clone(), new_path: file.path.clone() };
            }
            ui.close_menu();
        }
        if ui.button("Copiar").clicked() {
            if let Some(file) = self.context_menu_file.clone() {
                let file_id = self.resolve_file_id(&file.path);
                self.modal = Modal::Copy { file_id, source_path: file.path.clone(), destination_path: file.path.clone() };
            }
            ui.close_menu();
        }
        ui.separator();
        if ui.button("Eliminar").clicked() {
            if let Some(file) = self.context_menu_file.clone() {
                let file_id = self.resolve_file_id(&file.path);
                self.modal = Modal::Delete { file_id, path: file.path.clone() };
            }
            ui.close_menu();
        }
    }

    fn resolve_file_id(&self, relative_path: &str) -> String {
        let metadata = self.metadata.lock().unwrap();
        metadata.get_file_by_relative_path(relative_path)
            .ok()
            .flatten()
            .map(|f| f.file_id)
            .unwrap_or_default()
    }

    fn draw_modal(&mut self, ctx: &egui::Context) {
        if self.modal == Modal::None {
            return;
        }
        let mut close = false;
        let mut action: Option<ModalAction> = None;

        egui::Window::new("Operación")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                match self.modal.clone() {
                    Modal::Rename { file_id, old_path, ref mut new_path } => {
                        ui.heading("Renombrar archivo");
                        ui.label(format!("Actual: {}", old_path));
                        ui.text_edit_singleline(new_path);
                        if ui.button("Renombrar").clicked() {
                            action = Some(ModalAction::Rename { file_id, old_path, new_path: new_path.clone() });
                            close = true;
                        }
                    }
                    Modal::Move { file_id, old_path, ref mut new_path } => {
                        ui.heading("Mover archivo");
                        ui.label(format!("Actual: {}", old_path));
                        ui.text_edit_singleline(new_path);
                        if ui.button("Mover").clicked() {
                            action = Some(ModalAction::Move { file_id, old_path, new_path: new_path.clone() });
                            close = true;
                        }
                    }
                    Modal::Copy { file_id, source_path, ref mut destination_path } => {
                        ui.heading("Copiar archivo");
                        ui.label(format!("Origen: {}", source_path));
                        ui.text_edit_singleline(destination_path);
                        if ui.button("Copiar").clicked() {
                            action = Some(ModalAction::Copy { file_id, source_path, destination_path: destination_path.clone() });
                            close = true;
                        }
                    }
                    Modal::Delete { file_id, path } => {
                        ui.heading("Eliminar archivo");
                        ui.label(format!("¿Eliminar \"{}\"?", path));
                        if ui.button("Eliminar").clicked() {
                            action = Some(ModalAction::Delete { file_id, path });
                            close = true;
                        }
                    }
                    Modal::ConflictResolve { conflict_id, file_id: _ } => {
                        ui.heading("Resolver conflicto");
                        ui.label(format!("Conflicto {}", conflict_id));
                        if ui.button("Mantener local").clicked() {
                            action = Some(ModalAction::ResolveConflict { conflict_id: conflict_id.clone(), decision: "keep_local".to_string() });
                            close = true;
                        }
                        if ui.button("Mantener remoto").clicked() {
                            action = Some(ModalAction::ResolveConflict { conflict_id: conflict_id.clone(), decision: "keep_remote".to_string() });
                            close = true;
                        }
                    }
                    Modal::None => {}
                }
                if ui.button("Cancelar").clicked() {
                    close = true;
                }
            });

        if let Some(a) = action {
            match a {
                ModalAction::Rename { file_id, old_path, new_path } => self.do_rename(&file_id, &old_path, &new_path),
                ModalAction::Move { file_id, old_path, new_path } => self.do_move_file(&file_id, &old_path, &new_path),
                ModalAction::Copy { file_id, source_path, destination_path } => self.do_copy_file(&file_id, &source_path, &destination_path),
                ModalAction::Delete { file_id, path } => self.do_delete_file(&file_id, &path),
                ModalAction::ResolveConflict { conflict_id, decision } => self.resolve_conflict(&conflict_id, &decision),
            }
        }
        if close {
            self.modal = Modal::None;
            self.context_menu_file = None;
        }
    }

    fn draw_queue(&mut self, ui: &mut egui::Ui) {
        ui.heading("Cola de sincronización");
        ui.label("Operaciones pendientes y en progreso.");
        ui.add_space(16.0);
        if self.queue_entries.is_empty() {
            ui.label("No hay operaciones en cola.");
        } else {
            egui::Grid::new("queue").striped(true).show(ui, |ui| {
                ui.strong("Operación");
                ui.strong("Archivo");
                ui.strong("Estado");
                ui.strong("Intentos");
                ui.end_row();
                for entry in &self.queue_entries {
                    ui.label(&entry.operation);
                    ui.label(&entry.file_id);
                    ui.label(&entry.status);
                    ui.label(entry.attempts.to_string());
                    ui.end_row();
                }
            });
        }
        ui.add_space(16.0);
        if ui.button("Refrescar cola").clicked() {
            self.refresh_queue();
        }
    }

    fn draw_activity(&mut self, ui: &mut egui::Ui) {
        ui.heading("Actividad");
        ui.label("Últimos eventos del cliente.");
        ui.add_space(16.0);
        for event in &self.activity {
            ui.horizontal(|ui| {
                ui.colored_label(egui::Color32::from_rgb(45, 180, 110), "●");
                ui.label(event);
            });
        }
    }

    fn draw_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Configuración");
        ui.label("Ajusta la conexión y la carpeta de trabajo.");
        ui.add_space(16.0);
        let mut server_url = self.config.server_url.clone();
        let mut sync_root_str = self.config.sync_root.display().to_string();
        egui::Grid::new("settings").num_columns(2).show(ui, |ui| {
            ui.label("Servidor");
            ui.text_edit_singleline(&mut server_url);
            ui.end_row();
            ui.label("Carpeta local");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut sync_root_str);
                if ui.button("Elegir...").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        sync_root_str = path.display().to_string();
                    }
                }
            });
            ui.end_row();
            ui.label("Intervalo de sondeo (s)");
            ui.add(egui::DragValue::new(&mut self.config.polling_interval_secs).range(5..=600));
            ui.end_row();
        });
        ui.add_space(16.0);
        if ui.button("Aplicar y guardar").clicked() {
            let trimmed_server = server_url.trim_end_matches('/').to_string();
            let server_changed = trimmed_server != self.config.server_url;
            self.config.server_url = trimmed_server;
            let new_root = std::path::PathBuf::from(&sync_root_str);
            let root_changed = new_root != self.config.sync_root;
            self.config.sync_root = new_root.clone();
            if let Err(e) = self.config.save() {
                self.notify(NotificationKind::Error, format!("No se pudo guardar config: {}", e));
            } else {
                self.notify(NotificationKind::Success, "Configuración guardada");
            }
            if server_changed {
                if let Ok(client) = crate::network::SyncClient::new(&self.config.server_url, &self.config.device_id) {
                    self.sync_client = Arc::new(client);
                    self.notify(NotificationKind::Warning, "Servidor cambiado: vuelve a iniciar sesión");
                }
            }
            if root_changed {
                let _ = std::fs::create_dir_all(&new_root);
                self.watcher = None;
                if let Some(handle) = self.watcher_handle.take() {
                    drop(handle);
                }
                if self.logged_in {
                    self.start_watcher();
                }
            }
            self.refresh_files();
            self.refresh_queue();
        }
        if let Some(error) = &self.scan_error {
            ui.add_space(8.0);
            ui.colored_label(egui::Color32::from_rgb(220, 80, 80), error);
        }
    }

    fn status_label(state: FileState) -> (&'static str, egui::Color32) {
        match state {
            FileState::Local => ("Local", egui::Color32::from_rgb(150, 150, 150)),
            FileState::Synced => ("Sincronizado", egui::Color32::from_rgb(45, 180, 110)),
            FileState::Pending => ("Pendiente", egui::Color32::from_rgb(230, 170, 45)),
            FileState::Conflict => ("Conflicto", egui::Color32::from_rgb(220, 80, 80)),
            FileState::Error => ("Error", egui::Color32::from_rgb(220, 80, 80)),
            FileState::Deleted => ("Eliminado", egui::Color32::from_rgb(180, 180, 180)),
        }
    }

    fn draw_notifications(&mut self, ctx: &egui::Context) {
        if self.notifications.is_empty() {
            return;
        }
        let mut dismissed: Vec<uuid::Uuid> = Vec::new();
        egui::Area::new(egui::Id::new("notifications"))
            .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
            .show(ctx, |ui| {
                for n in &self.notifications {
                    let (color, icon) = match n.kind {
                        NotificationKind::Info => (egui::Color32::from_rgb(90, 130, 220), "ℹ"),
                        NotificationKind::Success => (egui::Color32::from_rgb(45, 180, 110), "✔"),
                        NotificationKind::Warning => (egui::Color32::from_rgb(230, 170, 45), "⚠"),
                        NotificationKind::Error => (egui::Color32::from_rgb(220, 80, 80), "✖"),
                    };
                    let frame = egui::Frame::popup(ui.style())
                        .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 235));
                    let resp = frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.colored_label(color, icon);
                            ui.label(&n.message);
                            if ui.small_button("×").clicked() {
                                dismissed.push(n.id);
                            }
                        });
                    });
                    let _ = resp;
                }
            });
        if !dismissed.is_empty() {
            self.notifications.retain(|n| !dismissed.contains(&n.id));
        }
        self.prune_notifications();
    }
}

enum ModalAction {
    Rename { file_id: String, old_path: String, new_path: String },
    Move { file_id: String, old_path: String, new_path: String },
    Copy { file_id: String, source_path: String, destination_path: String },
    Delete { file_id: String, path: String },
    ResolveConflict { conflict_id: String, decision: String },
}

fn chrono_ms_ago(ms: i64) -> u64 {
    let now = chrono::Utc::now().timestamp_millis();
    (now - ms).max(0) as u64 / 1000
}

impl eframe::App for SyncFilesUi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.connecting {
            let login_result = self.login_result.lock().unwrap().take();
            if let Some(result) = login_result {
                self.connecting = false;
                match result {
                    Ok(_) => {
                        let metadata = self.metadata.lock().unwrap();
                        let session_data = metadata.get_active_session()
                            .ok()
                            .flatten()
                            .map(|s| (s.user_id.clone(), s.session_id.clone()));
                        drop(metadata);
                        if let Some((user_id, session_id)) = session_data {
                            self.user_id = Some(user_id);
                            self.session_id = Some(session_id);
                            self.logged_in = true;
                            self.user_email = self.config.email.clone();
                            self.login_password.clear();
                            self.add_activity("Login exitoso".to_string());
                            self.notify(NotificationKind::Success, "Sesion iniciada");
                            self.refresh_all();
                            self.start_sync_engine();
                            self.start_watcher();
                        }
                    }
                    Err(e) => {
                        self.login_error = Some(e);
                        self.add_activity("Error de login".to_string());
                    }
                }
            }
        }

        if !self.logged_in {
            egui::CentralPanel::default().show(ctx, |ui| {
                self.draw_login_screen(ui);
            });
            ctx.request_repaint_after(Duration::from_millis(300));
            return;
        }

        // Refresco periódico de vistas desde la BD local
        if ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            self.refresh_all();
            self.load_stats_cache();
        }

        // Consumir estado del motor de sincronización (connected / last_sync)
        let status_path = crate::config::Config::data_dir().join("engine_status.json");
        if let Ok(content) = std::fs::read_to_string(&status_path) {
            if let Ok(status) = serde_json::from_str::<crate::sync::EngineStatus>(&content) {
                self.connected = status.last_cycle_ok && !status.paused;
                if status.last_cycle_ok {
                    self.last_sync = Some(std::time::Instant::now());
                    if self.syncing {
                        self.syncing = false;
                        self.refresh_files();
                        self.refresh_queue();
                        self.refresh_conflicts();
                    }
                }
            }
        }

        ctx.request_repaint_after(Duration::from_millis(500));

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("SyncFiles");
                ui.separator();
                let (label, color) = if self.paused {
                    ("Pausado", egui::Color32::from_rgb(230, 170, 45))
                } else if self.syncing {
                    ("Sincronizando...", egui::Color32::from_rgb(230, 170, 45))
                } else if self.connected {
                    ("Conectado", egui::Color32::from_rgb(45, 180, 110))
                } else {
                    ("Desconectado", egui::Color32::from_rgb(220, 80, 80))
                };
                ui.colored_label(color, format!("● {label}"));
                ui.separator();
                ui.small(format!("{} · {}", self.user_email, self.config.device_id));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cerrar sesión").clicked() {
                        self.logout();
                    }
                    let pause_label = if self.paused { "▶ Reanudar" } else { "⏸ Pausar" };
                    if ui.button(pause_label).clicked() {
                        self.toggle_pause();
                    }
                    if ui.button("Sincronizar ahora").clicked() {
                        self.sync_now();
                    }
                });
            });
        });

        self.mobile_navigation = ctx.available_rect().width() < 680.0;
        if !self.mobile_navigation {
            egui::SidePanel::left("navigation")
                .resizable(false)
                .default_width(190.0)
                .show(ctx, |ui| {
                ui.add_space(12.0);
                ui.label(egui::RichText::new("ESPACIO").small().strong());
                if ui.selectable_label(self.view == View::Dashboard, "  Panel").clicked() {
                    self.view = View::Dashboard;
                }
                if ui.selectable_label(self.view == View::Files, "  Mis archivos").clicked() {
                    self.view = View::Files;
                }
                if ui.selectable_label(self.view == View::Queue, format!("  Cola ({})", self.queue_entries.len())).clicked() {
                    self.view = View::Queue;
                }
                if ui.selectable_label(self.view == View::Conflicts, format!("  Conflictos ({})", self.conflicts.iter().filter(|c| !c.resolved).count())).clicked() {
                    self.view = View::Conflicts;
                }
                if ui.selectable_label(self.view == View::Devices, "  Dispositivos").clicked() {
                    self.view = View::Devices;
                }
                if ui.selectable_label(self.view == View::Activity, "  Actividad").clicked() {
                    self.view = View::Activity;
                }
                ui.separator();
                ui.label(egui::RichText::new("CONFIGURACIÓN").small().strong());
                if ui.selectable_label(self.view == View::Settings, "  Preferencias").clicked() {
                    self.view = View::Settings;
                }
                ui.label("Servidor");
                ui.small(&self.config.server_url);
                ui.add_space(8.0);
                ui.label("Carpeta local");
                ui.small(self.config.sync_root.display().to_string());
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(12.0);
            if self.mobile_navigation {
                let conflicts_count = self.conflicts.iter().filter(|c| !c.resolved).count();
                egui::ComboBox::from_label("Vista")
                    .selected_text(view_name(self.view))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.view, View::Dashboard, "Panel");
                        ui.selectable_value(&mut self.view, View::Files, "Archivos");
                        ui.selectable_value(&mut self.view, View::Queue, "Cola");
                        ui.selectable_value(&mut self.view, View::Conflicts, format!("Conflictos ({})", conflicts_count));
                        ui.selectable_value(&mut self.view, View::Devices, "Dispositivos");
                        ui.selectable_value(&mut self.view, View::Activity, "Actividad");
                        ui.selectable_value(&mut self.view, View::Settings, "Ajustes");
                    });
                ui.separator();
            }
            self.draw_content(ui);
        });

        self.draw_modal(ctx);
        self.draw_notifications(ctx);
    }
}

fn view_name(view: View) -> &'static str {
    match view {
        View::Dashboard => "Panel",
        View::Files => "Archivos",
        View::Queue => "Cola",
        View::Conflicts => "Conflictos",
        View::Devices => "Dispositivos",
        View::Activity => "Actividad",
        View::Settings => "Ajustes",
    }
}

/// Modo headless (SF_HEADLESS=1): ejecuta el SyncEngine real del cliente sin GUI.
/// Usado por la batería E2E de Fase 5 (`scripts/e2e-phase5.sh`) para probar
/// cola persistente, reintentos y propagación con el motor de producción.
/// Env vars: SF_SERVER_URL, SF_EMAIL, SF_PASSWORD, SF_DEVICE_ID, SF_SYNC_ROOT,
/// SF_DATA_DIR (aisla SQLite/config del usuario), SF_POLLING_INTERVAL.
/// Sale con: 0 si el primer ciclo completa OK, 1 si falla el login o el ciclo.
fn headless_main() {
    let config = match crate::config::Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("headless: error cargando config: {e}");
            std::process::exit(1);
        }
    };
    if config.email.is_empty() || config.password.is_empty() {
        eprintln!("headless: SF_EMAIL/SF_PASSWORD requeridos");
        std::process::exit(1);
    }
    std::fs::create_dir_all(&config.sync_root).expect("crear sync_root");

    let sync_client = Arc::new(crate::network::SyncClient::new(&config.server_url, &config.device_id)
        .expect("crear SyncClient"));
    let metadata = Arc::new(std::sync::Mutex::new(
        crate::metadata::MetadataStore::new(config.clone()).expect("init MetadataStore"),
    ));
    let auth = crate::auth::AuthService::new(sync_client.clone(), metadata.clone());

    let rt = tokio::runtime::Runtime::new().expect("runtime headless");
    rt.block_on(async move {
        if let Err(e) = auth.login().await {
            eprintln!("headless: login falló: {e}");
            std::process::exit(1);
        }
        eprintln!("headless: login OK");

        // Watcher + motor: el motor procesa cola persistente, pull y push.
        let metadata_cb = metadata.clone();
        let root = config.sync_root.clone();
        let watcher = crate::watcher::FileWatcher::new(root, Arc::new(move |path_str| {
            let mut store = metadata_cb.lock().unwrap();
            if let Err(e) = store.enqueue(&path_str, "pending", None) {
                tracing::warn!("headless: error encolando cambio: {e}");
            }
        }));
        let _watcher_handle = watcher.start().expect("iniciar watcher");

        let engine = crate::sync::SyncEngine::new(config.clone(), sync_client.clone(), metadata.clone());
        eprintln!("headless: motor iniciado; presionar Ctrl-C para salir");
        if let Err(e) = engine.run().await {
            eprintln!("headless: error del motor: {e}");
            std::process::exit(1);
        }
    });
}

fn collect_files(root: &std::path::PathBuf, current: &std::path::PathBuf, files: &mut Vec<UiFile>) {
    let entries = match std::fs::read_dir(current) {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!("No se pudo leer {}: {error}", current.display());
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let relative = match path.strip_prefix(root) {
            Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                eprintln!("No se pudo inspeccionar {}: {error}", path.display());
                continue;
            }
        };
        let is_dir = metadata.is_dir();
        files.push(UiFile {
            name: path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or(relative.clone()),
            path: relative,
            status: FileState::Local,
            size: if is_dir {
                "Carpeta".to_string()
            } else {
                format_size(metadata.len())
            },
            is_dir,
        });
        if is_dir {
            collect_files(root, &path, files);
        }
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
