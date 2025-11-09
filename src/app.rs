use eframe::egui;
use log::warn;
use num_rational::Rational32;
use rfd;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::thread;
use crate::file_utils::{count_files_in_directory, extract_raw_metadata, process_directory};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    MoveToFolder,
    SaveSequencesToTextfile,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvMode {
    Absolute,
    Delta,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BracketOrder {
    ZeroMinusPlus,
    MinusZeroPlus,
}

impl std::fmt::Display for BracketOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BracketOrder::ZeroMinusPlus => write!(f, "ZeroMinusPlus"),
            BracketOrder::MinusZeroPlus => write!(f, "MinusZeroPlus"),
        }
    }
}

impl std::fmt::Display for EvMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvMode::Absolute => write!(f, "Absolute EV Value"),
            EvMode::Delta => write!(f, "Delta EV Change"),
        }
    }
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::MoveToFolder => write!(f, "Move to Folder"),
            Action::SaveSequencesToTextfile => write!(f, "Save Sequences to Textfile"),
        }
    }
}
#[derive(Debug)]
pub struct ExposureInfo {
    pub filename: String,
    pub exposure_bias_n: Option<i32>,
    pub exposure_bias_d: Option<i32>,
    pub exposure_mode: Option<u16>,
    pub error_message: Option<String>,
}

pub struct ExposureBracketingOrganizerApp {
    pub picked_folder: Option<String>,
    pub total_files: Arc<AtomicUsize>,
    pub processed_files: Arc<AtomicUsize>,
    pub exposure_bracketings_found: Arc<AtomicUsize>,
    pub running: Arc<AtomicBool>,

    pub extensions: Vec<String>,
    pub exposure_bias_sequence: String,
    pub selected_action: Action,
    pub ev_mode: EvMode,
    pub filter_by_auto_bracket: bool,

    pub show_exposure_window: bool,
    pub exposure_infos: Vec<ExposureInfo>,
    pub show_error_messagebox: bool,
    pub error_messagebox_text: String,

    pub exposure_settings: ExposureSettings,
}

#[derive(Debug, Clone)]
pub struct ExposureSettings {
    pub ev_step: f32,
    pub num_images: u32,
    pub bracket_order: BracketOrder,
}

impl Default for ExposureSettings {
    fn default() -> Self {
        Self {
            ev_step: 1.0,
            num_images: 3,
            bracket_order: BracketOrder::ZeroMinusPlus,
        }
    }
}

impl Default for ExposureBracketingOrganizerApp {
    fn default() -> Self {
        let exposure_settings = ExposureSettings::default();
        let exposure_bias_sequence = generate_exposure_sequence(
            exposure_settings.ev_step,
            exposure_settings.num_images,
            &exposure_settings.bracket_order,
        );

        Self {
            picked_folder: None,
            total_files: Arc::new(AtomicUsize::new(0)),
            processed_files: Arc::new(AtomicUsize::new(0)),
            exposure_bracketings_found: Arc::new(AtomicUsize::new(0)),
            running: Arc::new(AtomicBool::new(false)),

            exposure_bias_sequence,
            selected_action: Action::MoveToFolder,
            ev_mode: EvMode::Delta,
            filter_by_auto_bracket: true,
            extensions: vec![
                "ari".into(),
                "cr3".into(),
                "cr2".into(),
                "crw".into(),
                "erf".into(),
                "raf".into(),
                "3fr".into(),
                "kdc".into(),
                "dcs".into(),
                "dcr".into(),
                "iiq".into(),
                "mos".into(),
                "mef".into(),
                "mrw".into(),
                "nef".into(),
                "nrw".into(),
                "orf".into(),
                "rw2".into(),
                "pef".into(),
                "iiq".into(),
                "srw".into(),
                "arw".into(),
                "srf".into(),
                "sr2".into(),
                "dng".into(),
            ],

            show_exposure_window: false,
            exposure_infos: Vec::new(),
            show_error_messagebox: false,
            error_messagebox_text: "".to_string(),
            exposure_settings,
        }
    }
}

fn parse_exposure_sequence(sequence_str: &str) -> Vec<Rational32> {
    sequence_str
        .split(',')
        .map(|s| s.trim())
        .filter_map(|s| {
            let parts: Vec<&str> = s.split('/').collect();
            if parts.len() == 2 {
                let n: i32 = parts[0].parse().ok()?;
                let d: i32 = parts[1].parse().ok()?;
                if d != 0 {
                    Some(Rational32::new(n, d))
                } else {
                    None
                }
            } else {
                s.parse::<i32>().ok().map(Rational32::from)
            }
        })
        .collect()
}

