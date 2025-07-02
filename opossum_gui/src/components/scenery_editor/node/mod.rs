use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_backend::{usize_to_f64, AnalyzerType, PortType};
use uuid::Uuid;
mod graph_node_components;
pub mod node_component;
use crate::components::scenery_editor::constants::{HEADER_HEIGHT, NODE_WIDTH, PORT_VER_SPACING};

use super::ports::ports_component::Ports;
pub use node_component::Node;

const NODE_BEAMSPLITTER: Asset = asset!("./assets/icons/node_beamsplitter.svg");
const NODE_CYLINDRIC_LENS: Asset = asset!("./assets/icons/node_cylindric_lens.svg");
const NODE_ENERGY_METER: Asset = asset!("./assets/icons/node_energymeter.svg");
const NODE_FILTER: Asset = asset!("./assets/icons/node_filter.svg");
const NODE_FLUENCE: Asset = asset!("./assets/icons/node_fluence.svg");
const NODE_GRATING: Asset = asset!("./assets/icons/node_grating.svg");
const NODE_GROUP: Asset = asset!("./assets/icons/node_group.svg");
const NODE_LENS: Asset = asset!("./assets/icons/node_lens.svg");
const NODE_MIRROR: Asset = asset!("./assets/icons/node_mirror.svg");
const NODE_PARABOLA: Asset = asset!("./assets/icons/node_parabola.svg");
const NODE_PARAXIAL: Asset = asset!("./assets/icons/node_paraxial.svg");
const NODE_PROPAGATION: Asset = asset!("./assets/icons/node_propagation.svg");
const NODE_SOURCE: Asset = asset!("./assets/icons/node_source.svg");
const NODE_SPECTROMETER: Asset = asset!("./assets/icons/node_spectrometer.svg");
const NODE_SPOTDIAGRAM: Asset = asset!("./assets/icons/node_spotdiagram.svg");
const NODE_UNKNOWN: Asset = asset!("./assets/icons/node_unknown.svg");
const NODE_WEDGE: Asset = asset!("./assets/icons/node_wedge.svg");

// Constants for node dimensions and port positions
const GOLDEN_RATIO: f64 = 1.618_033_988_7;
// The minimum node body height is fixed such that the overall node height (header + body) corresponds to
// to the golden ratio
pub const MIN_NODE_BODY_HEIGHT: f64 = NODE_WIDTH / GOLDEN_RATIO - HEADER_HEIGHT;
// Nodes with only one port will be vertically centered
// in the node body, so we need to add some padding
pub const PORT_VER_PADDING: f64 = MIN_NODE_BODY_HEIGHT / 2.0;

#[derive(Clone, PartialEq, Debug)]
pub enum NodeType {
    Optical(String),
    Analyzer(AnalyzerType),
}
impl Default for NodeType {
    fn default() -> Self {
        Self::Optical(String::new())
    }
}
impl NodeType {
    fn icon(&self) -> Option<Asset> {
        match self {
            Self::Optical(node_type) => match node_type.as_str() {
                // "dummy" => Some(NODE_UNKNOWN),
                "beam splitter" => Some(NODE_BEAMSPLITTER),
                "energy meter" => Some(NODE_ENERGY_METER),
                "group" => Some(NODE_GROUP),
                "ideal filter" => Some(NODE_FILTER),
                "reflective grating" => Some(NODE_GRATING),
                // "reference" => Some(NODE_UNKNOWN),
                "lens" => Some(NODE_LENS),
                "cylindric lens" => Some(NODE_CYLINDRIC_LENS),
                "source" => Some(NODE_SOURCE),
                "spectrometer" => Some(NODE_SPECTROMETER),
                "spot diagram" => Some(NODE_SPOTDIAGRAM),
                // "wavefront monitor" => Some(NODE_UNKNOWN),
                "paraxial surface" => Some(NODE_PARAXIAL),
                "ray propagation" => Some(NODE_PROPAGATION),
                "fluence detector" => Some(NODE_FLUENCE),
                "wedge" => Some(NODE_WEDGE),
                "mirror" => Some(NODE_MIRROR),
                "parabolic mirror" => Some(NODE_PARABOLA),
                _ => Some(NODE_UNKNOWN),
            },
            Self::Analyzer(_) => None,
        }
    }
}
#[derive(Clone, PartialEq, Default)]
pub struct NodeElement {
    name: String,
    node_type: NodeType,
    id: Uuid,
    pos: Point2D<f64>,
    z_index: usize,
    ports: Ports,
}

impl NodeElement {
    #[must_use]
    pub const fn new(
        name: String,
        node_type: NodeType,
        id: Uuid,
        pos: Point2D<f64>,
        ports: Ports,
    ) -> Self {
        Self {
            name,
            node_type,
            pos,
            id,
            z_index: 0,
            ports,
        }
    }
    #[must_use]
    pub const fn input_ports(&self) -> &Vec<String> {
        self.ports.input_ports()
    }
    #[must_use]
    pub const fn output_ports(&self) -> &Vec<String> {
        self.ports.output_ports()
    }
    #[must_use]
    pub const fn z_index(&self) -> usize {
        self.z_index
    }
    #[must_use]
    pub const fn pos(&self) -> Point2D<f64> {
        self.pos
    }
    #[must_use]
    pub fn name(&self) -> String {
        match &self.node_type {
            NodeType::Optical(_) => self.name.clone(),
            NodeType::Analyzer(analyzer_type) => format!("{analyzer_type}"),
        }
    }
    #[must_use]
    pub const fn id(&self) -> Uuid {
        self.id
    }
    pub fn shift_position(&mut self, shift: Point2D<f64>) {
        self.pos.x += shift.x;
        self.pos.y += shift.y;
    }
    #[must_use]
    pub fn rel_port_position(&self, port_type: &PortType, port_name: &str) -> Point2D<f64> {
        let (x_pos, port_list) = match port_type {
            PortType::Input => (0.0, self.input_ports()),
            PortType::Output => (NODE_WIDTH, self.output_ports()),
        };
        let port_index = port_list
            .iter()
            .position(|port| port == port_name)
            .unwrap_or(0);
        let y_pos = PORT_VER_SPACING.mul_add(usize_to_f64(port_index), PORT_VER_PADDING);
        Point2D::new(x_pos, y_pos)
    }
    #[must_use]
    pub fn abs_port_position(&self, port_type: &PortType, port_name: &str) -> Point2D<f64> {
        let rel_pos = self.rel_port_position(port_type, port_name);
        Point2D::new(
            self.pos.x + rel_pos.x,
            self.pos.y + rel_pos.y + HEADER_HEIGHT,
        )
    }
    #[must_use]
    pub fn node_body_height(&self) -> f64 {
        let max_vert_number_of_ports =
            usize_to_f64(self.output_ports().len().max(self.input_ports().len()));
        let necessary_body_height = 2.0f64.mul_add(
            PORT_VER_PADDING,
            PORT_VER_SPACING * (max_vert_number_of_ports - 1.0),
        );
        necessary_body_height.max(MIN_NODE_BODY_HEIGHT)
    }
    #[must_use]
    pub const fn node_type(&self) -> &NodeType {
        &self.node_type
    }
    pub const fn set_pos(&mut self, pos: Point2D<f64>) {
        self.pos = pos;
    }
    pub const fn set_z_index(&mut self, z_index: usize) {
        self.z_index = z_index;
    }
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }
}
