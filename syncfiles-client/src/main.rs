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

    connected: bool,
    syncing: bool,
    last_sync: std::time::Instant,
    paused: bool,

    watcher: Option<crate::watcher::FileWatcher>,

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
struct UiQueueEntry {
    queue_id: String,
    file_id: String,
    operation: String,
    status: String,
    attempts: i32,
    created_at: i64,
    last_error: Option<String>,
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
    Rename { file_id: String, old_path: String },
    Move { file_id: String, old_path: String },
    Copy { file_id: String, source_path: String },
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

        let mut ui = Self {
            config: config.clone(),
            metadata,
            sync_client,
            logged_in: false,
            login_error: None,
            connecting: false,
            login_result: Arc::new(std::sync::Mutex::new(None)),
            connected: false,
            syncing: false,
            last_sync: std::time::Instant::now() - Duration::from_secs(999),
            paused: false,
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
        std::thread::spawn(move || {
            let engine = crate::sync::SyncEngine::new(
                config.clone(),
                sync_client.clone(),
                metadata.clone(),
            );
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
            if let Ok(_watcher) = watcher.start() {
                self.watcher = Some(watcher);
                self.add_activity("File watcher iniciado".to_string());
            }
        }
    }

    fn login(&mut self) {
        if self.user_email.is_empty() {
            self.login_error = Some("Ingresa tu email".to_string());
            return;
        }
        if self.connecting {
            return;
        }
        self.connecting = true;
        self.login_error = None;
        *self.login_result.lock().unwrap() = None;

        let email = self.user_email.clone();
        let password = self.config.password.clone();
        let device_id = self.config.device_id.clone();
        let server_url = self.config.server_url.clone();
        let client = Arc::new(crate::network::SyncClient::new(&server_url, &device_id).unwrap());
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
        self.add_activity("Sesion cerrada".to_string());
        self.notify(NotificationKind::Info, "Sesion cerrada");
    }

    fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        self.syncing = !self.paused;
        self.add_activity(if self.paused { "Sincronizacion pausada".to_string() } else { "Sincronizacion reanudada".to_string() });
        self.notify(NotificationKind::Info, if self.paused { "Sincronizacion pausada" } else { "Sincronizacion reanudada" });
    }

    fn sync_now(&mut self) {
        if self.paused {
            self.notify(NotificationKind::Warning, "Sincronizacion pausada");
            return;
        }
        self.syncing = true;
        self.last_sync = std::time::Instant::now();
        self.add_activity("Sincronizacion solicitada...".to_string());
        self.notify(NotificationKind::Info, "Sincronizando...");
        std::thread::sleep(Duration::from_millis(100));
        self.syncing = false;
    }

    fn refresh_all(&mut self) {
        self.refresh_files();
        self.refresh_queue();
        self.refresh_conflicts();
        self.refresh_devices();
        self.refresh_activity();
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
                self.conflicts = entries.into_iter().map(|e| UiConflict {
                    conflict_id: e.conflict_id,
                    file_id: e.file_id.clone(),
                    path: e.file_id.clone(),
                    local_checksum: e.local_checksum.unwrap_or_default(),
                    remote_checksum: e.remote_checksum.unwrap_or_default(),
                    strategy: e.strategy,
                    created_at: e.created_at,
                    resolved: e.resolved_at.is_some(),
                }).collect();
                if !self.conflicts.is_empty() && self.syncing {
                    self.syncing = false;
                }
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

    fn draw_content(&mut self, ui: &mut egui::Ui) {
        match self.view {
            View::Files => self.draw_files(ui),
            View::Queue => self.draw_queue(ui),
            View::Conflicts => self.draw_conflicts(ui),
            View::Activity => self.draw_activity(ui),
            View::Settings => self.draw_settings(ui),
            View::Dashboard | View::Devices => {
                ui.label("Vista no implementada aún");
            }
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
        let conflict_id_for_log = conflict_id.clone();
        let decision_for_log = decision.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("resolve runtime");
            rt.block_on(async move {
                let req = syncfiles_models::ResolveConflictRequest {
                    session_id: session_id.clone(),
                    device_id: device_id.clone(),
                    conflict_id: conflict_id.clone(),
                    decision: decision.clone(),
                    preserve_alternative: true,
                    new_name: None,
                };
                if let Err(e) = client.resolve_conflict(&session_id, &req).await {
                    tracing::error!("Error al resolver conflicto: {}", e);
                } else {
                    let store = metadata.lock().unwrap();
                    let _ = store.resolve_conflict(&conflict_id, "ui");
                }
            });
        });
        self.add_activity(format!("Conflicto resuelto: {} -> {}", conflict_id_for_log, decision_for_log));
        self.notify(NotificationKind::Success, "Conflicto resuelto");
        self.refresh_conflicts();
    }