fn exposure_mode_to_string(mode: u16) -> &'static str {
    match mode {
        0 => "Auto exposure",
        1 => "Manual exposure",
        2 => "Auto bracket",
        _ => "Unknown",
    }
}

fn generate_exposure_sequence(ev_step: f32, num_images: u32, order: &BracketOrder) -> String {
    if num_images == 0 {
        return "".to_string();
    }

    let mut exposures = Vec::new();
    for i in 0..num_images {
        let index = i as i32 - (num_images as i32 - 1) / 2;
        let ev = ev_step * index as f32 * 10.0;
        exposures.push(ev.round() as i32);
    }

    let sequence: Vec<String> = match order {
        BracketOrder::ZeroMinusPlus => {
            let mut seq = vec!["0/10".to_string()];
            for i in 1..=(num_images - 1) / 2 {
                let ev = ev_step * i as f32 * 10.0;
                seq.push(format!("-{}/10", ev.round() as i32));
                seq.push(format!("{}/10", ev.round() as i32));
            }
            seq
        }
        BracketOrder::MinusZeroPlus => {
            let mut sorted_exposures = exposures;
            sorted_exposures.sort();
            sorted_exposures
                .into_iter()
                .map(|ev| format!("{}/10", ev))
                .collect()
        }
    };

    sequence.join(", ")
}

impl eframe::App for ExposureBracketingOrganizerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {

            // Create a grid that acts like a two-column WidgetGallery with 1/3 : 2/3 ratio
            let avail_width = ui.available_width();
            let horizontal_spacing = 16.0_f32;
            // First column takes 1/3 width (minimum 100px)
            let first_col_width = (avail_width * 0.33).max(100.0);

