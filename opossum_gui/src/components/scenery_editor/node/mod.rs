#![allow(clippy::volatile_composites)]
use dioxus::{
    html::geometry::euclid::default::{Point2D, Rect, Size2D},
    prelude::*,
};
use opossum_core::{
    prelude::*,
    types::api_types::{AnalyzerItemDto, NodeInfo},
    utils::to_f64,
};
use uuid::Uuid;
mod graph_node_components;
pub mod node_component;
use crate::components::scenery_editor::constants::{
    BORDER_WIDTH, HEADER_HEIGHT, NODE_WIDTH, PORT_VER_SPACING,
};

use super::ports::ports_component::Ports;
pub use node_component::Node;

const NODE_BEAMSPLITTER: Asset = asset!("/assets/icons/node_beamsplitter.svg");
const NODE_CYLINDRIC_LENS: Asset = asset!("/assets/icons/node_cylindric_lens.svg");
const NODE_ENERGY_METER: Asset = asset!("/assets/icons/node_energymeter.svg");
const NODE_FILTER: Asset = asset!("/assets/icons/node_filter.svg");
const NODE_FLUENCE: Asset = asset!("/assets/icons/node_fluence.svg");
const NODE_GRATING: Asset = asset!("/assets/icons/node_grating.svg");
const NODE_GROUP: Asset = asset!("/assets/icons/node_group.svg");
const NODE_LENS: Asset = asset!("/assets/icons/node_lens.svg");
const NODE_MIRROR: Asset = asset!("/assets/icons/node_mirror.svg");
const NODE_PARABOLA: Asset = asset!("/assets/icons/node_parabola.svg");
const NODE_PARAXIAL: Asset = asset!("/assets/icons/node_paraxial.svg");
const NODE_PROPAGATION: Asset = asset!("/assets/icons/node_propagation.svg");
const NODE_SOURCEPORT: Asset = asset!("/assets/icons/node_source.svg");
const NODE_SPECTROMETER: Asset = asset!("/assets/icons/node_spectrometer.svg");
const NODE_WAVEFRONT: Asset = asset!("/assets/icons/node_wavefront.svg");
const NODE_SPOTDIAGRAM: Asset = asset!("/assets/icons/node_spotdiagram.svg");
const NODE_UNKNOWN: Asset = asset!("/assets/icons/node_unknown.svg");
const NODE_WEDGE: Asset = asset!("/assets/icons/node_wedge.svg");

// Constants for node dimensions and port positions
const GOLDEN_RATIO: f64 = 1.618_033_988_7;
// The minimum node body height is fixed such that the overall node height (header + body) corresponds to
// to the golden ratio
pub const MIN_NODE_BODY_HEIGHT: f64 = NODE_WIDTH / GOLDEN_RATIO - HEADER_HEIGHT;
// Nodes with only one port will be vertically centered
// in the node body, so we need to add some padding
pub const PORT_VER_PADDING: f64 = MIN_NODE_BODY_HEIGHT / 2.0;
// Overall height of a node that has no more than two ports per side. Used where a node height is
// needed before any concrete node exists (e.g. to center a node that is about to be created);
// an existing node's own height is `NodeElement::total_height`.
pub const DEFAULT_NODE_HEIGHT: f64 = HEADER_HEIGHT + MIN_NODE_BODY_HEIGHT;
// Height of the status line shown below the body of an amplifying node. Nodes that do not amplify
// have no such line and are unaffected.
pub const AMP_STATUS_HEIGHT: f64 = 14.0;
// Where a node lands if the backend sent no position for it. In practice every add/paste/reference
// response carries one, so this is a safety net rather than a normal case.
const DEFAULT_NEW_NODE_POS: Point2D<f64> = Point2D::new(100.0, 100.0);

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
                "beam splitter" => Some(NODE_BEAMSPLITTER),
                "energy meter" => Some(NODE_ENERGY_METER),
                "group" => Some(NODE_GROUP),
                "ideal filter" => Some(NODE_FILTER),
                "reflective grating" => Some(NODE_GRATING),
                "lens" => Some(NODE_LENS),
                "cylindric lens" => Some(NODE_CYLINDRIC_LENS),
                "source port" => Some(NODE_SOURCEPORT),
                "spectrometer" => Some(NODE_SPECTROMETER),
                "spot diagram" => Some(NODE_SPOTDIAGRAM),
                "wavefront monitor" => Some(NODE_WAVEFRONT),
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
#[derive(Clone, PartialEq, Default, Debug)]
pub struct NodeElement {
    name: String,
    node_type: NodeType,
    id: Uuid,
    pos: Point2D<f64>,
    z_index: usize,
    ports: Ports,
    inverted: bool,
    node_index: usize,
    /// Name of the node's gain model in the *active pump scenario*, or `None` if it does not
    /// amplify there. A display marker only - kept in sync by `set_amp_model`/`sync_amp_markers`
    /// rather than fetched by this node itself; the parameters are edited through the scenario
    /// editor, not here.
    amp_model: Option<String>,
    /// Whether this node is a member of the document-wide amplifier-candidate set - a hardware
    /// fact, independent of the *active pump scenario* `amp_model` reflects. A display marker only,
    /// kept in sync by `set_amplifier_candidate`/`sync_amplifier_candidates`; candidacy itself is
    /// edited through the context menu's "As amplifier" toggle.
    is_amplifier_candidate: bool,
}

