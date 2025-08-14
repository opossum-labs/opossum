#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::scenery_editor::{
    GraphState, NodeElement,
    edges::edges_component::{
        EdgeCreation, EdgeCreationComponent, EdgesComponent, NewEdgeCreationStart,
    },
    graph_editor::hooks::{use_center_graph, use_drag, use_drag_end, use_drag_start, use_zoom},
    graph_store::{GraphStore, GraphStoreAction},
    nodes::Nodes,
};
use dioxus::{
    html::geometry::{PixelsSize, euclid::default::Point2D},
    prelude::*,
};
use opossum_backend::{
    AnalyzerType,
    nodes::{ConnectInfo, NewNode, NewRefNode},
    scenery::NewAnalyzerInfo,
};
use std::path::PathBuf;
use uuid::Uuid;
// #[derive(Debug)]
// pub enum NodeEditorCommand {
//     DeleteAll,
//     AddNode(String),
//     AddNodeRef(NewRefNode),
//     AddAnalyzer(AnalyzerType),
//     LoadFile(PathBuf),
//     SaveFile(PathBuf),
//     UpdateEdges(Vec<ConnectInfo>),
//     AutoLayout,
//     UpdateActiveNode(Option<NodeElement>),
// }
#[derive(Clone, Copy)]
pub struct EditorState {
    pub editor_size: Signal<PixelsSize>,
    pub drag_status: Signal<DragStatus>,
    pub edge_in_creation: Signal<Option<EdgeCreation>>,
    pub zoom: Signal<f64>,
    pub shift: Signal<Point2D<f64>>,
}

impl Default for EditorState{
    fn default() -> Self {
        EditorState { editor_size: Default::default(), drag_status: Default::default(), edge_in_creation: Default::default(), zoom: Signal::new(1.), shift: Default::default() }
    }
}

impl EditorState {
    pub fn get_view_port_center(&self) -> Point2D<f64> {
        let editor_size = *self.editor_size.read();

        Point2D::new(editor_size.width / 2., editor_size.height / 2.)
    }
}

#[derive(Clone, Debug, Default)]
pub enum DragStatus {
    #[default]
    None,
    Graph,
    Node(Uuid),
    Edge(NewEdgeCreationStart),
}

#[component]
pub fn GraphEditor(
    graph_state: Signal<GraphState>,
    // command: ReadOnlySignal<Option<NodeEditorCommand>>,
    node_selected: Signal<Option<NodeElement>>,
) -> Element {
    use_context_provider(|| graph_state().graph_store);
    use_context_provider(|| graph_state().editor_state);
    // let graph_processor = use_coroutine_handle::<GraphStoreAction>();

    // let mut editor_size: Signal<Option<PixelsSize>> = use_signal(|| None);
    // let editor_status = use_context_provider(|| EditorState::default());
    // let graph_shift_zoom = use_signal(ShiftZoom::default);
    let current_mouse_pos = use_signal(Point2D::default);
    let mut on_mounted: Signal<Option<std::rc::Rc<MountedData>>> = use_signal(|| None);

    let onwheel_handler = use_zoom(on_mounted);
    let ondoubleclick_handler = use_center_graph(node_selected);
    let onmousedown_handler = use_drag_start(current_mouse_pos, node_selected);
    let onmousemove_handler = use_drag(current_mouse_pos);
    let onmouseup_handler = use_drag_end();

    let shift = use_memo(move || *graph_state.read().editor_state.read().shift.read());
    let zoom = use_memo(move || *graph_state.read().editor_state.read().zoom.read());
    // graph_state.read().editor_state.read().shift.read().y,
    // *graph_state.read().editor_state.read().zoom.read(),

    // let view_port_center = use_memo(move || {
    //     let size = editor_size();
    //     size.map_or_else(
    //         || Point2D::new(0.0, 0.0),
    //         |size| Point2D::new(size.width / 2.0, size.height / 2.0),
    //     )
    // });
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
                    graph_state.write().editor_state.write().editor_size.set(size);
                }
            },
            onmounted: move |event| { on_mounted.set(Some(event.data)) },
            div {
                draggable: false,
                pointer_events: "none",
                style: format!(
                    "transform-origin: 0 0; transform: translate({}px, {}px) scale({});",
                    shift().x,
                    shift().y,
                    zoom(),
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
