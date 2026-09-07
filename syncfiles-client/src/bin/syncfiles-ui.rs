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
    activity: Vec<String>,
    view: View,
    onboarding: bool,
    email: String,
    password: String,
    remember_session: bool,
    last_scan: Instant,
    root_input: String,
    scan_error: Option<String>,
    mobile_navigation: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum View {
    Files,
    Activity,
    Conflicts,
    Settings,
}

struct UiFile {
    name: String,
    path: String,
    status: FileState,
    size: String,
    is_dir: bool,
}

#[derive(Clone, Copy)]
enum FileState {
    Local,
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
            sync_root: sync_root.clone(),
            connected: true,
            syncing: false,
            last_sync: Instant::now() - Duration::from_secs(42),
            files: Vec::new(),
            conflicts: Vec::new(),
            activity: vec![
                "Cliente iniciado".to_string(),
                "Sesión local restaurada".to_string(),
                "README.md sincronizado".to_string(),
            ],
            view: View::Files,
            onboarding: false,
            email: String::new(),
            password: String::new(),
            remember_session: true,
            last_scan: Instant::now(),
            root_input: sync_root.display().to_string(),
            scan_error: None,
            mobile_navigation: false,
        }
    }

    fn status_label(status: FileState) -> (&'static str, egui::Color32) {
        match status {
            FileState::Local => ("Local", egui::Color32::from_rgb(120, 150, 220)),
            FileState::Synced => ("Sincronizado", egui::Color32::from_rgb(45, 180, 110)),
            FileState::Pending => ("Pendiente", egui::Color32::from_rgb(230, 170, 45)),
            FileState::Conflict => ("Conflicto", egui::Color32::from_rgb(220, 80, 80)),
        }
    }

    fn sync_now(&mut self) {
        self.syncing = true;
        self.last_sync = Instant::now();
        let mut synced_names = Vec::new();
        for file in &mut self.files {
            if matches!(file.status, FileState::Pending) {
                file.status = FileState::Synced;
                synced_names.push(file.name.clone());
            }
        }
        self.activity
            .extend(synced_names.into_iter().map(|name| format!("{name} sincronizado")));
    }

    fn refresh_files(&mut self) {
        if !self.sync_root.is_dir() {
            let message = format!("La carpeta no existe o no es accesible: {}", self.sync_root.display());
            self.scan_error = Some(message.clone());
            self.files.clear();
            self.activity.insert(0, message);
            return;
        }
        let mut files = Vec::new();
        collect_files(&self.sync_root, &self.sync_root, &mut files);
        files.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
        self.files = files;
        self.last_scan = Instant::now();
        self.scan_error = None;
        self.activity.insert(0, format!("Carpeta inspeccionada: {} elementos", self.files.len()));
    }

    fn apply_root(&mut self, path: PathBuf) {
        self.sync_root = path;
        self.root_input = self.sync_root.display().to_string();
        self.refresh_files();
    }

    fn draw_onboarding(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(55.0);
                ui.heading("Bienvenido a SyncFiles");
                ui.label("Conecta tu cuenta para comenzar a sincronizar tus archivos.");
                ui.add_space(24.0);
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_width(380.0);
                    ui.label("Correo electrónico");
                    ui.text_edit_singleline(&mut self.email);
                    ui.label("Contraseña");
                    ui.add(egui::TextEdit::singleline(&mut self.password).password(true));
                    ui.checkbox(&mut self.remember_session, "Recordar sesión en este dispositivo");
                    ui.add_space(10.0);
                    if ui.button("Iniciar sesión").clicked() {
                        self.onboarding = false;
                        self.connected = true;
                        self.activity.insert(0, "Sesión iniciada".to_string());
                    }
                });
                ui.add_space(16.0);
                ui.small(format!("Servidor: {}", self.server_url));
            });
        });
    }

    fn draw_content(&mut self, ui: &mut egui::Ui) {
        match self.view {
            View::Files => self.draw_files(ui),
            View::Activity => {
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
            View::Conflicts => {
                ui.heading("Conflictos");
                ui.label("Revisa y decide cómo conservar cada versión.");
                ui.add_space(16.0);
                if self.conflicts.is_empty() {
                    ui.label("No hay conflictos pendientes.");
                } else {
                    for conflict in &self.conflicts {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.label(conflict);
                            ui.horizontal(|ui| {
                                ui.button("Conservar local");
                                ui.button("Usar remota");
                                ui.button("Guardar ambas");
                            });
                        });
                    }
                }
            }
            View::Settings => {
                ui.heading("Configuración");
                ui.label("Ajusta la conexión y la carpeta de trabajo.");
                ui.add_space(16.0);
                egui::Grid::new("settings").num_columns(2).show(ui, |ui| {
                    ui.label("Servidor");
                    ui.text_edit_singleline(&mut self.server_url);
                    ui.end_row();
                    ui.label("Carpeta local");
                    ui.text_edit_singleline(&mut self.root_input);
                    ui.end_row();
                    ui.label("Sesión");
                    ui.checkbox(&mut self.remember_session, "Recordar sesión");
                    ui.end_row();
                });
                ui.add_space(16.0);
                if ui.button("Aplicar carpeta y leer contenido").clicked() {
                    self.apply_root(PathBuf::from(self.root_input.trim()));
                }
                #[cfg(not(target_os = "android"))]
                if ui.button("Elegir carpeta...").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_directory(&self.sync_root)
                        .pick_folder()
                    {
                        self.apply_root(path);
                    }
                }
                #[cfg(target_os = "android")]
                ui.label("En Android, escribe la carpeta compartida y pulsa Aplicar.");
                if let Some(error) = &self.scan_error {
                    ui.add_space(8.0);
                    ui.colored_label(egui::Color32::from_rgb(220, 80, 80), error);
                }
                ui.add_space(8.0);
                if ui.button("Probar conexión").clicked() {
                    self.connected = true;
                    self.activity.insert(0, "Conexión verificada".to_string());
                }
            }
        }
    }

    fn draw_mobile_navigation(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (view, label) in [
                (View::Files, "Archivos"),
                (View::Activity, "Actividad"),
                (View::Conflicts, "Conflictos"),
                (View::Settings, "Ajustes"),
            ] {
                if ui.selectable_label(self.view == view, label).clicked() {
                    self.view = view;
                }
            }
        });
    }

    fn draw_files(&mut self, ui: &mut egui::Ui) {
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
        if ui.button("Actualizar contenido de la carpeta").clicked() {
            self.refresh_files();
        }
        ui.small(format!("Última lectura local: hace {} s", self.last_scan.elapsed().as_secs()));
        if let Some(error) = &self.scan_error {
            ui.colored_label(egui::Color32::from_rgb(220, 80, 80), error);
        }
        ui.horizontal(|ui| {
            ui.heading("Resumen");
            ui.separator();
            ui.label(format!("{} archivos", self.files.len()));
            ui.label(format!("Última sincronización: hace {} s", self.last_sync.elapsed().as_secs()));
        });
    }
}

