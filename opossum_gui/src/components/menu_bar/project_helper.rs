use crate::components::menu_bar::menu_bar_component::MenuSelection;
use dioxus::prelude::*;
use std::path::PathBuf;

// --- Desktop-spezifische Importe ---
// Importiere 'rfd' nur, wenn das Ziel NICHT wasm32 ist.
#[cfg(not(target_arch = "wasm32"))]
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

// --- WASM-spezifische Importe ---
// Importiere 'web_sys' nur, wenn das Ziel wasm32 ist.
#[cfg(target_arch = "wasm32")]
use web_sys;

/// Eine plattformspezifische Helferfunktion, um einen "Speichern"-Dialog anzuzeigen.
fn get_save_file_path() -> Option<PathBuf> {
    // Desktop-Version verwendet 'rfd'
    #[cfg(not(target_arch = "wasm32"))]
    {
        FileDialog::new()
            .set_directory("/")
            .set_title("Save OPOSSUM setup file")
            .add_filter("Opossum setup file", &["opm"])
            .save_file()
    }

    // WASM-Version: Dies wird im Browser anders gehandhabt (z.B. JS-Interop).
    // Diese Funktion sollte hier 'None' zurückgeben.
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// Fragt nach einem Pfad und sendet ein 'SaveProject'-Kommando.
/// (Funktioniert auf WASM nicht, da get_save_file_path() None zurückgibt).
pub fn save_project_as(mut menu_item_selected: Signal<Option<MenuSelection>>) {
    if let Some(path) = get_save_file_path() {
        menu_item_selected.set(Some(MenuSelection::SaveProject(path)));
    }
}

/// Speichert das Projekt unter dem bestehenden Pfad oder fragt nach einem neuen.
/// (Funktioniert auf WASM nur, wenn bereits ein 'model_file_path' existiert).
pub fn save_project(
    model_file_path: Signal<Option<PathBuf>>,
    mut menu_item_selected: Signal<Option<MenuSelection>>,
) {
    if let Some(path) = model_file_path().or_else(get_save_file_path) {
        menu_item_selected.set(Some(MenuSelection::SaveProject(path)));
    }
}

/// Zeigt einen "Öffnen"-Dialog an, nachdem ungespeicherte Änderungen bestätigt wurden.
pub fn open_project(
    mut menu_item_selected: Signal<Option<MenuSelection>>,
    model_modified: Signal<bool>,
) {
    let msg = "You have unsaved changes. Are you sure you want to open a new project?";
    if continue_operation(*model_modified.peek(), msg) {
        // Desktop-Version verwendet 'rfd'
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = FileDialog::new()
                .set_directory("/")
                .set_title("Open OPOSSUM setup file")
                .add_filter("Opossum setup file", &["opm"])
                .pick_file();
            if let Some(path) = path {
                menu_item_selected.set(Some(MenuSelection::OpenProject(path)));
            }
        }

        // WASM-Version: Das Öffnen von Dateien wird im Browser anders gehandhabt.
        // Diese Funktion tut nichts, da der Aufruf (z.B. von einem Menü-Item)
        // stattdessen JS-Interop auslösen müsste.
        #[cfg(target_arch = "wasm32")]
        {
            // Mache nichts.
        }
    }
}

/// Zeigt einen Dialog zur Auswahl eines Verzeichnisses an.
pub fn set_report_directory(mut menu_item_selected: Signal<Option<MenuSelection>>) {
    // Desktop-Version verwendet 'rfd'
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = FileDialog::new()
            .set_directory("./")
            .set_title("Select OPOSSUM report directory")
            .pick_folder();
        if let Some(path) = path {
            menu_item_selected.set(Some(MenuSelection::SetReportDir(path)));
        }
    }

    // WASM-Version: Das Konzept eines "Report-Verzeichnisses" existiert so im
    // Browser nicht. Mache nichts.
    #[cfg(target_arch = "wasm32")]
    {
        // Mache nichts.
    }
}

/// Startet ein neues Projekt, nachdem ungespeicherte Änderungen bestätigt wurden.
/// (Diese Funktion ist plattformunabhängig, da 'continue_operation'
/// plattformspezifisch implementiert ist).
pub fn new_project(
    mut menu_item_selected: Signal<Option<MenuSelection>>,
    model_modified: Signal<bool>,
) {
    let msg = "You have unsaved changes. Are you sure you want to open a new project?";
    if continue_operation(*model_modified.peek(), msg) {
        menu_item_selected.set(Some(MenuSelection::NewProject));
    }
}

/// Zeigt einen Bestätigungsdialog an, wenn ungespeicherte Änderungen vorhanden sind.
/// Gibt 'true' zurück, wenn der Vorgang fortgesetzt werden soll.
pub fn continue_operation(model_modified: bool, msg: &'static str) -> bool {
    if model_modified {
        // Desktop-Version verwendet 'rfd::MessageDialog'
        #[cfg(not(target_arch = "wasm32"))]
        {
            let confirm_quit = MessageDialog::new()
                .set_level(MessageLevel::Warning)
                .set_title("Unsaved Changes")
                .set_description(msg)
                .set_buttons(MessageButtons::YesNo)
                .show();
            matches!(confirm_quit, MessageDialogResult::Yes)
        }

        // WASM-Version verwendet 'window.confirm()'
        #[cfg(target_arch = "wasm32")]
        {
            web_sys::window()
                .and_then(|win| win.confirm_with_message(msg).ok())
                .unwrap_or(false) // Standardmäßig 'false' (Abbrechen)
        }
    } else {
        // Wenn keine Änderungen vorhanden sind, immer fortfahren.
        true
    }
}
