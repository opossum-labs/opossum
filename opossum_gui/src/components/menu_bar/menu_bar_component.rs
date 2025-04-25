use dioxus::{desktop::use_window, prelude::*};
use dioxus_free_icons::{
    icons::fa_solid_icons::{FaAngleRight, FaBars, FaWindowMaximize, FaWindowMinimize},
    Icon,
};

use crate::components::menu_bar::{
    callbacks::{use_on_double_click, use_on_mouse_down, use_on_mouse_move, use_on_mouse_up},
    controls::controls_menu::ControlsMenu,
    edit::nodes_menu::NodesMenu,
    file::callbacks::{use_new_project, use_open_project, use_save_project},
    help::about::About,
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
        nav { class: "navbar navbar-expand-sm navbar-dark bg-dark",
            div { class: "container-fluid",
                button {
                    class: "navbar-toggler",
                    "data-mdb-target": "#navbarSupportedContent",
                    "data-mdb-collapse-init": "",
                    Icon { width: 25, icon: FaBars }
                }
                div {
                    class: "collapse navbar-collapse",
                    id: "navbarSupportedContent",
                    img {
                        class: "navbar-brand mt-lg-0",
                        src: FAVICON,
                        height: "40",
                    }
                    ul { class: "navbar-nav me-auto mt-lg-0",
                        li { class: "nav-item dropdown",
                            a {
                                "data-mdb-dropdown-init": "",
                                class: "nav-link dropdown-toggle link-secondary hidden-arrow",
                                id: "navbarDropdownMenuLink",
                                role: "button",
                                "File"
                            }
                            ul { class: "dropdown-menu",
                                li {
                                    a {
                                        class: "dropdown-item",
                                        role: "button",
                                        onclick: move |e| { use_new_project()(e) },
                                        "New Project"
                                    }
                                }
                                li {
                                    a {
                                        class: "dropdown-item",
                                        role: "button",
                                        onclick: move |e| { use_open_project()(e) },
                                        "Open Project"
                                    }
                                }
                                li {
                                    a {
                                        class: "dropdown-item",
                                        role: "button",
                                        onclick: move |e| { use_save_project()(e) },
                                        "Save Project"
                                    }
                                }
                            }
                        }
                        li { class: "nav-item",
                            a {
                                "data-mdb-dropdown-init": "",
                                class: "nav-link dropdown-toggle link-secondary hidden-arrow",
                                id: "navbarDropdownMenuLink",
                                role: "button",
                                "Edit"
                            }
                            ul { class: "dropdown-menu",
                                li {
                                    a {
                                        class: "dropdown-item",
                                        role: "button",
                                        "Add Node"
                                        Icon { height: 10, icon: FaAngleRight }
                                    }
                                    ul { class: "dropdown-menu dropdown-submenu", NodesMenu {} }
                                }
                                li {
                                    a {
                                        class: "dropdown-item",
                                        role: "button",
                                        "Add Analyzer"
                                        Icon { height: 10, icon: FaAngleRight }
                                    }
                                    ul { class: "dropdown-menu dropdown-submenu" }
                                }
                            }
                        }
                        li { class: "nav-item",
                            a {
                                "data-mdb-dropdown-init": "",
                                class: "nav-link dropdown-toggle link-secondary hidden-arrow",
                                id: "navbarDropdownMenuLink",
                                role: "button",
                                "Help"
                            }
                            ul { class: "dropdown-menu",
                                li {
                                    a {
                                        class: "dropdown-item",
                                        role: "button",
                                        onclick: move |_| about_window.set(true),
                                        "About"
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "d-flex align-items-center",
                    a { class: "text-secondary me-2", href: "#",
                        Icon { width: 25, icon: FaWindowMinimize }
                    }
                    a { class: "text-secondary me-2", href: "#",
                        Icon { width: 25, icon: FaWindowMaximize }
                    }
                }
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
