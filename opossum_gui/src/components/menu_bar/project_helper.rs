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

pub fn open_project(mut menu_item_selected: Signal<Option<MenuSelection>>) {
    let path = FileDialog::new()
        .set_directory("/")
        .set_title("Open OPOSSUM setup file")
        .add_filter("Opossum setup file", &["opm"])
        .pick_file();
    if let Some(path) = path {
        menu_item_selected.set(Some(MenuSelection::OpenProject(path)));
    }
}

pub fn set_report_directory(mut menu_item_selected: Signal<Option<MenuSelection>>) {
    let path = FileDialog::new()
        .set_directory("./")
        .set_title("Select OPOSSUM report directory")
        .pick_folder();
    if let Some(path) = path {
        menu_item_selected.set(Some(MenuSelection::SetReportDir(path)));
    }
}
pub fn new_project(
    mut menu_item_selected: Signal<Option<MenuSelection>>,
    model_modified: Signal<bool>,
) {
    let msg = "You have unsaved changes. Are you sure you want to open a new project?";
    if continue_operation(model_modified, msg) {
        menu_item_selected.set(Some(MenuSelection::NewProject));
    }
}

pub fn continue_operation(model_modified: Signal<bool>, msg: &'static str) -> bool {
    if model_modified() {
        let confirm_quit = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("Unsaved Changes")
            .set_description(msg)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        matches!(confirm_quit, rfd::MessageDialogResult::Yes)
    } else {
        true
    }
}
