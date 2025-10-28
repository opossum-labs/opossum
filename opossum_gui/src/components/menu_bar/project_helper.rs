use crate::components::menu_bar::menu_bar_component::MenuSelection;
use dioxus::prelude::*;
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

#[cfg(target_arch = "wasm32")]
use web_sys;

/// Platform-specific helper function for handling a `Save` Dialog
fn get_save_file_path() -> Option<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        FileDialog::new()
            .set_directory("/")
            .set_title("Save OPOSSUM setup file")
            .add_filter("Opossum setup file", &["opm"])
            .save_file()
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// Asks for a path and sends the `SaveProject` signal
/// (Does not work with WASM, beacuse `get_save_file_path()` returns `None`).
pub fn save_project_as(mut menu_item_selected: Signal<Option<MenuSelection>>) {
    if let Some(path) = get_save_file_path() {
        menu_item_selected.set(Some(MenuSelection::SaveProject(path)));
    }
}

/// Saves a project under the given path
/// (Works for WASM only if `model_file_path` exists).
pub fn save_project(
    model_file_path: Signal<Option<PathBuf>>,
    mut menu_item_selected: Signal<Option<MenuSelection>>,
) {
    if let Some(path) = model_file_path().or_else(get_save_file_path) {
        menu_item_selected.set(Some(MenuSelection::SaveProject(path)));
    }
}

/// Shows an `File Open` Dialog.
pub fn open_project(
    mut menu_item_selected: Signal<Option<MenuSelection>>,
    model_modified: Signal<bool>,
) {
    let msg = "You have unsaved changes. Are you sure you want to open a new project?";
    if continue_operation(*model_modified.peek(), msg) {
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

        #[cfg(target_arch = "wasm32")]
        {}
    }
}

/// Shows a dialog for selecting the report directory.
pub fn set_report_directory(mut menu_item_selected: Signal<Option<MenuSelection>>) {
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
    #[cfg(target_arch = "wasm32")]
    {}
}

/// Starts a new project after acknowledging unsaved changes from an earlier session
/// (not platform-indepenedent).
pub fn new_project(
    mut menu_item_selected: Signal<Option<MenuSelection>>,
    model_modified: Signal<bool>,
) {
    let msg = "You have unsaved changes. Are you sure you want to open a new project?";
    if continue_operation(*model_modified.peek(), msg) {
        menu_item_selected.set(Some(MenuSelection::NewProject));
    }
}

/// Shows a dialog (warning) that unsaved changes are present.
pub fn continue_operation(model_modified: bool, msg: &'static str) -> bool {
    if model_modified {
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
        #[cfg(target_arch = "wasm32")]
        {
            web_sys::window()
                .and_then(|win| win.confirm_with_message(msg).ok())
                .unwrap_or(false)
        }
    } else {
        true
    }
}
