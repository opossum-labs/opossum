use dioxus::{desktop::use_window, prelude::*};

use crate::{
    components::menu_bar::{
        callbacks::{use_on_double_click, use_on_mouse_down, use_on_mouse_move, use_on_mouse_up},
        file::callbacks::{use_new_project, use_open_project, use_save_project},
        controls::controls_menu::ControlsMenu,
        edit::edit_dropdown::EditDropdownMenu,
        help::about::About,
    }
};

const FAVICON: Asset = asset!("./assets/favicon.ico");

#[component]
pub fn MenuBar() -> Element {
    let mut about_window = use_signal(|| false);
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
                img {
                    class: "navbar-brand",
                    id: "about-logo",
                    src: FAVICON,
                    height: "40",
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
                                a { class: "dropdown-item", onclick: move |e| {
                                    use_new_project()(e)
                                }, "New Project",  }
                            }
                            li {
                                a { class: "dropdown-item", onclick: move |e| {
                                    use_open_project()(e)
                                }, "Open Project",  }
                            }
                            li {
                                a { class: "dropdown-item", onclick: move |e| {
                                    use_save_project()(e)
                                }, "Save Project",  }
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
                        ul { class: "dropdown-menu",
                            li {
                                a { class: "dropdown-item", "Add Node"  }
                            }
                            li {
                                a { class: "dropdown-item", "Add Analyzer"  }
                            }
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
                                a {
                                    class: "dropdown-item",
                                    onclick: move |_| about_window.set(true),
                                    "About"
                                }
                            }
                        }
                    }
                }
                a { class: "nav-item nav-link text-bg-dark", href: "#", "🗕" }
                a { class: "nav-item nav-link text-bg-dark", href: "#", "🗖" }
                button { class: "btn-close" }
            }
        }
        {
            if *about_window.read() {
                rsx! {
                    About { show_about: about_window } // show_about: about_window
                }
            } else {
                rsx! {}
            }
        }
    }
}
