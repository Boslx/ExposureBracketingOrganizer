#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod application;
mod domain;
mod infrastructure;
mod ui;

use eframe::egui;

fn main() -> eframe::Result {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([450.0, 450.0])
            .with_icon(set_window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Exposure Bracketing Organizer",
        options,
        Box::new(|_cc| Ok(Box::<ui::app::ExposureBracketingOrganizerApp>::default())),
    )
}

pub fn set_window_icon() -> egui::IconData {
    let icon = include_bytes!("../static/favicon.ico");
    let image = image::load_from_memory(icon)
        .expect("Failed to load icon")
        .to_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();
    egui::IconData {
        rgba,
        width,
        height,
    }
}
