use eframe::egui;
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct SyncFilesUi {
    server_url: String,
    sync_root: PathBuf,
    connected: bool,
    syncing: bool,
    last_sync: Instant,
    files: Vec<UiFile>,
    conflicts: Vec<String>,
}

struct UiFile {
    name: String,
    path: String,
    status: FileState,
    size: String,
}

#[derive(Clone, Copy)]
enum FileState {
    Synced,
    Pending,
    Conflict,
}

impl SyncFilesUi {
    fn new() -> Self {
        let server_url = std::env::var("SF_SERVER_URL")
            .or_else(|_| std::env::var("SYNCFILES_SERVER_URL"))
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
        let sync_root = std::env::var("SF_SYNC_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("SyncFiles")
            });

        Self {
            server_url,
            sync_root,
            connected: true,
            syncing: false,
            last_sync: Instant::now() - Duration::from_secs(42),
            files: vec![
                UiFile {
                    name: "Proyecto".to_string(),
                    path: "Proyecto".to_string(),
                    status: FileState::Synced,
                    size: "Carpeta".to_string(),
                },
                UiFile {
                    name: "README.md".to_string(),
                    path: "README.md".to_string(),
                    status: FileState::Synced,
                    size: "4 KB".to_string(),
                },
                UiFile {
                    name: "notas.txt".to_string(),
                    path: "notas.txt".to_string(),
                    status: FileState::Pending,
                    size: "12 KB".to_string(),
                },
            ],
            conflicts: Vec::new(),
        }
    }

    fn status_label(status: FileState) -> (&'static str, egui::Color32) {
        match status {
            FileState::Synced => ("Sincronizado", egui::Color32::from_rgb(45, 180, 110)),
            FileState::Pending => ("Pendiente", egui::Color32::from_rgb(230, 170, 45)),
            FileState::Conflict => ("Conflicto", egui::Color32::from_rgb(220, 80, 80)),
        }
    }

    fn sync_now(&mut self) {
        self.syncing = true;
        self.last_sync = Instant::now();
        for file in &mut self.files {
            if matches!(file.status, FileState::Pending) {
                file.status = FileState::Synced;
            }
        }
    }
}

impl eframe::App for SyncFilesUi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.syncing && self.last_sync.elapsed() > Duration::from_millis(700) {
            self.syncing = false;
        }
        ctx.request_repaint_after(Duration::from_millis(100));

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

        egui::SidePanel::left("navigation")
            .resizable(false)
            .default_width(190.0)
            .show(ctx, |ui| {
                ui.add_space(12.0);
                ui.label(egui::RichText::new("ESPACIO").small().strong());
                ui.selectable_label(true, "  Mis archivos");
                ui.selectable_label(false, "  Actividad");
                ui.selectable_label(false, format!("  Conflictos ({})", self.conflicts.len()));
                ui.separator();
                ui.label(egui::RichText::new("CONFIGURACIÓN").small().strong());
                ui.label("Servidor");
                ui.small(&self.server_url);
                ui.add_space(8.0);
                ui.label("Carpeta local");
                ui.small(self.sync_root.display().to_string());
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(12.0);
            ui.heading("Mis archivos");
            ui.label("Estado de los archivos en tu carpeta sincronizada.");
            ui.add_space(16.0);

            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Carpeta sincronizada");
                    ui.separator();
                    ui.label(self.sync_root.display().to_string());
                });
                ui.add_space(8.0);
                egui::Grid::new("files").striped(true).show(ui, |ui| {
                    ui.strong("Nombre");
                    ui.strong("Ruta");
                    ui.strong("Tamaño");
                    ui.strong("Estado");
                    ui.end_row();
                    for file in &self.files {
                        let (label, color) = Self::status_label(file.status);
                        ui.label(&file.name);
                        ui.label(&file.path);
                        ui.label(&file.size);
                        ui.colored_label(color, label);
                        ui.end_row();
                    }
                });
            });

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.heading("Resumen");
                ui.separator();
                ui.label(format!("{} archivos", self.files.len()));
                ui.label(format!(
                    "Última sincronización: hace {} s",
                    self.last_sync.elapsed().as_secs()
                ));
            });

            if !self.conflicts.is_empty() {
                ui.add_space(16.0);
                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), "Hay conflictos pendientes");
            }
        });
    }
}

fn main() -> eframe::Result<()> {
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
