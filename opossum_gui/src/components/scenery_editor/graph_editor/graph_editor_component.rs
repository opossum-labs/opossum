use crate::{
    api,
    components::scenery_editor::{
        edges::edges_component::{
            EdgeCreation, EdgeCreationComponent, EdgesComponent, NewEdgeCreationStart,
        },
        nodes::{Nodes, NodesStore},
    },
    HTTP_API_CLIENT, OPOSSUM_UI_LOGS,
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_backend::scenery::NewAnalyzerInfo;
use opossum_backend::{nodes::NewNode, AnalyzerType};
use std::rc::Rc;
use uuid::Uuid;

fn use_init_signals() {
    use_context_provider(|| Signal::new(None::<Rc<MountedData>>));
    use_context_provider(|| Signal::new(None::<EdgeCreation>));
}

#[derive(Debug)]
pub enum NodeEditorCommand {
    DeleteAll,
    AddNode(String),
    AddAnalyzer(AnalyzerType),
}

#[derive(Clone, Copy)]
pub struct EditorState {
    pub drag_status: Signal<DragStatus>,
    pub edge_in_creation: Signal<Option<EdgeCreation>>,
}
#[derive(Clone, Debug)]
pub enum DragStatus {
    None,
    Graph,
    Node(Uuid),
    Edge(NewEdgeCreationStart),
}
#[component]
pub fn GraphEditor(
    command: ReadOnlySignal<Option<NodeEditorCommand>>,
    node_selected: Signal<Option<Uuid>>,
) -> Element {
    use_init_signals();
    let mut node_store = use_context_provider(|| NodesStore::default());
    let mut editor_status = use_context_provider(|| EditorState {
        drag_status: Signal::new(DragStatus::None),
        edge_in_creation: Signal::new(None),
    });
    let mut graph_shift = use_signal(|| (0, 0));
    let mut graph_zoom = use_signal(|| 1.0);
    let mut current_mouse_pos = use_signal(|| (0, 0));

    use_effect(move || {
        let command = command.read();
        if let Some(command) = &*(command) {
            match command {
                NodeEditorCommand::DeleteAll => {
                    println!("NodeEditor: Delete all nodes");
                    // delete_scenery();
                }
                NodeEditorCommand::AddNode(node_type) => {
                    println!("NodeEditor: AddNode: {:?}", node_type);
                    let new_node_info = NewNode::new(node_type.to_owned(), (0, 0, 0));
                    spawn(async move {
                        match api::post_add_node(&HTTP_API_CLIENT(), new_node_info, Uuid::nil())
                            .await
                        {
                            Ok(node_info) => {
                                match api::get_node_properties(&HTTP_API_CLIENT(), node_info.uuid())
                                    .await
                                {
                                    Ok(node_attr) => node_store.add_node(&node_info, &node_attr),
                                    Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
                                }
                            }
                            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
                        }
                    });
                }
                NodeEditorCommand::AddAnalyzer(analyzer_type) => {
                    println!("NodeEditor: AddAnalyzer: {:?}", analyzer_type);
                    let analyzer_type = analyzer_type.clone();
                    let new_analyzer_info = NewAnalyzerInfo::new(analyzer_type.clone(), (0, 0, 0));
                    spawn(async move {
                        match api::post_add_analyzer(&HTTP_API_CLIENT(), new_analyzer_info).await {
                            Ok(_) => {
                                OPOSSUM_UI_LOGS
                                    .write()
                                    .add_log(&format!("Added analyzer: {analyzer_type}"));
                                node_store.add_analyzer(&analyzer_type);
                            }
                            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
                        }
                    });
                }
            }
        }
    });
    rsx! {
        div {
            class: "graph-editor",
            draggable: false,
            // onmounted: use_on_mounted(),
            // onresize: use_on_resize(),
            onwheel: move |event| {
                let delta = event.delta().strip_units().y;
                if delta > 0.0 { graph_zoom *= 1.1 } else { graph_zoom /= 1.1 };
            },
            onmousedown: move |event| {
                println!("Graph mouse down");
                current_mouse_pos
                    .set((
                        event.client_coordinates().x as i32,
                        event.client_coordinates().y as i32,
                    ));
                editor_status.drag_status.set(DragStatus::Graph);
            },
            onmouseup: move |_| {
                editor_status.drag_status.set(DragStatus::None);
                let edge_in_creation=editor_status.edge_in_creation.read().clone();
                if let Some(edge_in_creation) = edge_in_creation {
                   if edge_in_creation.is_valid() {
                    println!("Edge in creation valid");
                   } else {
                    println!("Edge in creation invalid");
                    editor_status.edge_in_creation.set(None);
                   }
                }
            },
            onmousemove: move |event| {
                let drag_status = &*(editor_status.drag_status.read());
                // println!("drag_status: {:?}", drag_status);
                let rel_shift_x = event.client_coordinates().x as i32 - current_mouse_pos().0;
                let rel_shift_y = event.client_coordinates().y as i32 - current_mouse_pos().1;
                current_mouse_pos
                    .set((
                        event.client_coordinates().x as i32,
                        event.client_coordinates().y as i32,
                    ));
                match drag_status {
                    DragStatus::Graph => {
                        graph_shift
                            .set((graph_shift().0 + rel_shift_x, graph_shift().1 + rel_shift_y));
                    }
                    DragStatus::Node(id) => {
                        node_store
                            .shift_node_position(
                                id,
                                Point2D::new(
                                    rel_shift_x as f64 / graph_zoom(),
                                    rel_shift_y as f64 / graph_zoom(),
                                ),
                            );
                    }
                    DragStatus::Edge(edge_creation_start) => {
                        let edge_in_creation = editor_status.edge_in_creation.read().clone();
                        if edge_in_creation.is_none() {
                            let edge_creation = EdgeCreation::new(
                                edge_creation_start.src_node,
                                edge_creation_start.src_port.clone(),
                                edge_creation_start.src_port_type.clone(),
                                edge_creation_start.start_pos,
                            );
                            editor_status.edge_in_creation.set(Some(edge_creation));
                        } else {
                            let mut edge_in_creation = edge_in_creation.unwrap();
                            edge_in_creation
                                .shift_end(
                                    Point2D::new(
                                        rel_shift_x as f64 / graph_zoom(),
                                        rel_shift_y as f64 / graph_zoom(),
                                    ),
                                );
                            editor_status.edge_in_creation.set(Some(edge_in_creation));
                        }
                    }
                    _ => {}
                }
            },
            ondoubleclick: move |_| {
                println!("Graph double click");
            },
            div {
                class: "zoom-shift-container",
                draggable: false,
                style: format!(
                    "transform: translate({}px, {}px) scale({graph_zoom});",
                    graph_shift().0,
                    graph_shift().1,
                ),
                Nodes { node_activated: node_selected }
                svg { width: "100%", height: "100%", overflow: "visible",
                    {
                        rsx! {
                            EdgesComponent {}
                            EdgeCreationComponent {}
                        }
                    }
                }
            }
        }
    }
}
