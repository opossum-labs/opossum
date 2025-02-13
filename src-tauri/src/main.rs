// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, sync::{Arc, Mutex}};

use app_lib::{commands, OPMGUIModel};
use tauri::generate_handler;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
fn main() {
    tauri::Builder::default()
        .invoke_handler(generate_handler![
            commands::add_node,
            commands::get_node_info,
            commands::set_inverted,
            commands::set_name
        ])
        .manage(Mutex::new(OPMGUIModel::new("generic model name")))
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Debug)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
