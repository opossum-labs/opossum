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
// use crate::{api,HTTP_API_CLIENT, OPOSSUM_UI_LOGS};
// use std::path::PathBuf;
// use opossum_backend::{create_data_dir, create_report_and_data_files};

// pub async fn analyze_setup(path: PathBuf) {
//     match api::analyze(&HTTP_API_CLIENT()).await {
//         Ok(reports) => {
//             if create_data_dir(&path).is_err() {
//                 OPOSSUM_UI_LOGS
//                     .write()
//                     .add_log("Error while creating report-data directory");
//             }
//             // create_dot_file(&opossum_args.report_directory, document.scenery())?;
//             for report in reports.iter().enumerate() {
//                 if create_report_and_data_files(&path, report.1, report.0).is_err() {
//                     OPOSSUM_UI_LOGS
//                         .write()
//                         .add_log("Error while creating report and data files");
//                 }
//             }
//         }
//         Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
//     }
// }

#[component]
pub fn App() -> Element {
    let selected_node = use_signal(|| None::<NodeElement>);
    let graph_state: Signal<GraphState> = use_signal(GraphState::default);
    let graph_processor: Coroutine<GraphStoreAction> =
        use_graph_processor(selected_node, graph_state);
    let menu_item_selected = use_signal(|| None::<MenuSelection>);

    let cxt_command = use_signal(|| None::<CxtCommand>);
    let project_directory = use_signal(|| Path::new("./").to_path_buf());

    // let mut node_editor_command = use_signal(|| None::<NodeEditorCommand>);
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
        // node_editor_command.set(Some(NodeEditorCommand::UpdateActiveNode(selected_node())));
    });

    // use_effect(move || {
    //     if let Some(command) = command.read().as_ref() {
    //         match command {
    //             NodeEditorCommand::UpdateEdges(connect_infos) => {
    //                 graph_processor.send(GraphStoreAction::UpdateEdges(connect_infos.clone()));
    //             }
    //             NodeEditorCommand::DeleteAll => {
    //                 graph_processor.send(GraphStoreAction::DeleteScenery);
    //             }
    //             NodeEditorCommand::AddNode(node_type) => {
    //                 // calculate center of viewport (in graph coordinates)
    //                 let zoom = graph_shift_zoom.peek().zoom;
    //                 let shift = graph_shift_zoom.peek().shift;
    //                 let element_position = (
    //                     (view_port_center.peek().x - shift.x) / zoom,
    //                     (view_port_center.peek().y - shift.y) / zoom,
    //                 );
    //                 let new_node_info = NewNode::new(node_type.to_lowercase(), element_position);
    //                 graph_processor.send(GraphStoreAction::AddOpticNode(new_node_info));
    //             }
    //             NodeEditorCommand::AddNodeRef(new_ref_node) => {
    //                 graph_processor.send(GraphStoreAction::AddOpticReference(*new_ref_node));
    //             }
    //             NodeEditorCommand::AddAnalyzer(analyzer_type) => {
    //                 let new_analyzer_info =
    //                     NewAnalyzerInfo::new(analyzer_type.clone(), (100.0, 100.0));
    //                 graph_processor.send(GraphStoreAction::AddAnalyzer(new_analyzer_info));
    //             }
    //             NodeEditorCommand::AutoLayout => {
    //                 graph_processor.send(GraphStoreAction::OptimizeLayout);
    //             }
    //             NodeEditorCommand::LoadFile(path) => {
    //                 graph_processor.send(GraphStoreAction::LoadFromFile(path.to_owned()));
    //             }
    //             NodeEditorCommand::SaveFile(path) => {
    //                 graph_processor.send(GraphStoreAction::SaveToFile(path.to_owned()));
    //             }
    //             NodeEditorCommand::UpdateActiveNode(node) => {
    //                 graph_processor.send(GraphStoreAction::UpdateActiveNode(node.clone()));
    //             }
    //         }
    //     }
    // });

    use_effect(move || {
        let menu_item = menu_item_selected.read();
        if let Some(menu_item) = &*(menu_item) {
            match menu_item {
                MenuSelection::AddNode(node_type_string) => {
                    // // calculate center of viewport (in graph coordinates)
                    // let zoom = graph_shift_zoom.peek().zoom;
                    // let shift = graph_shift_zoom.peek().shift;
                    // let element_position = (
                    //     (view_port_center.peek().x - shift.x) / zoom,
                    //     (view_port_center.peek().y - shift.y) / zoom,
                    // );
                    // let new_node_info = NewNode::new(node_type.to_lowercase(), element_position);
                    graph_processor.send(GraphStoreAction::AddOpticNode(node_type_string.clone()));

                    // node_editor_command
                    //     .set(Some(NodeEditorCommand::AddNode(node_type_string.clone())));
                }
                MenuSelection::AddAnalyzer(analyzer_type) => {
                    let new_analyzer_info =
                        NewAnalyzerInfo::new(analyzer_type.clone(), (100.0, 100.0));
                    graph_processor.send(GraphStoreAction::AddAnalyzer(new_analyzer_info));
                    // node_editor_command.set(Some(NodeEditorCommand::AddAnalyzer(
                    //     analyzer_selected.clone(),
                    // )));
                }
                MenuSelection::AutoLayout => {
                    graph_processor.send(GraphStoreAction::OptimizeLayout);
                    // node_editor_command.set(Some(NodeEditorCommand::AutoLayout));
                }
                MenuSelection::NewProject => {
                    graph_processor.send(GraphStoreAction::DeleteScenery);

                    // node_editor_command.set(Some(NodeEditorCommand::DeleteAll));
                }
                MenuSelection::OpenProject(path) => {
                    graph_processor.send(GraphStoreAction::LoadFromFile(path.to_owned()));

                    // let path = path.to_owned();
                    // node_editor_command.set(Some(NodeEditorCommand::LoadFile(path)));
                }
                MenuSelection::SaveProject(path) => {
                    graph_processor.send(GraphStoreAction::SaveToFile(path.to_owned()));

                    // let path = path.to_owned();
                    // node_editor_command.set(Some(NodeEditorCommand::SaveFile(path)));
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
                    GraphEditor {
                        graph_state,
                        // command: node_editor_command,
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
