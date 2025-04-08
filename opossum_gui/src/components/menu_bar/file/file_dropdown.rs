use dioxus::prelude::*;

use crate::components::menu_bar::{
    file::callbacks::{use_new_project, use_open_file, use_open_project, use_save_file},
    sub_menu_item::MenuItem,
};

#[component]
pub fn FileDropdownMenu() -> Element {
    rsx! {
        div { class: "title-bar-item dropdown",
            "File"
            div { class: "title-bar-dropdown-content dropdown-content",
                MenuItem {
                    class: "title-bar-submenu-item".to_owned(),
                    onclick: Some(use_new_project()),
                    display: "New Project",
                }
                MenuItem {
                    class: "title-bar-submenu-item".to_owned(),
                    onclick: Some(use_open_file()),
                    display: "Open File",
                }
                MenuItem {
                    class: "title-bar-submenu-item".to_owned(),
                    onclick: Some(use_open_project()),
                    display: "Open Project",
                }
                MenuItem {
                    class: "title-bar-submenu-item".to_owned(),
                    onclick: Some(use_save_file()),
                    display: "Save File",
                }
            }
        }
    }
}