            // Allocate a UI the full available width and place the Grid inside it
            ui.allocate_ui_with_layout(
                egui::vec2(avail_width, 0.0),
                egui::Layout::left_to_right(egui::Align::Min),
                |ui| {
                    egui::Grid::new("widget_gallery_grid")
                        .striped(true)
                        .spacing([horizontal_spacing, 8.0])
                        .min_col_width(first_col_width) // Set minimum width for the first column
                        .num_columns(2) // Explicitly set number of columns
                        .show(ui, |ui| {
                            // Row: Folder picker
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Folder").strong());
                            });
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    if ui.button("Browse…").clicked() {
                                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                            self.picked_folder = Some(path.display().to_string());
                                        }
                                    }
                                    if let Some(p) = &self.picked_folder {
                                        ui.monospace(p);
                                    } else {
                                        ui.label("No folder selected");
                                    }
                                });
                            });
                            ui.end_row();

                            // Row: Generate Exposure Sequence
                            ui.label(egui::RichText::new("Generate Sequence").strong());
                            ui.vertical(|ui| {
                                let mut changed = false;
                                ui.horizontal(|ui| {
                                    ui.label("EV Step:").on_hover_text("Step between each exposure in EV (Exposure Value).");
                                    if ui.add(egui::Slider::new(&mut self.exposure_settings.ev_step, 0.1..=5.0).step_by(0.1).fixed_decimals(1)).changed() {
                                        changed = true;
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Images: ").on_hover_text("Total number of images in the bracket (must be an odd number).");
                                    if ui.add(egui::Slider::new(&mut self.exposure_settings.num_images, 3..=9).step_by(2.0)).changed() {
                                        changed = true;
                                    }
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Bracket Order:");
                                    egui::ComboBox::from_id_salt("bracket_order_selector")
                                        .selected_text(self.exposure_settings.bracket_order.to_string())
                                        .show_ui(ui, |ui| {
                                            changed |= ui.selectable_value(&mut self.exposure_settings.bracket_order, BracketOrder::ZeroMinusPlus, "ZeroMinusPlus").changed();
                                            changed |= ui.selectable_value(&mut self.exposure_settings.bracket_order, BracketOrder::MinusZeroPlus, "MinusZeroPlus").changed();
                                        });
                                });

                                if changed {
                                    self.exposure_bias_sequence = generate_exposure_sequence(
                                        self.exposure_settings.ev_step,
                                        self.exposure_settings.num_images,
                                        &self.exposure_settings.bracket_order,
                                    );
                                }
                            });
                            ui.end_row();

                            // Row: Exposure Bias Sequence
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Exposure Bias Sequence").strong())
                                    .on_hover_text("The Exposure Bias in EXIF is specified as signed rational");
                            });
                            ui.vertical(|ui| {
                                ui.text_edit_singleline(&mut self.exposure_bias_sequence);
                                egui::ComboBox::from_id_salt("ev_mode_selector")
                                    .selected_text(self.ev_mode.to_string())
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.ev_mode, EvMode::Absolute, "Absolute EV Value");
                                        ui.selectable_value(&mut self.ev_mode, EvMode::Delta, "Delta EV Change");
                                    });
                            });
                            ui.end_row();

                            // Row: Filter by Auto Bracket
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Filter").strong());
                            });
                            ui.vertical(|ui| {
                                ui.checkbox(&mut self.filter_by_auto_bracket, "Only 'Auto bracket' exposure mode");
                            });
                            ui.end_row();

                            // Row: Action
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Actions").strong());
                            });
                            ui.vertical(|ui| {
                                egui::ComboBox::from_id_salt("action_selector")
                                    .selected_text(self.selected_action.to_string())
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.selected_action, Action::MoveToFolder, "Move to Folder");
                                        ui.selectable_value(&mut self.selected_action, Action::SaveSequencesToTextfile, "Save Sequences to Textfile");
                                    });
                            });
                            ui.end_row();

                            // Row: Summary counts
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new("Found").strong());
                            });
                            ui.vertical(|ui| {
                                ui.label(format!(
                                    "Exposure bracketings: {}",
                                    self.exposure_bracketings_found.load(Ordering::Relaxed)
                                ));
                                ui.label(format!(
                                    "Files processed: {}",
                                    self.processed_files.load(Ordering::Relaxed)
                                ));
                            });
                            ui.end_row();
                        });
                },
            );

            ui.add_space(12.0);

            // If scanning/processing show a compact status in the central area (progress bar still handled in bottom panel)
            let total = self.total_files.load(Ordering::Relaxed);
            let processed = self.processed_files.load(Ordering::Relaxed);
            let is_running = self.running.load(Ordering::Relaxed);

            if total > 0 {
                let fraction = (processed as f32 / total as f32).clamp(0.0, 1.0);
                ui.horizontal(|ui| {
                    ui.add(egui::ProgressBar::new(fraction).show_percentage());
                });
            } else if is_running {
                ui.label("Scanning files...");
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(8.0); // leave space before bottom panel area
            });
        });

        // Bottom bar: big centered Start button and prettier layout (progress bar and start button are the exception per request)
        egui::TopBottomPanel::bottom("bottom_bar").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal_centered(|ui| {
                let button_size = egui::vec2(140.0, 44.0);

                // Big Start button (only enabled when not already running and folder selected)
                let start_enabled =
                    !self.running.load(Ordering::Relaxed) && self.picked_folder.is_some();
                let btn = egui::Button::new("Start").min_size(button_size).frame(true);
                let response = if start_enabled {
                    ui.add_enabled(true, btn)
                } else {
                    ui.add_enabled(false, btn)
                };

                if response.clicked() && start_enabled {
                    if let Some(picked_folder) = &self.picked_folder {
                        // spawn background processing if not already running
                        if !self.running.load(Ordering::Relaxed) {
                            // clone needed state into the thread
                            let folder = picked_folder.clone();
                            let total_files = Arc::clone(&self.total_files);
                            let processed_files = Arc::clone(&self.processed_files);
                            let exposure_bracketings_found =
                                Arc::clone(&self.exposure_bracketings_found);
                            let running = Arc::clone(&self.running);
                            let extensions_vec: Vec<String> = self.extensions.clone();
                            let exposure_bias_sequence = self.exposure_bias_sequence.clone();
                            let selected_action = self.selected_action.clone();
                            let ev_mode = self.ev_mode.clone();
                            let filter_by_auto_bracket = self.filter_by_auto_bracket;

                            let sequence = parse_exposure_sequence(&exposure_bias_sequence);
                            if sequence.is_empty() || sequence.len() == 1 {
                                self.show_error_messagebox = true;
                                self.error_messagebox_text =
                                    "Invalid or single-value exposure bias sequence.".to_string();
                                return;
                            }

                            // start background work
                            running.store(true, Ordering::Relaxed);
                            total_files.store(0, Ordering::Relaxed);
                            processed_files.store(0, Ordering::Relaxed);
                            exposure_bracketings_found.store(0, Ordering::Relaxed);

                            // Spawn a thread that calls the top-level helpers
                            thread::spawn(move || {
                                let root = PathBuf::from(folder);
                                if root.exists() {
                                    let total = count_files_in_directory(&root, &extensions_vec);
                                    total_files.store(total, Ordering::Relaxed);

                                    process_directory(
                                        &root,
                                        &processed_files,
                                        &exposure_bracketings_found,
                                        extensions_vec,
                                        sequence,
                                        selected_action,
                                        ev_mode,
                                        filter_by_auto_bracket,
                                    );
                                } else {
                                    warn!("Picked folder does not exist: {}", root.display());
                                }

                                running.store(false, Ordering::Relaxed);
                            });
                        }
                    }
                }

                ui.add_space(8.0);

                // Add Get Exposure Bias button
                let get_bias_button = egui::Button::new("Get Exposure Bias")
                    .min_size(button_size)
                    .frame(true);
                if ui.add(get_bias_button).clicked() {
                    if let Some(paths) = rfd::FileDialog::new()
                        .add_filter("Raw Images", &self.extensions)
                        .pick_files()
                    {
                        self.exposure_infos.clear();
                        for path in paths {
                            let filename = path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();

                            let info = if let Some(raw_metadata) =
                                extract_raw_metadata(&path)
                            {
                                let exposure_bias = raw_metadata
                                    .exif
                                    .exposure_bias
                                    .map(|eb| Rational32::new(eb.n, eb.d));
                                let exposure_mode = raw_metadata.exif.exposure_mode;
                                ExposureInfo {
                                    filename,
                                    exposure_bias_n: exposure_bias.map(|eb| *eb.numer()),
                                    exposure_bias_d: exposure_bias.map(|eb| *eb.denom()),
                                    exposure_mode,
                                    error_message: if exposure_bias.is_none() {
                                        Some("No exposure bias found".to_string())
                                    } else {
                                        None
                                    },
                                }
                            } else {
                                ExposureInfo {
                                    filename,
                                    exposure_bias_n: None,
                                    exposure_bias_d: None,
                                    exposure_mode: None,
                                    error_message: Some("Could not read metadata".to_string()),
                                }
                            };
                            self.exposure_infos.push(info);
                        }
                        self.show_exposure_window = true;
                    }
                }
            });
        });

        // Exposure Bias Information window
        self.show_exposure_window(ctx);
        self.show_error_messagebox(ctx);
        ctx.request_repaint();
    }
}

