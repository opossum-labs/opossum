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
    /// Where this graph's canvas sits in the window, in client coordinates.
    ///
    /// Per graph rather than per workspace because two graphs can be on screen side by side, each
    /// at its own position and size - every conversion from a pointer position to graph
    /// coordinates has to use the rectangle of the graph the pointer is over. Empty until the view
    /// has been measured once; read it through `editor_area_of`, which falls back to the most
    /// recent measurement of any graph until then.
    area: Rect<f64>,
}

impl EditorState {
    pub fn apply_shift(&mut self, relative_shift: Point2D<f64>) {
        self.shift = Point2D::new(
            self.shift.x + relative_shift.x,
            self.shift.y + relative_shift.y,
        );
    }

    /// A fresh editor state whose canvas is assumed to sit at `area` until it is measured.
    ///
    /// # Arguments
    ///
    /// * `area` - the rectangle to assume, typically that of the graph the new one is opened from,
    ///   which it replaces on screen.
    ///
    /// # Returns
    ///
    /// The default editor state with `area` preset.
    pub fn with_area(area: Rect<f64>) -> Self {
        Self {
            area,
            ..Self::default()
        }
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            edge_in_creation: Option::<EdgeCreation>::default(),
            zoom: 1.,
            shift: Point2D::<f64>::default(),
            area: Rect::zero(),
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
