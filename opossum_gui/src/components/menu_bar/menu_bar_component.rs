#![allow(clippy::derive_partial_eq_without_eq)]
use dioxus::prelude::*;
use dioxus_free_icons::{
    icons::fa_solid_icons::{FaAngleRight, FaBars, FaPowerOff, FaWindowMaximize, FaWindowMinimize},
    Icon,
};
use opossum_backend::AnalyzerType;
use rfd::FileDialog;
use std::path::{Path, PathBuf};

use crate::components::menu_bar::{
    edit::{analyzers_menu::AnalyzersMenu, nodes_menu::NodesMenu},
    help::about::About,
};

const FAVICON: Asset = asset!("./assets/favicon.ico");

#[derive(Debug)]
pub enum MenuSelection {
    NewProject,
    RunProject,
    OpenProject(PathBuf),
    SaveProject(PathBuf),
    AddNode(String),
    AddAnalyzer(AnalyzerType),
    AutoLayout,
    WinMaximize,
    WinMinimize,
    WinClose,
}
#[component]
pub fn MenuBar(menu_item_selected: Signal<Option<MenuSelection>>, project_directory: Signal<PathBuf>) -> Element {
    let mut about_window = use_signal(|| false);
    let node_selected = use_signal(String::new);
    let analyzer_selected = use_signal(|| None::<AnalyzerType>);
    // let window = use_window();
    // let is_dragging = use_signal(|| false);
    // let maximize_symbol = use_signal(|| {
    //     if window.is_maximized() {
    //         "🗗"
    //     } else {
    //         "🗖"
    //     }
    // });
    use_effect(move || {
        if let Some(analyzer) = analyzer_selected() {
            menu_item_selected.set(Some(MenuSelection::AddAnalyzer(analyzer)))
        }
        if !node_selected.read().is_empty() {
            menu_item_selected.set(Some(MenuSelection::AddNode(node_selected())))
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
                                    onclick: move |_| { 
                                        // let path = FileDialog::new()
                                        //     .set_directory("/")
                                        //     .set_title("Pick OPOSSUM project directory")
                                        //     .pick_folder();
                                        // if let Some(path) = path {
                                        //     project_directory.set(path);
                                        //     menu_item_selected.set(Some(MenuSelection::NewProject)) 
                                        // }
                                        menu_item_selected.set(Some(MenuSelection::NewProject)) 

                                    },
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
                                            .set_title("Save OPOSSUM setup file")
                                            .add_filter("Opossum setup file", &["opm"])
                                            .pick_file();
                                        if let Some(path) = path {
                                            // if let Some(path_dir) = path.clone().parent(){
                                            //     project_directory.set(path_dir.to_path_buf().clone());
                                            //     menu_item_selected.set(Some(MenuSelection::OpenProject(path)));
                                            // }
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
                                        let path = FileDialog::new()
                                            .set_directory("/")
                                            .set_title("Save OPOSSUM setup file")
                                            .add_filter("Opossum setup file", &["opm"])
                                            .save_file();
                                        if let Some(path) = path {
                                            // if let Some(path_dir) =  path.clone().parent(){
                                            //     project_directory.set(path_dir.to_path_buf().clone());
                                            //     menu_item_selected.set(Some(MenuSelection::SaveProject(path)));
                                            // }                                                
                                            menu_item_selected.set(Some(MenuSelection::SaveProject(path)));


                                        }
                                    },
                                    "Save Project"
                                }
                            }
                            // li {
                            //     a {
                            //         class: "dropdown-item",
                            //         role: "button",
                            //         onclick: move |_| {
                            //             menu_item_selected.set(Some(MenuSelection::RunProject));

                            //         },
                            //         "Run Project"
                            //     }
                            // }
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
                                a { class: "dropdown-item", role: "button",
                                    "Add Node"
                                    Icon { height: 10, icon: FaAngleRight }
                                }
                                ul { class: "dropdown-menu dropdown-submenu",
                                    NodesMenu { node_selected }
                                }
                            }
                            li {
                                a { class: "dropdown-item", role: "button",
                                    "Add Analyzer"
                                    Icon { height: 10, icon: FaAngleRight }
                                }
                                ul { class: "dropdown-menu dropdown-submenu",
                                    AnalyzersMenu { analyzer_selected }
                                }
                            }
                            li {
                                a {
                                    class: "dropdown-item",
                                    role: "button",
                                    onclick: move |_| {
                                        menu_item_selected.set(Some(MenuSelection::AutoLayout));
                                    },
                                    "Auto Layout"
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
                a {
                    class: "text-secondary me-2",
                    role: "button",
                    onclick: move |_| menu_item_selected.set(Some(MenuSelection::WinMinimize)),
                    Icon { width: 25, icon: FaWindowMinimize }
                }
                a {
                    class: "text-secondary me-2",
                    role: "button",
                    onclick: move |_| menu_item_selected.set(Some(MenuSelection::WinMaximize)),
                    Icon { width: 25, icon: FaWindowMaximize }
                }
                a {
                    class: "text-secondary me-2",
                    role: "button",
                    onclick: move |_| menu_item_selected.set(Some(MenuSelection::WinClose)),
                    Icon { width: 25, icon: FaPowerOff }
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
