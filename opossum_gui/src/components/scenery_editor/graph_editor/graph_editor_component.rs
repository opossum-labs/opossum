use std::path::PathBuf;
use crate::components::scenery_editor::{
    edges::edges_component::{
        EdgeCreation, EdgeCreationComponent, EdgesComponent, NewEdgeCreationStart,
    },
    graph_store::GraphStore,
    node::NodeElement,
    nodes::Nodes,
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_backend::{
    nodes::{ConnectInfo, NewNode},
    AnalyzerType,
};
use opossum_backend::{scenery::NewAnalyzerInfo, PortType};
use uuid::Uuid;

#[derive(Debug)]
pub enum NodeEditorCommand {
    DeleteAll,
    AddNode(String),
    AddAnalyzer(AnalyzerType),
    LoadFile(PathBuf),
    SaveFile(PathBuf)
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
    node_selected: Signal<Option<NodeElement>>,
) -> Element {
    // use_context_provider(|| Signal::new(None::<Rc<MountedData>>));
    // use_context_provider(|| Signal::new(None::<EdgeCreation>));
    let mut graph_store = use_context_provider(GraphStore::default);
    let mut editor_status = use_context_provider(|| EditorState {
        drag_status: Signal::new(DragStatus::None),
        edge_in_creation: Signal::new(None),
    });
    let mut graph_shift = use_signal(|| Point2D::<f64>::new(0.0, 0.0));
    let mut graph_zoom = use_signal(|| 1.0);
    let mut current_mouse_pos = use_signal(|| (0, 0));

    use_effect(move || {
        let command = command.read();
        if let Some(command) = &*(command) {
            match command {
                NodeEditorCommand::DeleteAll => {
                    spawn(async move {
                        graph_store.delete_all_nodes().await;
                    });
                }
                NodeEditorCommand::AddNode(node_type) => {
                    let new_node_info = NewNode::new(node_type.to_owned(), (100, 100, 0));
                    spawn(async move {
                        graph_store.add_optic_node(new_node_info).await;
                    });
                }
                NodeEditorCommand::AddAnalyzer(analyzer_type) => {
                    let analyzer_type = analyzer_type.clone();
                    let new_analyzer_info = NewAnalyzerInfo::new(analyzer_type, (100, 100, 0));
                    spawn(async move { graph_store.add_analyzer(new_analyzer_info).await });
                }
                NodeEditorCommand::LoadFile(path) => {
                    let path=path.to_owned();
                    spawn(async move { graph_store.load_from_opm_file(&path).await});
                }
                NodeEditorCommand::SaveFile(path) => {
                     let path=path.to_owned();
                    spawn(async move { graph_store.save_to_opm_file(&path).await});
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
                if delta > 0.0 { graph_zoom *= 1.1 } else { graph_zoom /= 1.1 }
            },
            onmousedown: move |event| {
                current_mouse_pos
                    .set((
                        event.client_coordinates().x as i32,
                        event.client_coordinates().y as i32,
                    ));
                editor_status.drag_status.set(DragStatus::Graph);
            },
            onmouseup: move |_| {
                editor_status.drag_status.set(DragStatus::None);
                let edge_in_creation = editor_status.edge_in_creation.read().clone();
                if let Some(edge_in_creation) = edge_in_creation {
                    if edge_in_creation.is_valid() {
                        let mut start_port = edge_in_creation.start_port();
                        let mut end_port = edge_in_creation.end_port().unwrap();
                        if start_port.port_type == PortType::Input {
                            (start_port, end_port) = (end_port, start_port);
                        }
                        let new_edge = ConnectInfo::new(
                            start_port.node_id,
                            start_port.port_name.clone(),
                            end_port.node_id,
                            end_port.port_name.clone(),
                            0.0,
                        );
                        spawn(async move {
                            graph_store.add_edge(new_edge).await;
                        });
                    }
                    editor_status.edge_in_creation.set(None);
                }
            },
            onmousemove: move |event| {
                let drag_status = &*(editor_status.drag_status.read());
                let rel_shift_x = event.client_coordinates().x - current_mouse_pos().0 as f64;
                let rel_shift_y = event.client_coordinates().y - current_mouse_pos().1 as f64;
                current_mouse_pos
                    .set((
                        event.client_coordinates().x as i32,
                        event.client_coordinates().y as i32,
                    ));
                match drag_status {
                    DragStatus::Graph => {
                        graph_shift
                            .set(
                                Point2D::new(
                                    graph_shift().x + rel_shift_x,
                                    graph_shift().y + rel_shift_y,
                                ),
                            );
                    }
                    DragStatus::Node(id) => {
                        graph_store
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
                    DragStatus::None => {}
                }
            },
            ondoubleclick: move |_| {
                let bounding_box = graph_store.get_bounding_box();
                let center = bounding_box.center();
                graph_shift.set(Point2D::new(-center.x, 250.0 - center.y));
            },
            div {
                class: "zoom-shift-container",
                draggable: false,
                style: format!(
                    "transform: translate({}px, {}px) scale({graph_zoom});",
                    graph_shift().x,
                    graph_shift().y,
                ),
                Nodes { node_activated: node_selected }
                svg {
                    width: "100%",
                    height: "100%",
                    overflow: "visible",
                    tabindex: 0,
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
