use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use crate::components::scenery_editor::{edges::edges_component::EdgeCreation, graph_editor::graph_editor_component::DragStatus};

#[derive(Clone, Copy)]
pub struct EditorState {
    pub drag_status: Signal<DragStatus>,
    pub edge_in_creation: Signal<Option<EdgeCreation>>,
    pub zoom: Signal<f64>,
    pub shift: Signal<Point2D<f64>>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            drag_status: Signal::<DragStatus>::default(),
            edge_in_creation: Signal::<Option<EdgeCreation>>::default(),
            zoom: Signal::new(1.),
            shift: Signal::<Point2D<f64>>::default(),
        }
    }
}