use crate::application::services::OrganizerService;
use crate::domain::models::{Action, BracketOrder, EvMode, ExposureInfo, ExposureSettings};
use crate::infrastructure::RawlerMetadataExtractor;
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
        egui::TopBottomPanel::bottom("controls_panel").show(ctx, |ui| {
            self.render_controls(ui);
            ui.add_space(5.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(5.0);
                self.render_folder_selection(ui);
                ui.add_space(5.0);
                ui.separator();
                ui.add_space(5.0);
                self.render_sequence_generation(ui);
                ui.add_space(5.0);
                self.render_exposure_bias_sequence(ui);
                ui.add_space(5.0);
                ui.separator();
                ui.add_space(5.0);
                self.render_filters(ui);
                ui.add_space(5.0);
                self.render_actions(ui);
                ui.add_space(5.0);
                ui.separator();
                ui.add_space(5.0);
                self.render_stats(ui);
                ui.add_space(5.0);
            });
        });

        self.show_exposure_window(ctx);
        self.show_error_messagebox(ctx);

        // Request repaint if running to animate progress bar or update stats
        //if self.running.load(Ordering::Relaxed) {
        //    ctx.request_repaint();
        //}
    }
}

impl ExposureBracketingOrganizerApp {
    fn render_folder_selection(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("📂 Browse...").clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.picked_folder = Some(path.display().to_string());
                }
            }
            if let Some(p) = &self.picked_folder {
                ui.monospace(p);
            } else {
                ui.label(egui::RichText::new("No folder selected").italics().weak());
            }
        });
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new("Note: Files are processed in Natural String Order.")
                .small()
                .weak(),
        );
    }

    fn render_sequence_generation(&mut self, ui: &mut egui::Ui) {
        let mut changed = false;
        egui::Grid::new("sequence_gen_grid")
            .num_columns(2)
            .spacing([10.0, 8.0])
            .show(ui, |ui| {
                ui.label("EV Step:");
                if ui
                    .add(
                        egui::Slider::new(&mut self.exposure_settings.ev_step, 0.1..=5.0)
                            .step_by(0.1)
                            .fixed_decimals(1),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Images:");
                if ui
                    .add(
                        egui::Slider::new(&mut self.exposure_settings.num_images, 3..=9)
                            .step_by(2.0),
                    )
                    .changed()
                {
                    changed = true;
                }
                ui.end_row();

                ui.label("Bracket Order:");
                egui::ComboBox::from_id_salt("bracket_order_selector")
                    .selected_text(self.exposure_settings.bracket_order.to_string())
                    .show_ui(ui, |ui| {
                        changed |= ui
                            .selectable_value(
                                &mut self.exposure_settings.bracket_order,
                                BracketOrder::ZeroMinusPlus,
                                "ZeroMinusPlus",
                            )
                            .changed();
                        changed |= ui
                            .selectable_value(
                                &mut self.exposure_settings.bracket_order,
                                BracketOrder::MinusZeroPlus,
                                "MinusZeroPlus",
                            )
                            .changed();
                    });
                ui.end_row();
            });

        if changed {
            self.exposure_bias_sequence = generate_exposure_sequence(
                self.exposure_settings.ev_step,
                self.exposure_settings.num_images,
                &self.exposure_settings.bracket_order,
            );
        }
    }

    fn render_exposure_bias_sequence(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut self.exposure_bias_sequence);
            egui::ComboBox::from_id_salt("ev_mode_selector")
                .selected_text(self.ev_mode.to_string())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.ev_mode, EvMode::Absolute, "Absolute EV Value");
                    ui.selectable_value(&mut self.ev_mode, EvMode::Delta, "Delta EV Change");
                });
        });
    }

    fn render_filters(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(
            &mut self.filter_by_auto_bracket,
            "Only process files with 'Auto bracket' exposure mode",
        );
    }

    fn render_actions(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Action to perform:");
            egui::ComboBox::from_id_salt("action_selector")
                .selected_text(self.selected_action.to_string())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.selected_action,
                        Action::MoveToFolder,
                        "Move to Folder",
                    );
                    ui.selectable_value(
                        &mut self.selected_action,
                        Action::SaveSequencesToTextfile,
                        "Save Sequences to Textfile",
                    );
                });
        });
    }

    fn render_stats(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("stats_grid")
            .num_columns(2)
            .spacing([20.0, 4.0])
            .show(ui, |ui| {
                ui.label("Exposure bracketings found:");
                ui.label(
                    self.exposure_bracketings_found
                        .load(Ordering::Relaxed)
                        .to_string(),
                );
                ui.end_row();

                ui.label("Files processed:");
                ui.label(self.processed_files.load(Ordering::Relaxed).to_string());
                ui.end_row();
            });
    }

    fn render_controls(&mut self, ui: &mut egui::Ui) {
        // Progress Bar
        let total = self.total_files.load(Ordering::Relaxed);
        let processed = self.processed_files.load(Ordering::Relaxed);
        let is_running = self.running.load(Ordering::Relaxed);

        if total > 0 {
            let fraction = (processed as f32 / total as f32).clamp(0.0, 1.0);
            ui.add(
                egui::ProgressBar::new(fraction)
                    .show_percentage()
                    .animate(is_running),
            );
        } else if is_running {
            ui.add(
                egui::ProgressBar::new(0.0)
                    .text("Scanning...")
                    .animate(true),
            );
        }

        ui.add_space(10.0);

        ui.columns(2, |columns| {
            let button_size = egui::vec2(columns[0].available_width(), 40.0);

            // Start Button
            let start_enabled = !is_running && self.picked_folder.is_some();
            let start_btn = egui::Button::new(egui::RichText::new("▶ Start Processing").size(16.0))
                .min_size(button_size);

            if columns[0].add_enabled(start_enabled, start_btn).clicked() {
                self.start_processing();
            }

            // Get Exposure Bias Button
            let get_bias_btn =
                egui::Button::new(egui::RichText::new("ℹ Get Exposure Bias").size(16.0))
                    .min_size(button_size);

            if columns[1].add(get_bias_btn).clicked() {
                self.get_exposure_bias();
            }
        });
    }

    fn start_processing(&mut self) {
        if let Some(picked_folder) = &self.picked_folder {
            if !self.running.load(Ordering::Relaxed) {
                let folder = picked_folder.clone();
                let total_files = Arc::clone(&self.total_files);
                let processed_files = Arc::clone(&self.processed_files);
                let exposure_bracketings_found = Arc::clone(&self.exposure_bracketings_found);
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

                running.store(true, Ordering::Relaxed);
                total_files.store(0, Ordering::Relaxed);
                processed_files.store(0, Ordering::Relaxed);
                exposure_bracketings_found.store(0, Ordering::Relaxed);

                thread::spawn(move || {
                    let service = OrganizerService::new(RawlerMetadataExtractor::new());
                    let root = PathBuf::from(folder);

                    if root.exists() {
                        let count = service.count_files(&root, &extensions_vec);
                        total_files.store(count, Ordering::Relaxed);

                        service.process_directory(
                            &root,
                            &processed_files,
                            &exposure_bracketings_found,
                            &extensions_vec,
                            &sequence,
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

    fn get_exposure_bias(&mut self) {
        if let Some(mut paths) = rfd::FileDialog::new()
            .add_filter("Raw Images", &self.extensions)
            .pick_files()
        {
            paths.sort_by(|a, b| natord::compare(&a.to_string_lossy(), &b.to_string_lossy()));

            self.exposure_infos.clear();
            let service = OrganizerService::new(RawlerMetadataExtractor::new());

            for path in paths {
                if let Some(info) = service.extract_exposure_info(&path) {
                    self.exposure_infos.push(info);
                }
            }
            self.show_exposure_window = true;
        }
    }

    fn show_exposure_window(&mut self, ctx: &egui::Context) {
        let mut action_to_take: Option<String> = None;

        if self.show_exposure_window {
            let mut is_open = true;

            egui::Window::new("Exposure Bias Information")
                .min_width(300.0)
                .title_bar(true)
                .open(&mut is_open)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("exposure_bias_grid")
                            .striped(true)
                            .num_columns(3)
                            .min_col_width(100.0)
                            .spacing([20.0, 4.0])
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
                                        ui.label(
                                            egui::RichText::new(error).color(egui::Color32::RED),
                                        );
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
                        self.show_error_messagebox = false;
                    }
                });
            if !is_open {
                self.show_error_messagebox = false;
            }
        }
    }
}