impl ExposureBracketingOrganizerApp {
    fn show_exposure_window(&mut self, ctx: &egui::Context) {
        let mut action_to_take: Option<String> = None;

        if self.show_exposure_window {
            let mut is_open = true;

            egui::Window::new("Exposure Bias Information")
                .min_width(200.0)
                .title_bar(true)
                .open(&mut is_open)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("exposure_bias_grid")
                            .striped(true)
                            .num_columns(3)
                            .min_col_width(100.0)
                            .show(ui, |ui| {
                                // Header
                                ui.strong("Filename");
                                ui.strong("Exposure Bias");
                                ui.strong("Exposure Mode");
                                ui.end_row();

                                // Data rows
                                for info in &self.exposure_infos {
                                    ui.label(&info.filename);

                                    if let Some(error) = &info.error_message {
                                        ui.label(error);
                                    } else if let (Some(n), Some(d)) =
                                        (info.exposure_bias_n, info.exposure_bias_d)
                                    {
                                        ui.label(format!("{}/{}", n, d));
                                    } else {
                                        ui.label("-");
                                    }

                                    if let Some(mode) = info.exposure_mode {
                                        ui.label(exposure_mode_to_string(mode));
                                    } else {
                                        ui.label("-");
                                    }
                                    ui.end_row();
                                }
                            });
                    });

                    ui.add_space(12.0);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui.button("Apply Sequence").clicked() {
                            let mut sequence = String::new();
                            for info in &self.exposure_infos {
                                if let (Some(n), Some(d)) =
                                    (info.exposure_bias_n, info.exposure_bias_d)
                                {
                                    if !sequence.is_empty() {
                                        sequence.push_str(", ");
                                    }
                                    sequence.push_str(&format!("{}/{}", n, d));
                                }
                            }
                            action_to_take = Some(sequence);
                        }
                    });
                });

            if !is_open {
                self.show_exposure_window = false;
            }

            if let Some(sequence) = action_to_take {
                self.exposure_bias_sequence = sequence;
                self.show_exposure_window = false;
            }
        }
    }

    fn show_error_messagebox(&mut self, ctx: &egui::Context) {
        if self.show_error_messagebox {
            let mut is_open = true;
            egui::Window::new("Error")
                .open(&mut is_open)
                .show(ctx, |ui| {
                    ui.label(&self.error_messagebox_text);
                    if ui.button("OK").clicked() {
                        //self.show_.error_messagebox = false;
                    }
                });
            if !is_open {
                self.show_error_messagebox = false;
            }
        }
    }
}
