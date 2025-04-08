use dioxus::{desktop::use_window, prelude::*};

use crate::components::menu_bar::{
    callbacks::{use_on_double_click, use_on_mouse_down, use_on_mouse_move, use_on_mouse_up},
    controls::controls_menu::ControlsMenu,
    edit::edit_dropdown::EditDropdownMenu,
    file::file_dropdown::FileDropdownMenu,
    help::help_dropdown::HelpDropdownMenu,
};

const FAVICON: Asset = asset!(".\\assets\\favicon.ico");

#[component]
pub fn MenuBar() -> Element {
    let window = use_window();
    let is_dragging = use_signal(|| false);
    let maximize_symbol = use_signal(|| {
        if window.is_maximized() {
            "🗗"
        } else {
            "🗖"
        }
    });
    rsx! {
        div { class: "title-bar",

            div { class: "menu-group",
                img {
                    src: FAVICON,
                    class: "title-bar-item",
                    id: "title-bar-icon",
                }
                FileDropdownMenu {}
                EditDropdownMenu {}
                HelpDropdownMenu {}
            }
            div {
                class: "menu-group",
                id: "menu-window-events",
                onmouseup: use_on_mouse_up(is_dragging),
                onmousedown: use_on_mouse_down(is_dragging),
                onmousemove: use_on_mouse_move(is_dragging),
                ondoubleclick: use_on_double_click(maximize_symbol),
            }
            div { class: "menu-group menu-right",
                ControlsMenu { maximize_symbol }
            }
        }
    }
}
