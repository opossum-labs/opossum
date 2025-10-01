#![allow(clippy::derive_partial_eq_without_eq)]
use dioxus::{desktop::use_window, prelude::*};
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaAngleRight, FaBars, FaWindowMaximize},
};
use opossum_backend::AnalyzerType;
use rfd::FileDialog;
use std::path::PathBuf;

use crate::components::menu_bar::{
    controls::controls_menu::ControlsMenu,
    edit::{analyzers_menu::AnalyzersMenu, nodes_menu::NodesMenu},
    help::about::About,
    path_helper::abbreviate_path,
};

const FAVICON: Asset = asset!("./assets/favicon.ico");

#[derive(Debug)]
pub enum MenuSelection {
    NewProject,
    RunProject,
    OpenProject(PathBuf),
    SaveProject(PathBuf),
    SetReportDir(PathBuf),
    AddNode(String),
    AddAnalyzer(AnalyzerType),
    AutoLayout,
    CenterGraph{zoom_to_fit:bool},
    Quit,
}
#[component]
pub fn MenuBar(
    menu_item_selected: Signal<Option<MenuSelection>>,
    project_directory: Signal<Option<PathBuf>>,
    model_file_path: Signal<Option<PathBuf>>,
    model_modified: Signal<bool>,
) -> Element {
    let mut about_window: Signal<bool> = use_signal(|| false);
    let node_selected = use_signal(String::new);
    let analyzer_selected = use_signal(|| None::<AnalyzerType>);
    let maximize_symbol: Signal<Result<VNode, RenderError>> = use_signal(|| {
        rsx! {
            Icon { width: 25, icon: FaWindowMaximize }
        }
    });
    use_effect(move || {
        if let Some(analyzer) = analyzer_selected() {
            menu_item_selected.set(Some(MenuSelection::AddAnalyzer(analyzer)));
        }
    });
    use_effect(move || {
        if !node_selected.read().is_empty() {
            menu_item_selected.set(Some(MenuSelection::AddNode(node_selected())));
        }
    });
    rsx! {
        nav { class: "navbar navbar-expand-sm navbar-dark bg-dark",
            button {
                class: "navbar-toggler",
                "data-mdb-target": "#navbarSupportedContent",
                "data-mdb-collapse-init": "",
                Icon { width: 25, icon: FaBars }
            }
            div {
                class: "collapse navbar-collapse flex-grow-0 w-auto",
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
                                    onclick: move |_| { menu_item_selected.set(Some(MenuSelection::NewProject)) },
                                    "New Project"
                                }
                            }
                            li {
                                a {
                                    class: "dropdown-item",
                                    role: "button",
                                    onclick: move |_| {
                                        let path = FileDialog::new()
                                            .set_directory("/")
                                            .set_title("Open OPOSSUM setup file")
                                            .add_filter("Opossum setup file", &["opm"])
                                            .pick_file();
                                        if let Some(path) = path {
                                            menu_item_selected.set(Some(MenuSelection::OpenProject(path)));
                                        }
                                    },
                                    "Open Project"
                                }
                            }
                            li {
                                a {
                                    class: "dropdown-item",
                                    role: "button",
                                    onclick: move |_| {
                                        let path = model_file_path()
                                            .as_ref()
                                            .map_or_else(
                                                || {
                                                    FileDialog::new()
                                                        .set_directory("/")
                                                        .set_title("Save OPOSSUM setup file")
                                                        .add_filter("Opossum setup file", &["opm"])
                                                        .save_file()
                                                },
                                                |model_path| Some(model_path.clone()),
                                            );
                                        if let Some(path) = path {
                                            menu_item_selected.set(Some(MenuSelection::SaveProject(path)));
                                        }
                                    },
                                    "Save Project"
                                }
                            }
                            li {
                                a {
                                    class: "dropdown-item",
                                    role: "button",
                                    onclick: move |_| {
                                        let path = FileDialog::new()
                                            .set_directory("/")
                                            .set_title("Save OPOSSUM setup file")
                                            .add_filter("Opossum setup file", &["opm"])
                                            .save_file();
                                        if let Some(path) = path {
                                            menu_item_selected.set(Some(MenuSelection::SaveProject(path)));
                                        }
                                    },
                                    "Save Project As"
                                }
                            }
                            li {
                                a {
                                    class: "dropdown-item",
                                    role: "button",
                                    onclick: move |_| {
                                        let path = FileDialog::new()
                                            .set_directory("./")
                                            .set_title("Select OPOSSUM report directory")
                                            .pick_folder();
                                        if let Some(path) = path {
                                            menu_item_selected.set(Some(MenuSelection::SetReportDir(path)));
                                        }
                                    },
                                    "Set Report Directory"
                                }
                            }
                        }
                    }
                    li { class: "nav-item dropdown",
                        a {
                            "data-mdb-dropdown-init": "",
                            class: "nav-link dropdown-toggle link-secondary hidden-arrow",
                            id: "navbarDropdownMenuLink",
                            role: "button",
                            "Edit"
                        }
                        ul { class: "dropdown-menu",
                            li {
                                a { class: "dropdown-item d-flex justify-content-between align-items-center",  role: "button",
                                    "Add Node"
                                    Icon {height: 10, icon: FaAngleRight }
                                }
                                ul { class: "dropdown-menu dropdown-submenu",
                                    NodesMenu { node_selected }
                                }
                            }
                            li {
                                a { class: "dropdown-item d-flex justify-content-between align-items-center", role: "button",
                                    "Add Analyzer"
                                    Icon { height: 10, icon: FaAngleRight }
                                }
                                ul { class: "dropdown-menu dropdown-submenu",
                                    AnalyzersMenu { analyzer_selected }
                                }
                            }
                        }
                    }
                    li { class: "nav-item dropdown",
                        a {
                            "data-mdb-dropdown-init": "",
                            class: "nav-link dropdown-toggle link-secondary hidden-arrow",
                            id: "navbarDropdownMenuLink",
                            role: "button",
                            "Layout"
                        }
                        ul { class: "dropdown-menu",
                            li {
                                a { class: "dropdown-item d-flex justify-content-between align-items-center", role: "button",onclick: move |_| {
                                        menu_item_selected.set(Some(MenuSelection::CenterGraph{zoom_to_fit:false}));
                                    },
                                    "Center graph"
                                    span { 
                                        class: "text-muted ms-4", 
                                        "Ctrl+Shift+c" 
                                    }
                                }
                            }
                            li {
                                a { class: "dropdown-item d-flex justify-content-between align-items-center", role: "button",onclick: move |_| {
                                        menu_item_selected.set(Some(MenuSelection::CenterGraph{zoom_to_fit:true}));
                                    },
                                    "Zoom to fit graph"
                                    span { 
                                        class: "text-muted ms-4", 
                                        "Ctrl+Shift+f" 
                                    }
                                }
                            }
                            li {
                                a {
                                    class: "dropdown-item d-flex justify-content-between align-items-center",
                                    role: "button",
                                    onclick: move |_| {
                                        menu_item_selected.set(Some(MenuSelection::AutoLayout));
                                    },
                                    "Auto Layout"
                                    span { 
                                        class: "text-muted ms-4", 
                                        "Ctrl+Shift+a" 
                                    }
                                }
                            }
                        }
                    }
                    li { class: "nav-item dropdown",
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
                            // This is a hack to circumvent layout problems in the menu with only one entry
                            // This li can be removed if further entries are added.
                            li { style: "height: 5px; padding-top: 0; padding-bottom: 0; border: 0;",
                                a {
                                    class: "dropdown-item",
                                    style: "visibility: hidden; pointer-events: none;",
                                }
                            }
                        }
                    }
                    {
                        let (display_path, full_path) = model_file_path()
                            .map_or_else(
                                || (
                                    "unsaved.opm".to_string(),
                                    "this model has not been saved yet".to_string(),
                                ),
                                |path| (abbreviate_path(&path, 40), path.to_string_lossy().to_string()),
                            );
                        let modified_marker = if model_modified() { "*" } else { "" };
                        rsx! {
                            li { class: "nav-item d-flex align-items-center",
                                span { class: "navbar-text text-white-50 ms-3", title: "{full_path}",
                                    "{display_path} {modified_marker}"
                                }
                            }
                        }
                    }
                }
            }
            ExpandOnClick { maximize_symbol }
            div { class: "d-flex align-items-center",
                button {
                    class: "btn btn-success me-4",
                    onclick: move |_| {
                        if project_directory().is_none() {
                            let path = FileDialog::new()
                                .set_directory("./")
                                .set_title("Select OPOSSUM report directory")
                                .pick_folder();
                            if let Some(path) = path {
                                project_directory.set(Some(path));
                                menu_item_selected.set(Some(MenuSelection::RunProject));
                            }
                        } else {
                            menu_item_selected.set(Some(MenuSelection::RunProject));
                        }
                    },
                    "Simulate"
                }
                ControlsMenu {
                    maximize_symbol,
                    on_quit: move || {
                        let should_close = if model_modified() {
                            let confirm_quit = rfd::MessageDialog::new()
                                .set_level(rfd::MessageLevel::Warning)
                                .set_title("Unsaved Changes")
                                .set_description(
                                    "You have unsaved changes. Are you sure you want to quit?",
                                )
                                .set_buttons(rfd::MessageButtons::YesNo)
                                .show();
                            matches!(confirm_quit, rfd::MessageDialogResult::Yes)
                        } else {
                            true
                        };
                        if should_close {
                            menu_item_selected.set(Some(MenuSelection::Quit));
                        }
                    },
                }
            }
        }
        {
            if *about_window.read() {
                rsx! {
                    About { show_about: about_window }
                }
            } else {
                rsx! {}
            }
        }
    }
}

#[cfg(feature = "desktop")]
#[component]
fn ExpandOnClick(mut maximize_symbol: Signal<Result<VNode, RenderError>>) -> Element {
    use dioxus_free_icons::icons::fa_solid_icons::FaWindowRestore;

    let window = use_window();
    let mut is_dragging = use_signal(|| false);
    rsx! {
        div {
            class: "d-flex align-items-center flex-grow-1 mx-2 px-2 rounded align-self-stretch my-n2",
            ondragstart: move |e| e.prevent_default(),
            onmousedown: {
                let window = window.clone();
                move |_| window.drag()
            },
            ondoubleclick: move |_| {
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
        }
    }
}
#[cfg(not(feature = "desktop"))]
#[component]
fn ExpandOnClick(mut maximize_symbol: Signal<Result<VNode, _>>) -> Element {
    rsx! {}
}
