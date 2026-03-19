use crate::components::scenery_editor::edges::edges_component::{
    EdgeCreation, NewEdgeCreationStart,
};
use dioxus::{
    html::geometry::euclid::default::{Point2D, Rect},
    prelude::*,
};
use uuid::Uuid;

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

#[derive(Clone, Debug, Default)]
pub enum DragStatus {
    #[default]
    None,
    Graph,
    Node(Uuid, Point2D<f64>), // stores also old position before drag.
    Edge(NewEdgeCreationStart),
    SelectionBox(Rect<f64>),
}