impl NodeElement {
    #[must_use]
    pub const fn new(
        name: String,
        node_type: NodeType,
        id: Uuid,
        pos: Point2D<f64>,
        ports: Ports,
        inverted: bool,
        node_index: usize, // this is a unique id for testing with playwright
    ) -> Self {
        Self {
            name,
            node_type,
            pos,
            id,
            z_index: 0,
            ports,
            inverted,
            node_index,
            amp_model: None,
            is_amplifier_candidate: false,
        }
    }
    /// Returns the unique sequential node index assigned upon creation.
    #[must_use]
    pub const fn node_index(&self) -> usize {
        self.node_index
    }
    #[must_use]
    pub const fn input_ports(&self) -> &Vec<String> {
        self.ports.input_ports()
    }
    #[must_use]
    pub const fn output_ports(&self) -> &Vec<String> {
        self.ports.output_ports()
    }
    pub fn remove_port(&mut self, remove_port: &str, port_type: PortType) {
        match port_type {
            PortType::Input => self.ports.remove_input_port(remove_port),
            PortType::Output => self.ports.remove_output_port(remove_port),
        }
    }
    pub fn set_ports(&mut self, input_ports: Vec<String>, output_ports: Vec<String>) {
        self.ports.set_input_ports(input_ports);
        self.ports.set_output_ports(output_ports);
    }
    #[must_use]
    pub const fn z_index(&self) -> usize {
        self.z_index
    }
    #[must_use]
    pub const fn inverted(&self) -> bool {
        self.inverted
    }
    #[must_use]
    pub const fn pos(&self) -> Point2D<f64> {
        self.pos
    }
    #[must_use]
    pub fn get_bounding_box(&self) -> Rect<f64> {
        let min_x = self.pos().x;
        let min_y = self.pos().y;
        let max_x = self.pos().x + NODE_WIDTH;
        let max_y = self.pos().y + self.total_height();

        Rect::new(
            Point2D::new(min_x, min_y),
            Size2D::new(max_x - min_x, max_y - min_y),
        )
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
    pub fn rel_port_position(&self, port_type: PortType, port_name: &str) -> Point2D<f64> {
        let (x_pos, port_list) = match port_type {
            PortType::Input => (0.0, self.input_ports()),
            PortType::Output => (NODE_WIDTH + BORDER_WIDTH, self.output_ports()),
        };
        let port_index = port_list
            .iter()
            .position(|port| port == port_name)
            .unwrap_or(0);
        let y_pos = PORT_VER_SPACING.mul_add(to_f64(port_index), PORT_VER_PADDING);
        Point2D::new(x_pos, y_pos)
    }
    #[must_use]
    pub fn abs_port_position(&self, port_type: PortType, port_name: &str) -> Point2D<f64> {
        let rel_pos = self.rel_port_position(port_type, port_name);
        Point2D::new(
            self.pos.x + rel_pos.x + BORDER_WIDTH,
            self.pos.y + rel_pos.y + HEADER_HEIGHT + BORDER_WIDTH / 2.,
        )
    }
    #[must_use]
    pub fn node_body_height(&self) -> f64 {
        let max_vert_number_of_ports =
            to_f64(self.output_ports().len().max(self.input_ports().len()));
        let necessary_body_height = 2.0f64.mul_add(
            PORT_VER_PADDING,
            PORT_VER_SPACING * (max_vert_number_of_ports - 1.0),
        );
        necessary_body_height.max(MIN_NODE_BODY_HEIGHT)
    }
    /// Returns the overall height this node occupies on the canvas.
    ///
    /// This is the single place that knows what a node is made of vertically, so everything that
    /// needs a node's extent - bounding box, selection box, auto-layout row heights, drop-in-group
    /// hit testing - stays correct when a node type grows an additional part.
    #[must_use]
    pub fn total_height(&self) -> f64 {
        // The status line is shown for every amplifier *candidate*, not only while it actively
        // amplifies in the current scenario - so its presence must survive switching to "no active
        // scenario" or to a scenario that leaves this node passive, not just disappear along with
        // `amp_model`.
        let amp_status_height = if self.is_amplifier_candidate {
            AMP_STATUS_HEIGHT
        } else {
            0.0
        };
        HEADER_HEIGHT + self.node_body_height() + amp_status_height
    }
    /// Returns the name of the node's active amplification model, or `None` if it does not amplify.
    #[must_use]
    pub fn amp_model(&self) -> Option<&str> {
        self.amp_model.as_deref()
    }
    /// Sets (or clears, with `None`) the node's amplification marker.
    pub fn set_amp_model(&mut self, amp_model: Option<String>) {
        self.amp_model = amp_model;
    }
    /// Returns whether this node is a member of the document-wide amplifier-candidate set.
    #[must_use]
    pub const fn is_amplifier_candidate(&self) -> bool {
        self.is_amplifier_candidate
    }
    /// Sets whether this node is a member of the document-wide amplifier-candidate set.
    pub const fn set_amplifier_candidate(&mut self, is_amplifier_candidate: bool) {
        self.is_amplifier_candidate = is_amplifier_candidate;
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
    /// Sets the node's sequential index, which serves as its Playwright test id. Assigned by the
    /// graph store, since it is a property of the canvas rather than of the node itself.
    pub const fn set_node_index(&mut self, node_index: usize) {
        self.node_index = node_index;
    }
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }
    pub fn set_inverted(&mut self, inverted: bool) {
        if self.inverted == inverted {
            return;
        }
        self.inverted = inverted;
        self.ports.invert_ports();
    }
    pub const fn is_optical_node(&self) -> bool {
        matches!(self.node_type, NodeType::Optical(_))
    }
}

/// The single place that maps the backend's [`NodeInfo`] onto a canvas node, so a field added to the
/// DTO cannot be picked up on one code path and silently dropped on another.
///
/// The node index (the Playwright test id) is not part of the DTO and is assigned by the graph store
/// afterwards via [`NodeElement::set_node_index`].
impl From<&NodeInfo> for NodeElement {
    fn from(node_info: &NodeInfo) -> Self {
        let position = node_info
            .gui_position()
            .map_or(DEFAULT_NEW_NODE_POS, |(x, y)| Point2D::new(x, y));
        let node = Self::new(
            node_info.name().to_string(),
            NodeType::Optical(node_info.node_type().to_string()),
            node_info.uuid(),
            position,
            Ports::new(node_info.input_ports(), node_info.output_ports()),
            node_info.inverted(),
            0,
        );
        // Not seeded from `node_info.amp_model` (the legacy `amp config` property marker,
        // superseded by pump scenarios - see `crate::ACTIVE_SCENARIO_GAIN_MODELS`), and likewise
        // `is_amplifier_candidate` is left at its default `false` here too: this conversion has to
        // stay usable outside a mounted app (it is unit-tested as plain conversion logic), so it
        // cannot read a Dioxus global signal. Callers running inside the live app look both markers
        // up themselves right after constructing the node - see
        // `GraphStore::add_new_optical_node`/`add_new_reference_node`.
        node
    }
}

impl From<&AnalyzerItemDto> for NodeElement {
    fn from(dto: &AnalyzerItemDto) -> Self {
        let position = dto
            .info
            .gui_position()
            .map_or_else(Point2D::zero, |p| Point2D::new(p.x, p.y));

        Self::new(
            format!("{}", dto.info.analyzer_type()),
            NodeType::Analyzer(dto.info.analyzer_type().clone()),
            dto.id,
            position,
            Ports::default(),
            false,
            0,
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn amplifying_node_info() -> NodeInfo {
        NodeInfo {
            name: "amp lens".to_string(),
            node_type: "lens".to_string(),
            gui_position: Some((10.0, 20.0)),
            amp_model: Some("Const".to_string()),
            ..NodeInfo::default()
        }
    }

    /// `NodeInfo::amp_model` is the legacy `amp config` property marker; the canvas marker now
    /// mirrors the *active pump scenario* instead (seeded separately by whichever live code
    /// constructs the node - see `GraphStore::add_new_optical_node`), so this plain conversion
    /// must leave it alone rather than carrying the property-based value over.
    #[test]
    fn conversion_ignores_the_legacy_amp_marker() {
        let node = NodeElement::from(&amplifying_node_info());
        assert_eq!(node.amp_model(), None);
        // No amplifier marker means no status line, so the node must not be inflated by its height.
        assert!(node.total_height() <= HEADER_HEIGHT + node.node_body_height());
    }

    /// Candidacy is document-wide data, not carried on `NodeInfo` at all - a freshly converted node
    /// must default to "not a candidate" until whichever live code constructs it seeds the flag from
    /// `crate::AMPLIFIER_CANDIDATES` (see `GraphStore::add_new_optical_node`).
    #[test]
    fn conversion_defaults_to_not_an_amplifier_candidate() {
        let node = NodeElement::from(&amplifying_node_info());
        assert!(!node.is_amplifier_candidate());
    }

    /// A node marked as a candidate must show the status line (and be inflated by its height) even
    /// while it does not actively amplify - `None` in the active scenario, or no scenario active at
    /// all - so candidacy stays visible while editing the node's other properties. Only a node that
    /// isn't a candidate at all gets no line.
    #[test]
    fn candidacy_alone_reserves_the_status_line_regardless_of_amp_model() {
        let mut node = NodeElement::from(&amplifying_node_info());
        node.set_amplifier_candidate(true);
        assert_eq!(
            node.amp_model(),
            None,
            "candidacy must not fabricate an active amp model"
        );
        assert!(
            node.total_height() > HEADER_HEIGHT + node.node_body_height(),
            "a candidate must reserve the status line even while passive in the current scenario"
        );

        node.set_amplifier_candidate(false);
        assert!(
            node.total_height() <= HEADER_HEIGHT + node.node_body_height(),
            "a non-candidate must show no status line"
        );
    }

    #[test]
    fn amplifier_candidate_getter_and_setter_round_trip() {
        let mut node = NodeElement::from(&amplifying_node_info());
        node.set_amplifier_candidate(true);
        assert!(node.is_amplifier_candidate());
        node.set_amplifier_candidate(false);
        assert!(!node.is_amplifier_candidate());
    }

    /// The fallback position only applies when the backend has no position of its own to report.
    #[test]
    fn conversion_prefers_the_backend_position() {
        let node = NodeElement::from(&amplifying_node_info());
        assert_eq!(node.pos(), Point2D::new(10.0, 20.0));

        let without_position = NodeInfo {
            gui_position: None,
            ..amplifying_node_info()
        };
        let node = NodeElement::from(&without_position);
        assert_eq!(node.pos(), DEFAULT_NEW_NODE_POS);
    }
}
