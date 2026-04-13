use crate::components::scenery_editor::edges::edges_component::{
    EdgeCreation, NewEdgeCreationStart,
};
use dioxus::{
    html::geometry::euclid::default::{Point2D, Rect},
    prelude::*,
};

#[derive(Clone, Copy, PartialEq)]
pub struct EditorState {
    pub edge_in_creation: Signal<Option<EdgeCreation>>,
    pub zoom: Signal<f64>,
    pub shift: Signal<Point2D<f64>>,
}

impl EditorState {
    pub fn apply_shift(&mut self, relative_shift: Point2D<f64>) {
        let current_shift = *self.shift.read();
        self.shift.set(Point2D::new(
            current_shift.x + relative_shift.x,
            current_shift.y + relative_shift.y,
        ));
    }
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
    ArmedSelection(Point2D<f64>),
    SelectionBox(Rect<f64>),
}
