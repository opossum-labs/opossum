// // Prevents additional console window on Windows in release, DO NOT REMOVE!!
// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// use std::sync::{Arc, Mutex};

// use tauri::generate_handler;
// use app_lib::{commands, OPMGUIModel};

// #[cfg_attr(mobile, tauri::mobile_entry_point)]
// fn main() {
//   tauri::Builder::default()
//     .invoke_handler(generate_handler![commands::add_node, commands::print_beep_boop])
//     .manage(Arc::new(Mutex::new(OPMGUIModel::new("generic model name"))))
//     .setup(|app| {
//       if cfg!(debug_assertions) {
//         app.handle().plugin(
//           tauri_plugin_log::Builder::default()
//             .level(log::LevelFilter::Info)
//             .build(),
//         )?;
//       }
//       Ok(())
//     })
//     .run(tauri::generate_context!())
//     .expect("error while running tauri application");
// }

use tauri::{command, State};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct AppState {
    counter: u32,
}

#[command]
fn increment_counter(state: State<'_, Arc<Mutex<AppState>>>, value: u32) -> Result<u32, String> {
    let mut state = state.lock().map_err(|_| "Fehler beim Sperren des Zustands")?;

    // Inkrementiere den Wert um den übergebenen Wert
    state.counter += value;

    // Gib den neuen Zustand zurück
    Ok(state.counter)
}

fn main() {
    tauri::Builder::default()
        .manage(Arc::new(Mutex::new(AppState::default()))) // Zustand initialisieren und verwalten
        .invoke_handler(tauri::generate_handler![increment_counter]) // Command registrieren
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten der App");
}

