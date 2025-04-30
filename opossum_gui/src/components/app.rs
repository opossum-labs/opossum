use dioxus::prelude::*;
use uuid::Uuid;

use crate::components::{
    logger::logger_component::Logger,
    menu_bar::menu_bar_component::{MenuBar, MenuSelection},
    node_editor::NodeEditor,
    scenery_editor::{graph_editor::NodeEditorCommand, GraphEditor}, zoom_shift_container::zoom_shift_container::ZoomShiftContainer,
};

#[component]
pub fn App() -> Element {
    use_context_provider(|| Signal::new(Uuid::nil()));
    let menu_item_selected = use_signal(|| None::<MenuSelection>);
    let mut node_editor_command = use_signal(|| None::<NodeEditorCommand>);
    let selected_node = use_signal(|| None::<Uuid>);
    // let mut main_window = use_signal(|| None::<Rc<MountedData>>);

    use_effect(move || {
        let menu_item = menu_item_selected.read();
        if let Some(menu_item) = &*(menu_item) {
            match menu_item {
                MenuSelection::AddNode(node_selected) => {
                    println!("Node selected: {:?}", node_selected);
                    node_editor_command
                        .set(Some(NodeEditorCommand::AddNode(node_selected.clone())));
                }
                MenuSelection::AddAnalyzer(analyzer_selected) => {
                    println!("Analyzer selected: {:?}", analyzer_selected);
                    node_editor_command.set(Some(NodeEditorCommand::AddAnalyzer(
                        analyzer_selected.clone(),
                    )));
                }
                MenuSelection::NewProject => {
                    println!("New project selected");
                    node_editor_command.set(Some(NodeEditorCommand::DeleteAll));
                }
                MenuSelection::OpenProject => {
                    println!("Open project selected");
                }
                MenuSelection::SaveProject => {
                    println!("Save project selected");
                }
                MenuSelection::WinMaximize => {
                    println!("Window maximize selected");
                }
                MenuSelection::WinMinimize => {
                    println!("Window minimize selected");
                }
                MenuSelection::WinClose => {
                    println!("Window close selected");
                }
            }
        }
    });

    rsx! {
        div { class: "d-flex flex-column text-bg-dark vh-100",
            div {
                MenuBar { menu_item_selected }
            }
            div { class: "d-flex flex-row",
                div { class: "p-1",
                    NodeEditor { node: selected_node }
                }
                div { class: "p-1 flex-grow-1",
                    GraphEditor {
                        command: node_editor_command,
                        node_selected: selected_node,
                    }
                }
            }
            footer {
                class: "footer p-1",
                style: "background-color:rgb(119, 119, 119);",
                Logger {}
            }
        }
    }
}
