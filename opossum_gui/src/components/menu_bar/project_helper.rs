use crate::components::menu_bar::menu_bar_component::MenuSelection;
use dioxus::prelude::*;
use rfd::FileDialog;
use std::path::PathBuf;

fn get_save_file_path() -> Option<PathBuf> {
    FileDialog::new()
        .set_directory("/")
        .set_title("Save OPOSSUM setup file")
        .add_filter("Opossum setup file", &["opm"])
        .save_file()
}

pub fn save_project_as(mut menu_item_selected: Signal<Option<MenuSelection>>) {
    if let Some(path) = get_save_file_path() {
        menu_item_selected.set(Some(MenuSelection::SaveProject(path)));
    }
}

pub fn save_project(
    model_file_path: Signal<Option<PathBuf>>,
    mut menu_item_selected: Signal<Option<MenuSelection>>,
) {
    if let Some(path) = model_file_path().or_else(get_save_file_path) {
        menu_item_selected.set(Some(MenuSelection::SaveProject(path)));
    }
}
