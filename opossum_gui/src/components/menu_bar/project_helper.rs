use crate::components::menu_bar::menu_bar_component::MenuSelection;
use dioxus::prelude::*;
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use rfd::AsyncFileDialog;

#[cfg(target_arch = "wasm32")]
use web_sys;

#[allow(clippy::future_not_send)]
pub async fn open_project(mut menu_item_selected: Signal<Option<MenuSelection>>) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let file = AsyncFileDialog::new()
            .set_directory("/")
            .set_title("Open OPOSSUM setup file")
            .add_filter("Opossum setup file", &["opm"])
            .pick_file()
            .await;

        if let Some(handle) = file {
            menu_item_selected.set(Some(MenuSelection::OpenProject(
                handle.path().to_path_buf(),
            )));
        }
    }
    #[cfg(target_arch = "wasm32")]
    {}
}

/// Asks for a path and sends the `SaveProject` signal
/// (Does not work with WASM, beacuse `get_save_file_path()` returns `None`).
#[allow(clippy::future_not_send)]
pub async fn save_project_as(mut menu_item_selected: Signal<Option<MenuSelection>>) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // AsyncFileDialog nutzen + .await
        let file = AsyncFileDialog::new()
            .set_directory("/")
            .set_title("Save OPOSSUM setup file")
            .add_filter("Opossum setup file", &["opm"])
            .save_file()
            .await; // Wartet hier, ohne zu blockieren

        if let Some(handle) = file {
            // handle.path() gibt den Pfad zurück
            menu_item_selected.set(Some(MenuSelection::SaveProject(
                handle.path().to_path_buf(),
            )));
        }
    }
    #[cfg(target_arch = "wasm32")]
    {}
}
/// Saves a project under the given path
/// (Works for WASM only if `model_file_path` exists).
#[allow(clippy::future_not_send)]
pub async fn save_project(
    model_file_path: ReadSignal<Option<PathBuf>>,
    mut menu_item_selected: Signal<Option<MenuSelection>>,
) {
    if let Some(path) = model_file_path() {
        menu_item_selected.set(Some(MenuSelection::SaveProject(path)));
    } else {
        let file = AsyncFileDialog::new()
            .set_directory("/")
            .set_title("Save OPOSSUM setup file")
            .add_filter("Opossum setup file", &["opm"])
            .save_file()
            .await;
        if let Some(handle) = file {
            menu_item_selected.set(Some(MenuSelection::SaveProject(
                handle.path().to_path_buf(),
            )));
        }
    }
}
/// Shows a dialog for selecting the report directory.
#[allow(clippy::future_not_send)]
pub async fn set_report_directory(mut menu_item_selected: Signal<Option<MenuSelection>>) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let folder = AsyncFileDialog::new()
            .set_directory("./")
            .set_title("Select OPOSSUM report directory")
            .pick_folder()
            .await;

        if let Some(handle) = folder {
            menu_item_selected.set(Some(MenuSelection::SetReportDir(
                handle.path().to_path_buf(),
            )));
        }
    }
    #[cfg(target_arch = "wasm32")]
    {}
}
