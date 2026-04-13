use crate::components::scenery_editor::edges::edges_component::{
    EdgeCreation, NewEdgeCreationStart,
};
use dioxus::{
    html::geometry::euclid::default::{Point2D, Rect},
    prelude::*,
};

#[derive(Clone, PartialEq, Store)]
pub struct EditorState {
    edge_in_creation: Option<EdgeCreation>,
    zoom: f64,
    shift: Point2D<f64>,
}

impl EditorState {
    pub fn apply_shift(&mut self, relative_shift: Point2D<f64>) {
        self.shift = Point2D::new(
            self.shift.x + relative_shift.x,
            self.shift.y + relative_shift.y,
        );
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            edge_in_creation: Option::<EdgeCreation>::default(),
            zoom: 1.,
            shift: Point2D::<f64>::default(),
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
    ArmedSelection(Point2D<f64>),
    SelectionBox(Rect<f64>),
}
