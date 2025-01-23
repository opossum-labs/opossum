#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use log::LevelFilter;
use opossum_gui::gui_app::GuiApp;

fn main() -> eframe::Result {
    egui_logger::builder()
        .max_level(LevelFilter::Debug)
        .init()
        .unwrap();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Opossum",
        options,
        Box::new(|_cc| Ok(Box::<GuiApp>::default())),
    )
}
