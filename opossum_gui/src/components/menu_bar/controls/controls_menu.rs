#![allow(clippy::derive_partial_eq_without_eq)]
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaPowerOff, FaWindowMaximize, FaWindowMinimize, FaWindowRestore},
};

// Import desktop-specific window hook only when the desktop feature is active
#[cfg(feature = "desktop")]
use dioxus::desktop::use_window;

#[cfg(feature = "desktop")]
#[component]
pub fn ControlsMenu(
    mut maximize_symbol: Signal<Result<VNode, RenderError>>,
    on_quit: EventHandler<()>,
) -> Element {
    let window = use_window();
    rsx! {
        div { class: "menu-group menu-right",
            a {
                class: "text-secondary me-2",
                role: "button",
                onclick: {
                    let window = window.clone();
                    move |_| window.set_minimized(true)
                },
                Icon { width: 25, icon: FaWindowMinimize }
            }
            a {
                class: "text-secondary me-2",
                role: "button",
                onclick: move |_| {

                    if window.is_maximized() {
                        window.set_maximized(false);
                        maximize_symbol.set(rsx! {
                            Icon { width: 25, icon: FaWindowMaximize }
                        });
                    } else {
                        window.set_maximized(true);
                        maximize_symbol.set(rsx! {
                            Icon { width: 25, icon: FaWindowRestore }
                        });
                    }
                },
                {maximize_symbol()}
            }
            a {
                class: "text-secondary me-2",
                role: "button",
                onclick: move |_| on_quit.call(()),
                Icon { width: 25, icon: FaPowerOff }
            }
        }
    }
}

// Fallback implementation when building without the desktop feature (e.g. --renderer native)
#[cfg(not(feature = "desktop"))]
#[component]
pub fn ControlsMenu(
    #[allow(unused_variables)]
    maximize_symbol: Signal<Result<VNode, RenderError>>,
    #[allow(unused_variables)]
    on_quit: EventHandler<()>,
) -> Element {
    rsx! {}
}