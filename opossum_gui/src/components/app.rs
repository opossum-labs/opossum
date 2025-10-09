use crate::{
    api::delete_scenery,
    components::{
        context_menu::cx_menu::{ContextMenu, CxtCommand},
        logger::logger_component::Logger,
        menu_bar::menu_bar_component::{MenuBar, MenuSelection},
        scenery_editor::{GraphEditor, NodeEditorCommand},
        short_cuts::ShortcutHandler,
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
    let mut node_editor_command: Signal<Option<NodeEditorCommand>> =
        use_signal(|| None::<NodeEditorCommand>);
    let cxt_command = use_signal(|| None::<CxtCommand>);

    let menu_item_selected: Signal<Option<MenuSelection>> = use_signal(|| None::<MenuSelection>);
    let mut project_directory: Signal<Option<PathBuf>> = use_signal(|| None);
    let mut model_file_path: Signal<Option<PathBuf>> = use_signal(|| None);
    let mut model_modified: Signal<bool> = use_signal(|| false);

    let short_cut_handler =
        ShortcutHandler::new(menu_item_selected, model_modified, model_file_path);
    use_context_provider(|| short_cut_handler);
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
                    model_file_path.set(None);
                    node_editor_command.set(Some(NodeEditorCommand::DeleteAll));
                }
                MenuSelection::OpenProject(path) => {
                    let path = path.to_owned();
                    node_editor_command.set(Some(NodeEditorCommand::LoadFile(path.clone())));
                    model_file_path.set(Some(path));
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
            onkeydown: move |e| short_cut_handler.handle_event(&e),
            // short_cut_handler(menu_item_selected, model_modified, model_file_path),
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
                GraphEditor { command: node_editor_command, is_modified: model_modified }
                div { class: "row footer",
                    div { class: "col", Logger {} }
                }
                SimulationWindow { show_simulation: run_simulation, project_directory }
            }
        }
    }
}
