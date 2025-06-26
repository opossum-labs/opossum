use crate::components::scenery_editor::{
    edges::edges_component::{
        EdgeCreation, EdgeCreationComponent, EdgesComponent, NewEdgeCreationStart,
    },
    graph_store::{graph_processor, GraphStore, GraphStoreAction},
    node::NodeElement,
    nodes::Nodes,
};
use dioxus::{
    html::geometry::{euclid::default::Point2D, PixelsSize},
    prelude::*,
};
use opossum_backend::{
    nodes::{ConnectInfo, NewNode, NewRefNode},
    AnalyzerType,
};
use opossum_backend::{scenery::NewAnalyzerInfo, PortType};
use std::path::PathBuf;
use uuid::Uuid;
#[derive(Debug)]
pub enum NodeEditorCommand {
    DeleteAll,
    AddNode(String),
    AddNodeRef(NewRefNode),
    AddAnalyzer(AnalyzerType),
    LoadFile(PathBuf),
    SaveFile(PathBuf),
    AutoLayout,
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
    let graph_store: Signal<GraphStore> = use_signal(GraphStore::default);
    let mut editor_size: Signal<Option<PixelsSize>> = use_signal(|| None);
    let mut editor_status = use_context_provider(|| EditorState {
        drag_status: Signal::new(DragStatus::None),
        edge_in_creation: Signal::new(None),
    });
    let mut graph_shift = use_signal(|| Point2D::<f64>::new(0.0, 0.0));
    let mut graph_zoom = use_signal(|| 1.0);
    let mut current_mouse_pos = use_signal(|| (0.0, 0.0));
    let mut on_mounted = use_signal(|| None);

    let graph_processor: Coroutine<GraphStoreAction> = graph_processor(&graph_store);
    use_context_provider(|| graph_store);
    use_context_provider(|| graph_processor);

    use_effect(move || {
        let command = command.read();
        if let Some(command) = &*(command) {
            match command {
                NodeEditorCommand::DeleteAll => {
                    graph_processor.send(GraphStoreAction::DeleteScenery);
                }
                NodeEditorCommand::AddNode(node_type) => {
                    let new_node_info = NewNode::new(node_type.to_owned(), (100.0, 100.0));
                    graph_processor.send(GraphStoreAction::AddOpticNode(new_node_info));
                }
                NodeEditorCommand::AddNodeRef(new_ref_node) => {
                    let ref_node_clone = new_ref_node.clone();
                    graph_processor.send(GraphStoreAction::AddOpticReference(ref_node_clone));
                }
                NodeEditorCommand::AddAnalyzer(analyzer_type) => {
                    let new_analyzer_info =
                        NewAnalyzerInfo::new(analyzer_type.clone(), (100.0, 100.0));
                    graph_processor.send(GraphStoreAction::AddAnalyzer(new_analyzer_info));
                }
                NodeEditorCommand::AutoLayout => {
                    graph_processor.send(GraphStoreAction::OptimizeLayout);
                }
                NodeEditorCommand::LoadFile(path) => {
                    graph_processor.send(GraphStoreAction::LoadFromFile(path.to_owned()));
                }
                NodeEditorCommand::SaveFile(path) => {
                    graph_processor.send(GraphStoreAction::SaveToFile(path.to_owned()));
                }
            }
        }
    });
    rsx! {
        div {
            class: "graph-editor",
            id: "editor",
            draggable: false,
            onmounted: move |event| { on_mounted.set(Some(event.data)) },
            onwheel: move |wheel_event| {
                async move {
                    if let Ok(rect) = on_mounted().unwrap().get_client_rect().await {
                        let client_pos = wheel_event.data.client_coordinates();
                        let mouse_pos = Point2D::new(
                            client_pos.x - rect.min_x(),
                            client_pos.y - rect.min_y(),
                        );
                        let current_graph_shift = graph_shift();
                        let current_graph_zoom = graph_zoom();
                        let mouse_on_graph_x = (mouse_pos.x - current_graph_shift.x)
                            / current_graph_zoom;
                        let mouse_on_graph_y = (mouse_pos.y - current_graph_shift.y)
                            / current_graph_zoom;
                        let delta = wheel_event.delta().strip_units().y;
                        let new_graph_zoom = if delta > 0.0 {
                            (current_graph_zoom * 1.1).min(2.5)
                        } else {
                            (current_graph_zoom / 1.1).max(0.1)
                        };
                        graph_zoom.set(new_graph_zoom);
                        let new_shift_x = mouse_on_graph_x.mul_add(-new_graph_zoom, mouse_pos.x);
                        let new_shift_y = mouse_on_graph_y.mul_add(-new_graph_zoom, mouse_pos.y);
                        graph_shift.set(Point2D::new(new_shift_x, new_shift_y));
                    }
                }
            },
            onmousedown: move |event| {
                current_mouse_pos
                    .set((event.client_coordinates().x, event.client_coordinates().y));
                editor_status.drag_status.set(DragStatus::Graph);
                node_selected.set(None);
                graph_store.set_active_node_none();
            },
            onmouseup: move |_| {
                let drag_status = editor_status.drag_status.read().clone();
                match drag_status {
                    DragStatus::Node(uuid) => {
                        graph_processor.send(GraphStoreAction::SyncNodePosition(uuid));
                    }
                    DragStatus::Edge(_) => {
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
                                graph_processor.send(GraphStoreAction::AddEdge(new_edge));
                            }
                            editor_status.edge_in_creation.set(None);
                        }
                    }
                    _ => {}
                }
                editor_status.drag_status.set(DragStatus::None);
            },
            onresize: move |event| {
                if let Ok(size) = event.data().get_content_box_size() {
                    editor_size.set(Some(size));
                }
            },
            onmousemove: move |event| {
                let drag_status = &*(editor_status.drag_status.read());
                let rel_shift_x = event.client_coordinates().x - current_mouse_pos().0;
                let rel_shift_y = event.client_coordinates().y - current_mouse_pos().1;
                current_mouse_pos
                    .set((event.client_coordinates().x, event.client_coordinates().y));
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
                        graph_store()
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
            ondoubleclick: move |e| {
                e.stop_propagation();
                let bounding_box = graph_store().get_bounding_box();
                let center = bounding_box.center();
                if let Some(window_size) = editor_size() {
                    let zoom = graph_zoom();
                    let view_center_x = window_size.width / 2.0;
                    let view_center_y = window_size.height / 2.0;
                    graph_shift
                        .set(
                            Point2D::new(
                                center.x.mul_add(-zoom, view_center_x),
                                center.y.mul_add(-zoom, view_center_y),
                            ),
                        );
                }
            },
            div {
                draggable: false,
                pointer_events: "none",
                style: format!(
                    "transform-origin: 0 0; transform: translate({}px, {}px) scale({graph_zoom});",
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
