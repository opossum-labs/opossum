use crate::components::menu_bar::help::about::About;
use dioxus::prelude::*;

#[component]
pub fn HelpDropdownMenu() -> Element {
    let mut about_window = use_signal(|| false);

    rsx! {
        div { class: "title-bar-item dropdown",
            "Help"
            div { class: "dropdown-content title-bar-dropdown-content",
                a {
                    class: "title-bar-submenu-item",
                    onclick: move |_| about_window.set(true),
                    "About"
                }
            }
        }
        {
            if *about_window.read() {
                rsx! {
                    About {} // show_about: about_window }
                }
            } else {
                rsx! {}
            }
        }
    }
}
