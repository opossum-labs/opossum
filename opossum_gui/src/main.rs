#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use env_logger::Env;
use opossum_gui::gui_app::GuiApp;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Opossum",
        options,
        Box::new(|_cc| {
            Ok(Box::<GuiApp>::default())
        }),
    )
}
