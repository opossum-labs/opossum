#![allow(clippy::derive_partial_eq_without_eq)]
use dioxus::{document::eval, prelude::*};
use dioxus_free_icons::{
    Icon,
    icons::fa_solid_icons::{FaAngleRight, FaBars, FaWindowMaximize},
};
use opossum_core::prelude::*;
use std::path::PathBuf;

use crate::components::{
    menu_bar::{
        edit::{analyzers_menu::AnalyzersMenu, nodes_menu::NodesMenu},
        file_path_display::FilePathDisplay,
        help::about::About,
    },
    short_cuts::{SHORTCUTS, ShortCutAction},
};

#[cfg(not(target_arch = "wasm32"))]
use crate::components::menu_bar::controls::controls_menu::ControlsMenu;

#[allow(clippy::volatile_composites)]
const FAVICON: Asset = asset!("/assets/favicon.ico");

#[derive(Debug, Clone)]
pub enum AppCommand {
    NewProject,
    OpenTrigger, // start `Open` dialog
    Save,
    SaveAs,
    SetReportDir(PathBuf),
    AddNode(String),
    AddAnalyzer(AnalyzerType),
    AutoLayout,
    CenterGraph { zoom_to_fit: bool },
    Quit,
    Simulate,
}

#[component]
pub fn MenuBar(
    model_file_path_sig: ReadSignal<Option<PathBuf>>,
    model_modified_sig: ReadSignal<bool>,
    on_menu_action: EventHandler<AppCommand>,
) -> Element {
    let mut about_window: Signal<bool> = use_signal(|| false);

    let maximize_symbol: Signal<Result<VNode, RenderError>> = use_signal(|| {
        rsx! {
            Icon { width: 25, icon: FaWindowMaximize }
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
                    // --- File Menu ---
                    li { class: "nav-item dropdown",
                        a {
                            "data-mdb-dropdown-init": "",
                            class: "nav-link dropdown-toggle link-secondary hidden-arrow",
                            id: "navbarDropdownFileMenuLink",
                            "data-mdb-toggle": "dropdown",
                            role: "button",
                            "File"
                        }
                        ul { class: "dropdown-menu",
                            MenuListItemShortCut {
                                short_cut_action: ShortCutAction::New,
                                on_click: move |_| on_menu_action.call(AppCommand::NewProject),
                            }
                            MenuListItemShortCut {
                                short_cut_action: ShortCutAction::Open,
                                on_click: move |_| on_menu_action.call(AppCommand::OpenTrigger),
                            }
                            MenuListItemShortCut {
                                short_cut_action: ShortCutAction::Save,
                                on_click: move |_| on_menu_action.call(AppCommand::Save),
                            }
                            MenuListItemShortCut {
                                short_cut_action: ShortCutAction::SaveAs,
                                on_click: move |_| on_menu_action.call(AppCommand::SaveAs),
                            }
                            MenuListItemShortCut {
                                short_cut_action: ShortCutAction::Report,
                                on_click: move |_| on_menu_action.call(AppCommand::SetReportDir(PathBuf::new())),
                            }
                            li {
                                hr { class: "dropdown-divider" }
                            }
                            MenuListItemShortCut {
                                short_cut_action: ShortCutAction::Quit,
                                on_click: move |_| on_menu_action.call(AppCommand::Quit),
                            }
                        }
                    }
                    // --- Edit Menu  ---
                    li { class: "nav-item dropdown",
                        a {
                            "data-mdb-dropdown-init": "",
                            "data-mdb-toggle": "dropdown",
                            "data-mdb-auto-close": "outside",
                            class: "nav-link dropdown-toggle link-secondary hidden-arrow",
                            id: "navbarDropdownEditMenuLink",
                            role: "button",
                            "Edit"
                        }
                        ul { class: "dropdown-menu",
                            li {
                                a {
                                    class: "dropdown-item d-flex justify-content-between align-items-center",
                                    role: "button",
                                    "Add Node"
                                    Icon { height: 10, icon: FaAngleRight }
                                }
                                ul { class: "dropdown-menu dropdown-submenu custom-scroll",
                                    NodesMenu {
                                        on_node_selected: move |node_name| {
                                            on_menu_action.call(AppCommand::AddNode(node_name));
                                            hide_dropdown("navbarDropdownEditMenuLink");
                                        },
                                    }
                                }
                            }
                            li {
                                a {
                                    class: "dropdown-item d-flex justify-content-between align-items-center",
                                    role: "button",
                                    "Add Analyzer"
                                    Icon { height: 10, icon: FaAngleRight }
                                }
                                ul { class: "dropdown-menu dropdown-submenu custom-scroll",
                                    AnalyzersMenu {
                                        on_analyzer_selected: move |analyzer_type| {
                                            on_menu_action.call(AppCommand::AddAnalyzer(analyzer_type));
                                            hide_dropdown("navbarDropdownEditMenuLink");
                                        },
                                    }
                                }
                            }
                        }
                    }
                    // --- Layout Menu ---
                    li { class: "nav-item dropdown",
                        a {
                            "data-mdb-dropdown-init": "",
                            "data-mdb-toggle": "dropdown",
                            class: "nav-link dropdown-toggle link-secondary hidden-arrow",
                            id: "navbarDropdownLayoutMenuLink",
                            role: "button",
                            "Layout"
                        }
                        ul { class: "dropdown-menu",
                            MenuListItemShortCut {
                                short_cut_action: ShortCutAction::Center,
                                on_click: move |_| {
                                    on_menu_action
                                        .call(AppCommand::CenterGraph {
                                            zoom_to_fit: false,
                                        });
                                },
                            }
                            MenuListItemShortCut {
                                short_cut_action: ShortCutAction::ZoomToFit,
                                on_click: move |_| {
                                    on_menu_action
                                        .call(AppCommand::CenterGraph {
                                            zoom_to_fit: true,
                                        });
                                },
                            }
                            MenuListItemShortCut {
                                short_cut_action: ShortCutAction::AutoLayout,
                                on_click: move |_| on_menu_action.call(AppCommand::AutoLayout),
                            }
                        }
                    }
                    // --- Help Menu  ---
                    li { class: "nav-item dropdown",
                        a {
                            "data-mdb-dropdown-init": "",
                            "data-mdb-toggle": "dropdown",
                            class: "nav-link dropdown-toggle link-secondary hidden-arrow",
                            id: "navbarDropdownHelpMenuLink",
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
                    // Display File Path
                    FilePathDisplay { model_file_path_sig, model_modified_sig }
                }
            }
            ExpandOnClick { maximize_symbol }

            // --- Desktop-specific window controls ---
            {
                let simulate_shortcut = SHORTCUTS
                    .get(&ShortCutAction::Simulate)
                    .map_or(String::new(), |s| format!(" ({s})"));
                #[cfg(not(target_arch = "wasm32"))]
                rsx! {
                    div { class: "d-flex align-items-center",
                        button {
                            class: "btn btn-success me-4",
                            "data-mdb-ripple-init": "",
                            "data-mdb-tooltip-init": "",
                            title: "Simulate{simulate_shortcut}",
                            onclick: move |_| on_menu_action.call(AppCommand::Simulate),
                            "Simulate"
                        }
                        ControlsMenu {
                            maximize_symbol,
                            on_quit: move |()| {
                                on_menu_action.call(AppCommand::Quit);
                            },
                        }
                    }
                }
            }
        }
        if *about_window.read() {
            About { show_about: about_window }
        }
    }
}

fn hide_dropdown(id: &str) {
    let script = format!(
        r"
        const el = document.getElementById('{id}');
        if (el) {{
            const instance = mdb.Dropdown.getInstance(el);
            if (instance) instance.hide();
        }}
    "
    );
    spawn(async move {
        let _ = eval(&script).await;
    });
}

#[component]
fn MenuListItemShortCut(
    short_cut_action: ShortCutAction,
    on_click: EventHandler<MouseEvent>,
) -> Element {
    let short_cut_display = SHORTCUTS
        .get(&short_cut_action)
        .map_or(String::new(), ToString::to_string);
    rsx! {
        li {
            a {
                class: "dropdown-item d-flex justify-content-between align-items-center",
                role: "button",
                onclick: move |evt| on_click.call(evt),
                {format!("{short_cut_action}")}
                span { class: "text-muted ms-4", {short_cut_display} }
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[component]
fn ExpandOnClick(mut maximize_symbol: Signal<Result<VNode, RenderError>>) -> Element {
    use dioxus::desktop::use_window;
    use dioxus_free_icons::icons::fa_solid_icons::FaWindowRestore;
    use std::time::{Duration, Instant};

    let window = use_window();
    let mut last_click = use_signal(|| Option::<Instant>::None);
    let dc_time = Duration::from_millis(300);

    rsx! {
        div {
            class: "d-flex align-items-center flex-grow-1 mx-2 px-2 rounded align-self-stretch my-n2",
            ondragstart: move |e| e.prevent_default(),
            onmousedown: {
                move |_| {
                    let now = Instant::now();
                    let t0_opt = *last_click.read();
                    if t0_opt.is_some_and(|t0| now.duration_since(t0) < dc_time) {
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
                        last_click.set(None);
                    } else {
                        window.drag();
                    }
                    last_click.set(Some(now));
                }
            },
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[component]
fn ExpandOnClick(mut maximize_symbol: Signal<Result<VNode, RenderError>>) -> Element {
    rsx! {}
}
