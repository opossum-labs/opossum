#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::{
    node_editor::NodeConfigEditor,
    scenery_editor::{
        GraphState, GraphStoreAction, NodeType,
        constants::{MAX_ZOOM, MIN_ZOOM},
        edges::edges_component::{
            EdgeCreation, EdgeCreationComponent, EdgesComponent, NewEdgeCreationStart,
        },
        graph_editor::hooks::{
            use_drag, use_drag_end, use_on_key_down, use_on_mouse_down, use_on_resize, use_zoom,
        },
        nodes::Nodes,
        use_graph_processor,
    },
};
use dioxus::{
    html::geometry::{
        Pixels, PixelsSize,
        euclid::{Rect, Size2D, UnknownUnit, default::Point2D},
    },
    prelude::*,
};
use std::{path::PathBuf, rc::Rc, time::Instant};

use opossum_backend::{AnalyzerType, nodes::NewRefNode, scenery::NewAnalyzerInfo};
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
    CenterGraph { zoom_to_fit: bool },
}

#[derive(Clone, Copy)]
pub struct EditorState {
    pub editor_size: Signal<PixelsSize>,
    pub drag_status: Signal<DragStatus>,
    pub edge_in_creation: Signal<Option<EdgeCreation>>,
    pub zoom: Signal<f64>,
    pub shift: Signal<Point2D<f64>>,
    pub rect: Signal<Rect<f64, Pixels>>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            editor_size: Signal::<PixelsSize>::default(),
            drag_status: Signal::<DragStatus>::default(),
            edge_in_creation: Signal::<Option<EdgeCreation>>::default(),
            zoom: Signal::new(1.),
            shift: Signal::<Point2D<f64>>::default(),
            rect: Signal::<Rect<f64, Pixels>>::default(),
        }
    }
}

impl EditorState {
    pub fn get_view_port_center(&self) -> Point2D<f64> {
        let editor_size = *self.editor_size.read();

        Point2D::new(editor_size.width / 2., editor_size.height / 2.)
    }
    pub fn get_view_port_size(&self) -> Size2D<f64, Pixels> {
        *self.editor_size.read()
    }

    pub fn center_graph(&mut self, bounding_box: Rect<f64, UnknownUnit>, zoom_to_fit: bool) {
        if zoom_to_fit {
            self.zoom_to_fit(bounding_box);
        }
        let center = bounding_box.center();
        let zoom = *self.zoom.read();
        let view_center = self.get_view_port_center();
        self.shift.set(Point2D::new(
            center.x.mul_add(-zoom, view_center.x),
            center.y.mul_add(-zoom, view_center.y),
        ));
    }

    fn zoom_to_fit(&mut self, bounding_box: Rect<f64, UnknownUnit>) {
        let padding_fac = 0.95;
        let view_box = self.get_view_port_size();
        let zoom = *self.zoom.read();
        let height_fac = view_box.height * padding_fac / zoom / bounding_box.height();
        let width_fac = view_box.width * padding_fac / zoom / bounding_box.width();
        self.zoom
            .set((zoom * width_fac.min(height_fac)).clamp(MIN_ZOOM, MAX_ZOOM));
    }
}

#[derive(Clone, Debug, Default)]
pub enum DragStatus {
    #[default]
    None,
    Graph,
    Node(Uuid, Point2D<f64>), // stores also old position before drag.
    Edge(NewEdgeCreationStart),
}

#[component]
pub fn GraphEditor(
    mut command: Signal<Option<NodeEditorCommand>>,
    is_modified: Signal<bool>,
) -> Element {
    let last_auxiliary_click = use_signal(|| Option::<Instant>::None);
    let copied_node = use_signal(|| None::<(NodeType, Uuid)>);

    let graph_state: Signal<GraphState> = use_signal(GraphState::default);
    let graph_processor: Coroutine<GraphStoreAction> = use_graph_processor(graph_state);

    let active_node_opt = use_memo(move || {
        graph_state
            .read()
            .graph_store
            .read()
            .get_active_node()
            .map(|n| (n.node_type().clone(), n.id()))
    });

    use_context_provider(|| graph_state().graph_store);
    use_context_provider(|| graph_state().editor_state);

    let current_mouse_pos = use_signal(Point2D::default);
    let mut on_mounted: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    let onwheel_handler = use_zoom(on_mounted);
    let onmousedown_handler = use_on_mouse_down(current_mouse_pos, last_auxiliary_click);
    let onmousemove_handler = use_drag(current_mouse_pos);
    let onmouseup_handler = use_drag_end(is_modified);
    let onmouseleave_handler = use_drag_end(is_modified);
    let onkeydownhandler = use_on_key_down(current_mouse_pos, copied_node);
    let onresizehandler = use_on_resize(on_mounted);

    let shift = use_memo(move || *graph_state.read().editor_state.read().shift.read());
    let zoom = use_memo(move || *graph_state.read().editor_state.read().zoom.read());

    use_effect(move || {
        graph_processor.send(GraphStoreAction::GetSceneryId);
    });

    use_effect(move || {
        if let Some(command) = command.read().as_ref() {
            match command {
                NodeEditorCommand::DeleteAll => {
                    is_modified.set(true);
                    graph_processor.send(GraphStoreAction::DeleteScenery);
                    graph_processor.send(GraphStoreAction::GetSceneryId);
                }
                NodeEditorCommand::AddNode(node_type) => {
                    is_modified.set(true);
                    graph_processor.send(GraphStoreAction::AddOpticNode(node_type.clone()));
                }
                NodeEditorCommand::AddNodeRef(new_ref_node) => {
                    is_modified.set(true);
                    graph_processor.send(GraphStoreAction::AddOpticReference(*new_ref_node));
                }
                NodeEditorCommand::AddAnalyzer(analyzer_type) => {
                    is_modified.set(true);
                    let new_analyzer_info =
                        NewAnalyzerInfo::new(analyzer_type.clone(), (100.0, 100.0));
                    graph_processor.send(GraphStoreAction::AddAnalyzer(new_analyzer_info));
                }
                NodeEditorCommand::AutoLayout => {
                    is_modified.set(true);
                    graph_processor.send(GraphStoreAction::OptimizeLayout);
                    graph_processor.send(GraphStoreAction::CenterGraph { zoom_to_fit: true });
                }
                NodeEditorCommand::CenterGraph { zoom_to_fit } => {
                    graph_processor.send(GraphStoreAction::CenterGraph {
                        zoom_to_fit: *zoom_to_fit,
                    });
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
        div { class: "row main-content-row",
            div { style: "min-width:256px;", class: "col-2 sidebar",
                NodeConfigEditor { active_node_opt }
            }
            div {
                class: "col px-0 graph-editor-container",
                tabindex: 0,
                onkeydown: onkeydownhandler,
                onmouseleave: onmouseleave_handler,
                div {
                    class: "graph-editor",
                    id: "editor",
                    draggable: false,

                    onwheel: onwheel_handler,
                    onmousedown: onmousedown_handler,
                    onmouseup: onmouseup_handler,
                    onmousemove: onmousemove_handler,
                    onresize: onresizehandler,
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
                        Nodes { is_modified }
                        svg {
                            width: "100%",
                            height: "100%",
                            overflow: "visible",
                            tabindex: 0,
                            {
                                rsx! {
                                    EdgesComponent { is_modified }
                                    EdgeCreationComponent {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
