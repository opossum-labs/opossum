#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::scenery_editor::{
    GraphState, NodeElement,
    edges::edges_component::{
        EdgeCreation, EdgeCreationComponent, EdgesComponent, NewEdgeCreationStart,
    },
    graph_editor::hooks::{use_center_graph, use_drag, use_drag_end, use_drag_start, use_zoom},
    nodes::Nodes,
};
use dioxus::{
    html::geometry::{PixelsSize, euclid::default::Point2D},
    prelude::*,
};

use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct EditorState {
    pub editor_size: Signal<PixelsSize>,
    pub drag_status: Signal<DragStatus>,
    pub edge_in_creation: Signal<Option<EdgeCreation>>,
    pub zoom: Signal<f64>,
    pub shift: Signal<Point2D<f64>>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            editor_size: Signal::<PixelsSize>::default(),
            drag_status: Signal::<DragStatus>::default(),
            edge_in_creation: Signal::<Option<EdgeCreation>>::default(),
            zoom: Signal::new(1.),
            shift: Signal::<Point2D<f64>>::default(),
        }
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
    node_selected: Signal<Option<NodeElement>>,
) -> Element {
    use_context_provider(|| graph_state().graph_store);
    use_context_provider(|| graph_state().editor_state);

    let current_mouse_pos = use_signal(Point2D::default);
    let mut on_mounted: Signal<Option<std::rc::Rc<MountedData>>> = use_signal(|| None);

    let onwheel_handler = use_zoom(on_mounted);
    let ondoubleclick_handler = use_center_graph(node_selected);
    let onmousedown_handler = use_drag_start(current_mouse_pos, node_selected);
    let onmousemove_handler = use_drag(current_mouse_pos);
    let onmouseup_handler = use_drag_end();

    let shift = use_memo(move || *graph_state.read().editor_state.read().shift.read());
    let zoom = use_memo(move || *graph_state.read().editor_state.read().zoom.read());

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
