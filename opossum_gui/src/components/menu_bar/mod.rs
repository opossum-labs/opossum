#[cfg(not(target_arch = "wasm32"))]
pub mod controls;
pub mod edit;
pub mod help;
pub mod menu_bar_component;
// mod path_helper;
mod file_path_display;
pub mod project_helper;

pub use project_helper::{open_project, save_project, save_project_as, set_report_directory};