    fn draw_files(&mut self, ui: &mut egui::Ui) {
        ui.heading("Mis archivos");
        ui.label("Estado de los archivos en tu carpeta sincronizada.");
        ui.add_space(16.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Carpeta sincronizada");
                ui.separator();
                ui.label(self.config.sync_root.display().to_string());
            });
            ui.add_space(8.0);
            let compact = ui.available_width() < 600.0;
            egui::Grid::new("files").striped(true).show(ui, |ui| {
                ui.strong("Nombre");
                if !compact {
                    ui.strong("Ruta");
                    ui.strong("Tamaño");
                }
                ui.strong("Estado");
                ui.end_row();
                for file in &self.files {
                    let (label, color) = Self::status_label(file.status);
                    ui.label(if file.is_dir {
                        format!("📁 {}", file.name)
                    } else {
                        format!("📄 {}", file.name)
                    });
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
        ui.small(format!("Última lectura local: hace {} s", self.last_sync.elapsed().as_secs()));
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
        egui::Grid::new("settings").num_columns(2).show(ui, |ui| {
            ui.label("Servidor");
            ui.text_edit_singleline(&mut self.config.server_url);
            ui.end_row();
            ui.label("Carpeta local");
            ui.text_edit_singleline(&mut self.config.sync_root.display().to_string());
            ui.end_row();
        });
        ui.add_space(16.0);
        if ui.button("Aplicar y refrescar").clicked() {
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
                            self.add_activity("Login exitoso".to_string());
                            self.notify(NotificationKind::Success, "Sesion iniciada");
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

        if self.syncing && self.last_sync.elapsed() > Duration::from_millis(700) {
            self.syncing = false;
        }
        ctx.request_repaint_after(Duration::from_millis(500));

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("SyncFiles");
                ui.separator();
                let (label, color) = if self.syncing {
                    ("Sincronizando...", egui::Color32::from_rgb(230, 170, 45))
                } else if self.connected {
                    ("Conectado", egui::Color32::from_rgb(45, 180, 110))
                } else {
                    ("Desconectado", egui::Color32::from_rgb(220, 80, 80))
                };
                ui.colored_label(color, format!("● {label}"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                if ui.selectable_label(self.view == View::Files, "  Mis archivos").clicked() {
                    self.view = View::Files;
                }
                if ui.selectable_label(self.view == View::Queue, format!("  Cola ({})", self.queue_entries.len())).clicked() {
                    self.view = View::Queue;
                }
                if ui.selectable_label(self.view == View::Conflicts, format!("  Conflictos ({})", self.conflicts.len())).clicked() {
                    self.view = View::Conflicts;
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
                egui::ComboBox::from_label("Vista").selected_text(format!("{:?}", self.view))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.view, View::Files, "Archivos");
                        ui.selectable_value(&mut self.view, View::Queue, "Cola");
                        ui.selectable_value(&mut self.view, View::Activity, "Actividad");
                        ui.selectable_value(&mut self.view, View::Settings, "Ajustes");
                    });
                ui.separator();
            }
            self.draw_content(ui);
        });
    }
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