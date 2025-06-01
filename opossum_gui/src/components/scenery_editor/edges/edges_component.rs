#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::scenery_editor::{
    edges::{define_bezier_path, edge_component::EdgeComponent},
    graph_editor::graph_editor_component::EditorState,
    graph_store::GraphStore,
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum::optic_ports::PortType;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct NewEdgeCreationStart {
    pub src_node: Uuid,
    pub src_port: String,
    pub src_port_type: PortType,
    pub start_pos: Point2D<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EdgePort {
    pub node_id: Uuid,
    pub port_name: String,
    pub port_type: PortType,
}

#[derive(Clone, PartialEq, Debug)]
pub struct EdgeCreation {
    start_port: EdgePort,
    end_port: Option<EdgePort>,
    start: Point2D<f64>,
    end: Point2D<f64>,
    bezier_offset: f64,
}

impl EdgeCreation {
    #[must_use]
    pub fn new(
        src_node: Uuid,
        src_port: String,
        src_port_type: PortType,
        start: Point2D<f64>,
    ) -> Self {
        let connection_factor = if src_port_type == PortType::Input {
            -1.
        } else {
            1.
        };
        Self {
            start_port: EdgePort {
                node_id: src_node,
                port_name: src_port,
                port_type: src_port_type,
            },
            end_port: None,
            start,
            end: start,
            bezier_offset: 50. * connection_factor,
        }
    }
    #[must_use]
    pub const fn start(&self) -> Point2D<f64> {
        self.start
    }
    #[must_use]
    pub const fn end(&self) -> Point2D<f64> {
        self.end
    }
    pub fn shift_end(&mut self, shift: Point2D<f64>) {
        self.end.x += shift.x;
        self.end.y += shift.y;
    }
    #[must_use]
    pub const fn bezier_offset(&self) -> f64 {
        self.bezier_offset
    }
    pub fn set_end_port(&mut self, end_port: Option<EdgePort>) {
        self.end_port = end_port;
    }
    pub fn is_valid(&self) -> bool {
        if let Some(end_port) = &self.end_port {
            if end_port.node_id == self.start_port.node_id {
                return false;
            }
            if end_port.port_type == self.start_port.port_type {
                return false;
            }
            true
        } else {
            false
        }
    }
    pub const fn start_port(&self) -> &EdgePort {
        &self.start_port
    }
    pub const fn end_port(&self) -> Option<&EdgePort> {
        self.end_port.as_ref()
    }
}
#[component]
pub fn EdgesComponent() -> Element {
    let graph_store = use_context::<GraphStore>();
    rsx! {
        for edge in graph_store.edges()() {
            EdgeComponent { edge }
        }
    }
}
#[component]
pub fn EdgeCreationComponent() -> Element {
    let editor_status = use_context::<EditorState>();
    let edge_in_creation = &*(editor_status.edge_in_creation.read());
    edge_in_creation.clone().map_or_else(
        || rsx! {},
        |edge| {
            let new_path = define_bezier_path(edge.start(), edge.end(), edge.bezier_offset());
            rsx! {
                path {
                    d: new_path,
                    stroke: "black",
                    fill: "transparent",
                    stroke_width: format!("{}", 2.),
                }
            }
        },
    )
}
