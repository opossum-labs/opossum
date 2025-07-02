#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::scenery_editor::{
    edges::edges_component::{
        EdgeCreation, EdgeCreationComponent, EdgesComponent, NewEdgeCreationStart,
    },
    graph_editor::hooks::{use_center_graph, use_drag, use_drag_end, use_drag_start, use_zoom},
    graph_store::{use_graph_processor, GraphStore, GraphStoreAction},
    node::NodeElement,
    nodes::Nodes,
};
use dioxus::{
    html::geometry::{euclid::default::Point2D, PixelsSize},
    prelude::*,
};
use opossum_backend::{
    nodes::{NewNode, NewRefNode},
    scenery::NewAnalyzerInfo,
    AnalyzerType,
};
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
    UpdateActiveNode(Option<NodeElement>),
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
#[derive(Clone, Copy)]
pub struct ShiftZoom {
    pub zoom: f64,
    pub shift: Point2D<f64>,
}

impl ShiftZoom {
    #[must_use]
    pub const fn new(zoom: f64, shift: Point2D<f64>) -> Self {
        Self { zoom, shift }
    }
}
impl Default for ShiftZoom {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            shift: Point2D::default(),
        }
    }
}
#[component]
pub fn GraphEditor(
    command: ReadOnlySignal<Option<NodeEditorCommand>>,
    node_selected: Signal<Option<NodeElement>>,
) -> Element {
    let graph_store: Signal<GraphStore> = use_signal(GraphStore::default);
    let mut editor_size: Signal<Option<PixelsSize>> = use_signal(|| None);
    let editor_status = use_context_provider(|| EditorState {
        drag_status: Signal::new(DragStatus::None),
        edge_in_creation: Signal::new(None),
    });
    let graph_shift_zoom = use_signal(ShiftZoom::default);
    let current_mouse_pos = use_signal(Point2D::default);
    let mut on_mounted: Signal<Option<std::rc::Rc<MountedData>>> = use_signal(|| None);

    let graph_processor: Coroutine<GraphStoreAction> =
        use_graph_processor(&graph_store, node_selected);
    use_context_provider(|| graph_store);
    use_context_provider(|| graph_processor);

    let onwheel_handler = use_zoom(graph_shift_zoom, on_mounted);
    let ondoubleclick_handler = use_center_graph(graph_store, editor_size, graph_shift_zoom);
    let onmousedown_handler = use_drag_start(editor_status, current_mouse_pos);
    let onmousemove_handler = use_drag(
        editor_status,
        current_mouse_pos,
        graph_shift_zoom,
        graph_store,
    );
    let onmouseup_handler = use_drag_end(editor_status, graph_processor);

    let view_port_center = use_memo(move || {
        let size = editor_size();
        size.map_or_else(
            || Point2D::new(0.0, 0.0),
            |size| Point2D::new(size.width / 2.0, size.height / 2.0),
        )
    });
    use_effect(move || {
        if let Some(command) = command.read().as_ref() {
            match command {
                NodeEditorCommand::DeleteAll => {
                    graph_processor.send(GraphStoreAction::DeleteScenery);
                }
                NodeEditorCommand::AddNode(node_type) => {
                    // calculate center of viewport (in graph coordinates)
                    let zoom = graph_shift_zoom.peek().zoom;
                    let shift = graph_shift_zoom.peek().shift;
                    let element_position = (
                        (view_port_center.peek().x - shift.x) / zoom,
                        (view_port_center.peek().y - shift.y) / zoom,
                    );
                    let new_node_info = NewNode::new(node_type.to_lowercase(), element_position);
                    graph_processor.send(GraphStoreAction::AddOpticNode(new_node_info));
                }
                NodeEditorCommand::AddNodeRef(new_ref_node) => {
                    graph_processor.send(GraphStoreAction::AddOpticReference(new_ref_node.clone()));
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
                NodeEditorCommand::UpdateActiveNode(node) => {
                    graph_processor.send(GraphStoreAction::UpdateActiveNode(node.clone()));
                }
            }
        }
    });
    rsx! {
        div {
            class: "graph-editor",
            id: "editor",
            draggable: false,

            onwheel: onwheel_handler,
            onmousedown: onmousedown_handler,
            onmouseup: onmouseup_handler,
            onmousemove: onmousemove_handler,
            ondoubleclick: ondoubleclick_handler,
            onresize: move |event| {
                if let Ok(size) = event.data().get_content_box_size() {
                    editor_size.set(Some(size));
                }
            },
            onmounted: move |event| { on_mounted.set(Some(event.data)) },
            div {
                draggable: false,
                pointer_events: "none",
                style: format!(
                    "transform-origin: 0 0; transform: translate({}px, {}px) scale({});",
                    graph_shift_zoom.read().shift.x,
                    graph_shift_zoom.read().shift.y,
                    graph_shift_zoom.read().zoom,
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
