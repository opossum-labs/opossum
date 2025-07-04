use crate::components::{
    context_menu::cx_menu::{ContextMenu, CxtCommand},
    logger::logger_component::Logger,
    menu_bar::menu_bar_component::{MenuBar, MenuSelection},
    node_editor::NodeEditor,
    scenery_editor::{GraphEditor, NodeEditorCommand, NodeElement},
};
use dioxus::prelude::*;

#[component]
pub fn App() -> Element {
    let menu_item_selected = use_signal(|| None::<MenuSelection>);
    let mut node_editor_command = use_signal(|| None::<NodeEditorCommand>);
    let cxt_command = use_signal(|| None::<CxtCommand>);
    let selected_node = use_signal(|| None::<NodeElement>);

    use_effect(move || {
        let cxt_command = cxt_command.read();
        if let Some(cxt_command) = &*(cxt_command) {
            match cxt_command {
                CxtCommand::AddRefNode(new_ref_node) => node_editor_command
                    .set(Some(NodeEditorCommand::AddNodeRef(new_ref_node.clone()))),
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
                MenuSelection::WinMaximize => {
                    println!("App::Window maximize selected");
                }
                MenuSelection::WinMinimize => {
                    println!("App::Window minimize selected");
                }
                MenuSelection::WinClose => {
                    println!("App::Window close selected");
                }
            }
        }
    });
    rsx! {
        ContextMenu { command: cxt_command }
        div { class: "container-fluid text-bg-dark",
            div { class: "row",
                div { class: "col",
                    MenuBar { menu_item_selected }
                }
            }
            div { class: "row main-content-row",
                div { class: "col-2 sidebar",
                    NodeEditor { node: selected_node }
                }
                div { class: "col px-0 graph-editor-container",
                    GraphEditor {
                        command: node_editor_command,
                        node_selected: selected_node,
                    }
                }
            }
            div { class: "row footer",
                div { class: "col", Logger {} }
            }
        }
    }
}
