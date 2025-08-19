use std::path::Path;

use crate::components::{
    context_menu::cx_menu::{ContextMenu, CxtCommand},
    logger::logger_component::Logger,
    menu_bar::menu_bar_component::{MenuBar, MenuSelection},
    node_editor::NodeConfigEditor,
    scenery_editor::{GraphEditor, GraphState, GraphStoreAction, NodeElement, use_graph_processor},
};
use dioxus::prelude::*;
use opossum_backend::scenery::NewAnalyzerInfo;

#[component]
pub fn App() -> Element {
    let selected_node = use_signal(|| None::<NodeElement>);
    let graph_state: Signal<GraphState> = use_signal(GraphState::default);
    let graph_processor: Coroutine<GraphStoreAction> =
        use_graph_processor(selected_node, graph_state);
    let menu_item_selected = use_signal(|| None::<MenuSelection>);

    let cxt_command = use_signal(|| None::<CxtCommand>);
    let project_directory = use_signal(|| Path::new("./").to_path_buf());

    use_effect(move || {
        let cxt_command = cxt_command.read();
        if let Some(cxt_command) = &*(cxt_command) {
            match cxt_command {
                CxtCommand::AddRefNode(new_ref_node) => {
                    graph_processor.send(GraphStoreAction::AddOpticReference(*new_ref_node));
                }
            }
        }
    });

    use_effect(move || {
        graph_processor.send(GraphStoreAction::UpdateActiveNode(selected_node()));
    });

    use_effect(move || {
        let menu_item = menu_item_selected.read();
        if let Some(menu_item) = &*(menu_item) {
            match menu_item {
                MenuSelection::AddNode(node_type_string) => {
                    graph_processor.send(GraphStoreAction::AddOpticNode(node_type_string.clone()));
                }
                MenuSelection::AddAnalyzer(analyzer_type) => {
                    let new_analyzer_info =
                        NewAnalyzerInfo::new(analyzer_type.clone(), (100.0, 100.0));
                    graph_processor.send(GraphStoreAction::AddAnalyzer(new_analyzer_info));
                }
                MenuSelection::AutoLayout => {
                    graph_processor.send(GraphStoreAction::OptimizeLayout);
                }
                MenuSelection::NewProject => {
                    graph_processor.send(GraphStoreAction::DeleteScenery);
                }
                MenuSelection::OpenProject(path) => {
                    graph_processor.send(GraphStoreAction::LoadFromFile(path.to_owned()));
                }
                MenuSelection::SaveProject(path) => {
                    graph_processor.send(GraphStoreAction::SaveToFile(path.to_owned()));
                }
                MenuSelection::WinMaximize => {
                    println!("App::Window maximize selected");
                }
                MenuSelection::WinMinimize => {
                    println!("App::Window minimize selected");
                }
                MenuSelection::WinClose => {
                    println!("App::Window close selected");
                } // MenuSelection::RunProject => {
                  //     spawn(async move { analyze_setup(project_directory()).await });
                  // }
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
            div { class: "row main-content-row",
                div { style: "min-width:256px;", class: "col-2 sidebar",
                    NodeConfigEditor { node_element_sig: selected_node }
                }
                div { class: "col px-0 graph-editor-container",
                    GraphEditor { graph_state, node_selected: selected_node }
                }
            }
            div { class: "row footer",
                div { class: "col", Logger {} }
            }
        }
    }
}
