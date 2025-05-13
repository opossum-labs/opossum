use crate::components::{
    logger::logger_component::Logger,
    menu_bar::menu_bar_component::{MenuBar, MenuSelection},
    node_editor::NodeEditor,
    scenery_editor::{graph_editor::NodeEditorCommand, node::NodeElement, GraphEditor},
};
use dioxus::prelude::*;

#[component]
pub fn App() -> Element {
    let menu_item_selected = use_signal(|| None::<MenuSelection>);
    let mut node_editor_command = use_signal(|| None::<NodeEditorCommand>);
    let selected_node = use_signal(|| None::<NodeElement>);
    // let mut main_window = use_signal(|| None::<Rc<MountedData>>);

    use_effect(move || {
        let menu_item = menu_item_selected.read();
        if let Some(menu_item) = &*(menu_item) {
            match menu_item {
                MenuSelection::AddNode(node_selected) => {
                    println!("App::Node selected: {node_selected}");
                    node_editor_command
                        .set(Some(NodeEditorCommand::AddNode(node_selected.clone())));
                }
                MenuSelection::AddAnalyzer(analyzer_selected) => {
                    println!("App::Analyzer selected: {analyzer_selected}");
                    node_editor_command.set(Some(NodeEditorCommand::AddAnalyzer(
                        analyzer_selected.clone(),
                    )));
                }
                MenuSelection::NewProject => {
                    println!("App::New project selected");
                    node_editor_command.set(Some(NodeEditorCommand::DeleteAll));
                }
                MenuSelection::OpenProject => {
                    println!("App::Open project selected");
                }
                MenuSelection::SaveProject => {
                    println!("App::Save project selected");
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
        div { class: "container-fluid text-bg-dark",
            div { class: "row",
                div { class: "col",
                    MenuBar { menu_item_selected }
                }
            }
            div { class: "row",
                div { class: "col-2",
                    NodeEditor { node: selected_node }
                }
                div { class: "col",
                    GraphEditor {
                        command: node_editor_command,
                        node_selected: selected_node,
                    }
                }
            }
            div { class: "row",
                div { class: "col",
                    footer {
                        class: "footer p-1",
                        style: "background-color:rgb(119, 119, 119);",
                        Logger {}
                    }
                }
            }
        }
    }
}
