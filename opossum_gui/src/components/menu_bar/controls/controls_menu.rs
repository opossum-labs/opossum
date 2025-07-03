#![allow(clippy::derive_partial_eq_without_eq)]
use dioxus::{desktop::use_window, prelude::*};

use crate::components::context_menu::sub_menu_item::MenuItem;
#[must_use]
pub fn use_maximize(mut maximize_symbol: Signal<&'static str>) -> Callback<Event<MouseData>> {
    let window = use_window();
    use_callback(move |_| {
        if window.is_maximized() {
            maximize_symbol.set("🗖");
            window.set_maximized(false);
        } else {
            maximize_symbol.set("🗗");
            window.set_maximized(true);
        }
    })
}
#[must_use]
pub fn use_close() -> Callback<Event<MouseData>> {
    let window = use_window();
    use_callback(move |_| {
        window.close();
    })
}
#[must_use]
pub fn use_minimize() -> Callback<Event<MouseData>> {
    let window = use_window();
    use_callback(move |_| {
        window.set_minimized(true);
    })
}

#[component]
pub fn ControlsMenu(maximize_symbol: Signal<&'static str>) -> Element {
    rsx! {
        div { class: "menu-group menu-right",
            MenuItem {
                class: "title-bar-item title-bar-control",
                onclick: use_minimize(),
                display: "🗕",
            }
            MenuItem {
                class: "title-bar-item title-bar-control",
                onclick: use_maximize(maximize_symbol),
                display: maximize_symbol(),
            }
            MenuItem {
                class: "title-bar-item title-bar-control",
                onclick: use_close(),
                display: "🗙",
            }
        }
    }
}
