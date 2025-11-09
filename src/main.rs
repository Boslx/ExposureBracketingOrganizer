#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod app;
mod file_utils;

use eframe::egui;

fn main() -> eframe::Result {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([450.0, 450.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Exposure Bracketing Organizer",
        options,
        Box::new(|_cc| Ok(Box::<app::ExposureBracketingOrganizerApp>::default())),
    )
}
