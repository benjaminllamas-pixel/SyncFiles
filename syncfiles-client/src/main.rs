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
    metadata: Arc<crate::metadata::MetadataStore>,
    _sync_client: Arc<crate::network::SyncClient>,
    connected: bool,
    syncing: bool,
    last_sync: std::time::Instant,
    files: Vec<UiFile>,
    queue_entries: Vec<crate::metadata::SyncQueueEntry>,
    activity: Vec<String>,
    view: View,
    scan_error: Option<String>,
    mobile_navigation: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum View {
    Files,
    Queue,
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

impl SyncFilesUi {
    fn new() -> Self {
        let config = crate::config::Config::load().expect("Failed to load config");
        let sync_client = Arc::new(crate::network::SyncClient::new(&config.server_url, &config.device_id).expect("Failed to create client"));

        std::thread::spawn({
            let config = config.clone();
            let sync_client = sync_client.clone();
            move || {
                let engine = crate::sync::SyncEngine::new(
                    config.clone(),
                    sync_client.clone(),
                    Arc::new(crate::metadata::MetadataStore::new(config).expect("Failed to init sync metadata")),
                );
                let rt = tokio::runtime::Runtime::new().expect("sync runtime");
                rt.block_on(async move {
                    if let Err(e) = engine.run().await {
                        tracing::error!("Sync engine error: {}", e);
                    }
                });
            }
        });

        let mut ui = Self {
            config: config.clone(),
            metadata: Arc::new(crate::metadata::MetadataStore::new(config.clone()).expect("Failed to init metadata")),
            _sync_client: sync_client,
            connected: false,
            syncing: false,
            last_sync: std::time::Instant::now() - Duration::from_secs(999),
            files: Vec::new(),
            queue_entries: Vec::new(),
            activity: vec!["Cliente iniciado".to_string()],
            view: View::Files,
            scan_error: None,
            mobile_navigation: false,
        };

        ui.ensure_auth();
        ui.refresh_files();
        ui.refresh_queue();

        ui
    }

    fn ensure_auth(&self) {
        let config = self.config.clone();
        std::thread::spawn(move || {
            let metadata = match crate::metadata::MetadataStore::new(config.clone()) {
                Ok(m) => Arc::new(m),
                Err(_) => return,
            };
            let sync_client = match crate::network::SyncClient::new(&config.server_url, &config.device_id) {
                Ok(c) => Arc::new(c),
                Err(_) => return,
            };
            let auth = crate::auth::AuthService::new(sync_client, metadata.clone());
            let rt = tokio::runtime::Runtime::new().expect("auth runtime");
            rt.block_on(async move {
                if let Err(e) = auth.ensure_session().await {
                    tracing::error!("Auth error: {}", e);
                    let _ = metadata.audit_event("auth.error", &e.to_string());
                }
            });
        });
    }

    fn status_label(status: FileState) -> (&'static str, egui::Color32) {
        match status {
            FileState::Local => ("Local", egui::Color32::from_rgb(120, 150, 220)),
            FileState::Synced => ("Sincronizado", egui::Color32::from_rgb(45, 180, 110)),
            FileState::Pending => ("Pendiente", egui::Color32::from_rgb(230, 170, 45)),
            FileState::Conflict => ("Conflicto", egui::Color32::from_rgb(220, 80, 80)),
            FileState::Error => ("Error", egui::Color32::from_rgb(220, 80, 80)),
            FileState::Deleted => ("Borrado", egui::Color32::GRAY),
        }
    }

    fn sync_now(&mut self) {
        self.syncing = true;
        self.last_sync = std::time::Instant::now();
        self.activity.insert(0, "Sincronización solicitada...".to_string());
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

        let db_entries = self.metadata.get_files_by_status("pending")
            .or_else(|_| self.metadata.get_files_by_status("synced"))
            .unwrap_or_default();

        let mut db_map = std::collections::HashMap::new();
        for entry in &db_entries {
            db_map.insert(entry.relative_path.clone(), entry.status.clone());
        }

        for file in &mut files {
            let state = db_map.get(&file.path).map(|s| match s {
                crate::metadata::FileStatus::Synced => FileState::Synced,
                crate::metadata::FileStatus::Pending => FileState::Pending,
                crate::metadata::FileStatus::Conflict => FileState::Conflict,
                crate::metadata::FileStatus::Quarantined => FileState::Error,
                crate::metadata::FileStatus::Deleted => FileState::Deleted,
            }).unwrap_or(FileState::Local);
            file.status = state;
        }

        files.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
        self.files = files;
        self.activity.insert(0, format!("Carpeta inspeccionada: {} elementos", self.files.len()));
    }

    fn refresh_queue(&mut self) {
        match self.metadata.get_queued_ops() {
            Ok(entries) => {
                self.queue_entries = entries;
            }
            Err(e) => {
                tracing::warn!("Failed to load queue: {}", e);
                self.queue_entries.clear();
            }
        }
    }

    fn draw_content(&mut self, ui: &mut egui::Ui) {
        match self.view {
            View::Files => self.draw_files(ui),
            View::Queue => self.draw_queue(ui),
            View::Activity => self.draw_activity(ui),
            View::Settings => self.draw_settings(ui),
        }
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
}

impl eframe::App for SyncFilesUi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
