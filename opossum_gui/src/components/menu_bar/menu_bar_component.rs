use dioxus::{desktop::use_window, prelude::*};

use crate::{
    components::menu_bar::{
        callbacks::{use_on_double_click, use_on_mouse_down, use_on_mouse_move, use_on_mouse_up},
        controls::controls_menu::ControlsMenu,
        edit::edit_dropdown::EditDropdownMenu,
        file::file_dropdown::FileDropdownMenu,
        help::help_dropdown::HelpDropdownMenu,
    },
    router::Route,
};

const FAVICON: Asset = asset!("./assets/favicon.ico");

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
        nav { class: "navbar navbar-expand-sm bg-body-tertiary",
            div { class: "container-fluid",
                Link { to: Route::App,
                    img {
                        class: "navbar-brand",
                        id: "about-logo",
                        src: FAVICON,
                        height: "40",
                    }
                }
                ul { class: "navbar-nav me-auto",
                    li { class: "nav-item dropdown",
                        a {
                            class: "nav-link dropdown-toggle",
                            role: "button",
                            "data-bs-toggle": "dropdown",
                            "File"
                        }
                        ul { class: "dropdown-menu",
                            li {
                                a { class: "dropdown-item", href: "#", "New Project" }
                            }
                            li {
                                a { class: "dropdown-item", href: "#", "Open Project" }
                            }
                            li {
                                a { class: "dropdown-item", href: "#", "Save Project" }
                            }
                        }
                    }
                    li { class: "nav-item dropdown",
                        a {
                            class: "nav-link dropdown-toggle",
                            role: "button",
                            "data-bs-toggle": "dropdown",
                            "Edit"
                        }
                    }
                    li { class: "nav-item dropdown",
                        a {
                            class: "nav-link dropdown-toggle",
                            role: "button",
                            "data-bs-toggle": "dropdown",
                            "Help"
                        }
                        ul { class: "dropdown-menu",
                            li {
                                Link { class: "dropdown-item", to: Route::About, "About" }
                            }
                        }
                    }
                }
                a { class: "nav-item nav-link text-bg-dark", href: "#", "🗕" }
                a { class: "nav-item nav-link text-bg-dark", href: "#", "🗖" }
                button { class: "btn-close" }
            }
        }
        Outlet::<Route> {}
    }
}
