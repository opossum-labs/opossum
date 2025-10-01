use crate::{
    api::delete_scenery,
    components::{
        context_menu::cx_menu::{ContextMenu, CxtCommand},
        logger::logger_component::Logger,
        menu_bar::{
            menu_bar_component::{MenuBar, MenuSelection},
            save_project, save_project_as,
        },
        scenery_editor::{GraphEditor, NodeEditorCommand},
        simulation::simulation_window::SimulationWindow,
    },
};
use dioxus::{
    desktop::{tao::window::ResizeDirection, use_window},
    prelude::*,
};
use std::path::PathBuf;

#[component]
pub fn App() -> Element {
    let mut node_editor_command = use_signal(|| None::<NodeEditorCommand>);
    let mut menu_item_selected = use_signal(|| None::<MenuSelection>);
    let cxt_command = use_signal(|| None::<CxtCommand>);
    let mut project_directory: Signal<Option<PathBuf>> = use_signal(|| None);
    let mut model_file_path: Signal<Option<PathBuf>> = use_signal(|| None);
    let mut model_modified: Signal<bool> = use_signal(|| false);
    let mut run_simulation = use_signal(|| false);

    // Get a reference to the window
    let window = use_window();

    use_effect(|| {
        spawn(async move {
            let _ = delete_scenery().await;
        });
    });
    use_effect(move || {
        let cxt_command = cxt_command.read();
        if let Some(cxt_command) = &*(cxt_command) {
            match cxt_command {
                CxtCommand::AddRefNode(new_ref_node) => {
                    node_editor_command.set(Some(NodeEditorCommand::AddNodeRef(*new_ref_node)));
                }
            }
        }
    });
    let window_for_quit = window.clone();
    use_effect(move || {
        let menu_item = menu_item_selected.read();
        if let Some(menu_item) = &*(menu_item) {
            match menu_item {
                MenuSelection::AddNode(node_selected) => {
                    model_modified.set(true);
                    node_editor_command
                        .set(Some(NodeEditorCommand::AddNode(node_selected.clone())));
                }
                MenuSelection::AddAnalyzer(analyzer_selected) => {
                    model_modified.set(true);
                    node_editor_command.set(Some(NodeEditorCommand::AddAnalyzer(
                        analyzer_selected.clone(),
                    )));
                }
                MenuSelection::AutoLayout => {
                    model_modified.set(true);
                    node_editor_command.set(Some(NodeEditorCommand::AutoLayout));
                }

                MenuSelection::CenterGraph { zoom_to_fit } => {
                    node_editor_command.set(Some(NodeEditorCommand::CenterGraph {
                        zoom_to_fit: *zoom_to_fit,
                    }));
                }
                MenuSelection::NewProject => {
                    model_modified.set(true);
                    node_editor_command.set(Some(NodeEditorCommand::DeleteAll));
                }
                MenuSelection::OpenProject(path) => {
                    let path = path.to_owned();
                    node_editor_command.set(Some(NodeEditorCommand::LoadFile(path)));
                    model_modified.set(false);
                }
                MenuSelection::SaveProject(path) => {
                    let path = path.to_owned();
                    node_editor_command.set(Some(NodeEditorCommand::SaveFile(path.clone())));
                    model_file_path.set(Some(path));
                    model_modified.set(false);
                }
                MenuSelection::RunProject => {
                    run_simulation.set(true);
                }
                MenuSelection::SetReportDir(path) => {
                    project_directory.set(Some(path.clone()));
                }
                MenuSelection::Quit => {
                    window_for_quit.close();
                }
            }
        }
    });

    rsx! {
        // The main container for the app and resize handles
        div {
            class: "app-container",
            tabindex: 0,
            onkeydown: move |event| {
                let modifiers = event.modifiers();
                let ctrl_or_meta = modifiers.ctrl() || modifiers.meta();
                if ctrl_or_meta && modifiers.shift()
                    && (event.data().key() == Key::Character("C".to_string())
                        || event.data().key() == Key::Character("c".to_string()))
                {
                    node_editor_command
                        .set(
                            Some(NodeEditorCommand::CenterGraph {
                                zoom_to_fit: false,
                            }),
                        );
                } else if ctrl_or_meta && modifiers.shift()
                    && (event.data().key() == Key::Character("F".to_string())
                        || event.data().key() == Key::Character("f".to_string()))
                {
                    node_editor_command
                        .set(
                            Some(NodeEditorCommand::CenterGraph {
                                zoom_to_fit: true,
                            }),
                        );
                } else if ctrl_or_meta && modifiers.shift()
                    && (event.data().key() == Key::Character("A".to_string())
                        || event.data().key() == Key::Character("a".to_string()))
                {
                    node_editor_command.set(Some(NodeEditorCommand::AutoLayout));
                } else if ctrl_or_meta && event.data().key() == Key::Character("s".to_string()) {
                    save_project(model_file_path, menu_item_selected);
                } else if ctrl_or_meta && modifiers.shift()
                    && (event.data().key() == Key::Character("s".to_string())
                        || event.data().key() == Key::Character("S".to_string()))
                {
                    save_project_as(menu_item_selected);
                }
            },
            // Resize Handles
            div {
                class: "resize-handle-top",
                onmousedown: {
                    let window = window.clone();
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::North);
                    }
                },
            }
            div {
                class: "resize-handle-bottom",
                onmousedown: {
                    let window = window.clone();
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::South);
                    }
                },
            }
            div {
                class: "resize-handle-left",
                onmousedown: {
                    let window = window.clone();
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::West);
                    }
                },
            }
            div {
                class: "resize-handle-right",
                onmousedown: {
                    let window = window.clone();
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::East);
                    }
                },
            }
            div {
                class: "resize-handle-top-left",
                onmousedown: {
                    let window = window.clone();
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::NorthWest);
                    }
                },
            }
            div {
                class: "resize-handle-top-right",
                onmousedown: {
                    let window = window.clone();
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::NorthEast);
                    }
                },
            }
            div {
                class: "resize-handle-bottom-left",
                onmousedown: {
                    let window = window.clone();
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::SouthWest);
                    }
                },
            }
            div {
                class: "resize-handle-bottom-right",
                onmousedown: {
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::SouthEast);
                    }
                },
            }
            ContextMenu { command: cxt_command }
            div { class: "container-fluid text-bg-dark",
                div { class: "row",
                    div { class: "col",
                        MenuBar {
                            menu_item_selected,
                            project_directory,
                            model_file_path,
                            model_modified,
                        }
                    }
                }
                GraphEditor { command: node_editor_command }
                div { class: "row footer",
                    div { class: "col", Logger {} }
                }
                SimulationWindow { show_simulation: run_simulation, project_directory }
            }
        }
    }
}
