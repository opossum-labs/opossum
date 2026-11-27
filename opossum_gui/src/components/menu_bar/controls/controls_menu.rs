// src/components/menu_bar/controls/controls_menu.rs
use dioxus::{desktop::use_window, prelude::*};
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaPowerOff, FaWindowMaximize, FaWindowMinimize, FaWindowRestore},
};

#[cfg(feature = "desktop")]
#[component]
pub fn ControlsMenu(
    mut maximize_symbol: Signal<Result<VNode, RenderError>>,
    on_quit: EventHandler<()>,
) -> Element {
    let window = use_window();
    rsx! {
        div { class: "d-flex align-items-center flex-nowrap flex-shrink-0 menu-group menu-right",
            a {
                class: "text-secondary me-2 d-flex align-items-center",
                role: "button",
                title: "Minimize",
                onclick: {
                    let window = window.clone();
                    move |_| window.set_minimized(true)
                },
                Icon { width: 22, icon: FaWindowMinimize }
            }
            a {
                class: "text-secondary me-2 d-flex align-items-center",
                role: "button",
                title: "Maximize / Restore",
                onclick: move |_| {
                    if window.is_maximized() {
                        window.set_maximized(false);
                        maximize_symbol.set(rsx! {
                            Icon { width: 22, icon: FaWindowMaximize }
                        });
                    } else {
                        window.set_maximized(true);
                        maximize_symbol.set(rsx! {
                            Icon { width: 22, icon: FaWindowRestore }
                        });
                    }
                },
                {maximize_symbol()}
            }
            a {
                class: "text-secondary me-2 d-flex align-items-center",
                role: "button",
                title: "Quit",
                onclick: move |_| on_quit.call(()),
                Icon { width: 22, icon: FaPowerOff }
            }
        }
    }
}

#[cfg(not(feature = "desktop"))]
#[component]
pub fn ControlsMenu() -> Element {
    rsx! {}
}