impl eframe::App for SyncFilesUi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.syncing && self.last_sync.elapsed() > Duration::from_millis(700) {
            self.syncing = false;
        }
        ctx.request_repaint_after(Duration::from_millis(100));
        if self.onboarding {
            self.draw_onboarding(ctx);
            return;
        }

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
                if ui.selectable_label(self.view == View::Activity, "  Actividad").clicked() {
                    self.view = View::Activity;
                }
                if ui.selectable_label(self.view == View::Conflicts, format!("  Conflictos ({})", self.conflicts.len())).clicked() {
                    self.view = View::Conflicts;
                }
                ui.separator();
                ui.label(egui::RichText::new("CONFIGURACIÓN").small().strong());
                if ui.selectable_label(self.view == View::Settings, "  Preferencias").clicked() {
                    self.view = View::Settings;
                }
                ui.label("Servidor");
                ui.small(&self.server_url);
                ui.add_space(8.0);
                ui.label("Carpeta local");
                ui.small(self.sync_root.display().to_string());
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(12.0);
            if self.mobile_navigation {
                self.draw_mobile_navigation(ui);
                ui.separator();
            }
            self.draw_content(ui);
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
        Box::new(|_cc| {
            let mut app = SyncFilesUi::new();
            app.refresh_files();
            Ok(Box::new(app))
        }),
    )
}

fn collect_files(root: &PathBuf, current: &PathBuf, files: &mut Vec<UiFile>) {
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
