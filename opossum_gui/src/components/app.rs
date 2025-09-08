use std::path::PathBuf;

use crate::components::{
    context_menu::cx_menu::{ContextMenu, CxtCommand},
    logger::logger_component::Logger,
    menu_bar::menu_bar_component::{MenuBar, MenuSelection},
    scenery_editor::{GraphEditor, NodeEditorCommand},
    simulation::simulation_window::SimulationWindow,
};
use dioxus::prelude::*;

#[component]
pub fn App() -> Element {
    let mut node_editor_command = use_signal(|| None::<NodeEditorCommand>);
    let menu_item_selected = use_signal(|| None::<MenuSelection>);
    let cxt_command = use_signal(|| None::<CxtCommand>);
    let mut project_directory: Signal<Option<PathBuf>> = use_signal(|| None);
    let mut run_simulation = use_signal(|| false);

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

    use_effect(move || {
        let menu_item = menu_item_selected.read();
        if let Some(menu_item) = &*(menu_item) {
            match menu_item {
                MenuSelection::AddNode(node_selected) => {
                    node_editor_command
                        .set(Some(NodeEditorCommand::AddNode(node_selected.clone())));
                }
                MenuSelection::AddAnalyzer(analyzer_selected) => {
                    node_editor_command.set(Some(NodeEditorCommand::AddAnalyzer(
                        analyzer_selected.clone(),
                    )));
                }
                MenuSelection::AutoLayout => {
                    node_editor_command.set(Some(NodeEditorCommand::AutoLayout));
                }
                MenuSelection::NewProject => {
                    node_editor_command.set(Some(NodeEditorCommand::DeleteAll));
                }
                MenuSelection::OpenProject(path) => {
                    let path = path.to_owned();
                    node_editor_command.set(Some(NodeEditorCommand::LoadFile(path)));
                }
                MenuSelection::SaveProject(path) => {
                    let path = path.to_owned();
                    node_editor_command.set(Some(NodeEditorCommand::SaveFile(path)));
                }
                MenuSelection::RunProject => {
                    run_simulation.set(true);
                }
                MenuSelection::SetReportDir(path) => {
                    project_directory.set(Some(path.clone()));
                }
            }
        }
    });
    rsx! {
        ContextMenu { command: cxt_command }
        div { class: "container-fluid text-bg-dark",
            div { class: "row",
                div { class: "col",
                    MenuBar { menu_item_selected, project_directory }
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
