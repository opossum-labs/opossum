use crate::components::scenery_editor::edges::edges_component::{
    EdgeCreation, NewEdgeCreationStart,
};
use dioxus::{
    html::geometry::euclid::default::{Point2D, Rect},
    prelude::*,
};

#[derive(Clone, Copy)]
pub struct EditorState {
    pub edge_in_creation: Signal<Option<EdgeCreation>>,
    pub zoom: Signal<f64>,
    pub shift: Signal<Point2D<f64>>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            edge_in_creation: Signal::<Option<EdgeCreation>>::default(),
            zoom: Signal::new(1.),
            shift: Signal::<Point2D<f64>>::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum DragStatus {
    #[default]
    None,
    Graph,
    Nodes,
    NodeInit,
    Edge(NewEdgeCreationStart),
    SelectionBox(Rect<f64>),
}
