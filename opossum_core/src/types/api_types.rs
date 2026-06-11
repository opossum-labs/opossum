use std::{collections::BTreeMap, fmt::Display};

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    analyzers::Analyzable,
    coatings::CoatingType,
    core_optics::optic_ports::{PortConfig, ValidatedLidt},
    nodes::ConnectionInfo,
    opm_document::AnalyzerInfo,
    prelude::{AnalyzerType, Aperture, Isometry, PortMap, PortType, Properties},
};

// ============================================================================
// GENERAL TYPES & ERRORS
// ============================================================================

/// Structure holding the version information
#[derive(ToSchema, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    /// Version of the OPOSSUM API backend
    #[schema(example = "0.1.0")]
    pub backend_version: String,
    /// Version of the OPOSSUM library (possibly including the git hash)
    #[schema(example = "0.6.0-18-g80cb67f (2025/02/19 15:29)")]
    pub opossum_version: String,
    /// Most current software version on GitHub (`None`, if not accessible)
    pub latest_github_version: Option<String>,
    /// URL of the release information (`None`, if not accessible)
    pub release_url: Option<String>,
    /// True, if GitHub Version is newer than the local one
    pub update_available: bool,
}

impl VersionInfo {
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn backend_version(&self) -> &str {
        &self.backend_version
    }
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn opossum_version(&self) -> &str {
        &self.opossum_version
    }
}

/// Structure holding information about an (optical) node type
#[derive(Deserialize, Serialize, ToSchema)]
pub struct NodeType {
    /// The internal identifier of the node type
    #[schema(example = "Lens")]
    pub node_type: String,
    /// A human-readable description of the node type
    pub description: String,
}

impl Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.node_type)
    }
}

/// Standardized error response for API failures
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct ErrorResponse {
    /// HTTP status code
    #[schema(example = 400)]
    pub status: u16,
    /// High-level category of the error (e.g., `Parse Error`, `OpticScenery`)
    #[schema(example = "OpticScenery")]
    pub category: String,
    /// Detailed error message
    #[schema(example = "UUID not found in the current model")]
    pub message: String,
}

impl ErrorResponse {
    #[must_use]
    pub fn new(status: u16, category: &str, message: &str) -> Self {
        Self {
            status,
            category: category.to_string(),
            message: message.to_string(),
        }
    }
    #[must_use]
    pub fn not_found() -> Self {
        Self::new(404, "General", "Resource not found")
    }
}

// ============================================================================
// NODES & PROPERTIES
// ============================================================================

/// Comprehensive information about an optical node in the scenery
#[derive(Serialize, Deserialize, Default, ToSchema, Clone, Debug, PartialEq)]
pub struct NodeInfo {
    pub uuid: Uuid,
    #[schema(example = "Main Focusing Lens")]
    pub name: String,
    #[schema(example = "Lens")]
    pub node_type: String,
    /// Indicates if the node is physically inverted in the optical path
    pub inverted: bool,
    /// The 2D coordinates on the frontend canvas
    #[schema(example = json!([100.0, 200.0]))]
    pub gui_position: Option<(f64, f64)>,
    /// Global 3D position and rotation
    #[schema(value_type = Option<Object>)]
    pub isometry: Option<Isometry>,
    /// Local alignment (decenter and tilt)
    #[schema(value_type = Option<Object>)]
    pub alignment: Option<Isometry>,
    /// List of available input port names
    pub input_ports: Vec<String>,
    /// List of available output port names
    pub output_ports: Vec<String>,
}

impl NodeInfo {
    /// Create a `NodeInfo` struct from this [`Analyzable`]
    pub fn from_analyzable(
        node: &dyn Analyzable,
        gui_position: Option<Option<(f64, f64)>>,
    ) -> Self {
        Self {
            uuid: node.node_attr().uuid(),
            name: node.name(),
            inverted: node.inverted(),
            node_type: node.node_type(),
            input_ports: node.ports().names(&PortType::Input),
            output_ports: node.ports().names(&PortType::Output),
            gui_position: gui_position.unwrap_or_else(|| node.gui_position().map(|p| (p.x, p.y))),
            isometry: node.isometry(),
            alignment: node.alignment(),
        }
    }

    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.uuid
    }
    #[must_use]
    pub const fn inverted(&self) -> bool {
        self.inverted
    }
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub fn node_type(&self) -> &str {
        &self.node_type
    }
    #[must_use]
    pub const fn gui_position(&self) -> Option<(f64, f64)> {
        self.gui_position
    }
    #[must_use]
    pub fn input_ports(&self) -> Vec<String> {
        self.input_ports.clone()
    }
    #[must_use]
    pub fn output_ports(&self) -> Vec<String> {
        self.output_ports.clone()
    }
    pub fn set_input_ports(&mut self, inputs: Vec<String>) {
        self.input_ports = inputs;
    }
    pub fn set_output_ports(&mut self, outputs: Vec<String>) {
        self.output_ports = outputs;
    }
}

/// Request payload to create a standard optical node
#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
pub struct NewNode {
    #[schema(example = "Lens")]
    node_type: String,
    #[schema(example = json!([0.0, 0.0]))]
    gui_position: (f64, f64),
}

impl NewNode {
    #[must_use]
    pub const fn new(node_type: String, gui_position: (f64, f64)) -> Self {
        Self {
            node_type,
            gui_position,
        }
    }
    #[must_use]
    pub fn node_type(&self) -> &str {
        &self.node_type
    }
    #[must_use]
    pub const fn gui_position(&self) -> (f64, f64) {
        self.gui_position
    }
}

/// Request payload to create a reference node pointing to an existing node
#[derive(Clone, Serialize, Deserialize, ToSchema, Debug, PartialEq, Copy)]
pub struct NewRefNode {
    /// UUID of the optical node this reference points to
    referring_node: Uuid,
    #[schema(example = json!([50.0, -20.0]))]
    gui_position: (f64, f64),
}

impl NewRefNode {
    #[must_use]
    pub const fn new(referring_node: Uuid, gui_position: (f64, f64)) -> Self {
        Self {
            referring_node,
            gui_position,
        }
    }
    #[must_use]
    pub const fn gui_position(&self) -> (f64, f64) {
        self.gui_position
    }
    #[must_use]
    pub const fn referring_node(&self) -> Uuid {
        self.referring_node
    }
}

/// Request payload for partial updates of a node's properties
#[derive(Debug, Default, Serialize, Deserialize, ToSchema, Clone)]
pub struct UpdateNodeRequest {
    /// The new name of the node
    #[schema(example = "Lens 1")]
    pub name: Option<String>,

    /// The new inverted status of the node
    #[schema(example = true)]
    pub inverted: Option<bool>,

    /// The new base isometry (position and rotation in 3D space)
    #[schema(value_type = Option<Object>)]
    pub isometry: Option<Option<Isometry>>, // Option<Option> erlaubt explizites Null-Setzen!

    /// The new alignment isometry (local decenter and tilt)
    #[schema(value_type = Option<Object>)]
    pub alignment: Option<Isometry>,

    /// The GUI position on the 2D canvas
    #[schema(example = json!([100.5, 200.0]))]
    pub gui_position: Option<Option<(f64, f64)>>,
}

/// Response payload containing the physical and custom properties of a node
#[derive(Debug, Serialize, ToSchema, Deserialize)]
pub struct NodePropertiesResponse {
    #[schema(value_type = Object)] // Hides internal Properties structure from Utoipa
    pub properties: Properties,
    /// True if the properties belong to a reference node
    pub is_reference: bool,
}

// ============================================================================
// CONNECTIONS
// ============================================================================

/// Information about a connection between two optical ports
#[derive(ToSchema, Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct ConnectInfo {
    /// UUID of the source node
    src_uuid: Uuid,
    /// Name of the (outgoing) source port
    #[schema(example = "output_1")]
    src_port: String,
    /// UUID of the target node
    target_uuid: Uuid,
    /// Name of the (incoming) target port
    #[schema(example = "input_1")]
    target_port: String,
    /// Geometric distance between nodes (optical axis) in meters
    #[schema(example = 0.05)]
    distance: f64,
    /// True if the target node is a reference node
    target_is_reference: bool,
}

impl ConnectInfo {
    #[must_use]
    pub const fn new(
        src_uuid: Uuid,
        src_port: String,
        target_uuid: Uuid,
        target_port: String,
        distance: f64,
        target_is_reference: bool,
    ) -> Self {
        Self {
            src_uuid,
            src_port,
            target_uuid,
            target_port,
            distance,
            target_is_reference,
        }
    }
    #[must_use]
    pub fn from_connection_info(c: &ConnectionInfo, is_reference: bool) -> Self {
        Self {
            src_uuid: c.src_id,
            src_port: c.src_port.clone(),
            target_uuid: c.target_id,
            target_port: c.target_port.clone(),
            distance: c.distance.value,
            target_is_reference: is_reference,
        }
    }
    #[must_use]
    pub const fn src_uuid(&self) -> Uuid {
        self.src_uuid
    }
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn src_port(&self) -> &str {
        &self.src_port
    }
    #[must_use]
    pub const fn target_uuid(&self) -> Uuid {
        self.target_uuid
    }
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn target_port(&self) -> &str {
        &self.target_port
    }
    #[must_use]
    pub const fn distance(&self) -> f64 {
        self.distance
    }
    pub const fn set_distance(&mut self, distance: f64) {
        self.distance = distance;
    }
    pub const fn set_is_reference(&mut self, target_is_reference: bool) {
        self.target_is_reference = target_is_reference;
    }
    #[must_use]
    pub const fn targets_reference(&self) -> bool {
        self.target_is_reference
    }
}

/// Request payload to update the distance of an existing connection
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateConnectionRequest {
    /// UUID of the source node (used to identify the connection)
    pub src_uuid: Uuid,
    /// Name of the source port (used to identify the connection)
    #[schema(example = "output_1")]
    pub src_port: String,
    /// The new geometric distance in meters
    #[schema(example = 0.15)]
    pub distance: f64,
}
// ============================================================================
// PORTS & PORT MAPPINGS
// ============================================================================

/// Response payload containing port configurations (Aperture, Coating, LIDT)
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct NodePortsResponse {
    /// The input ports of the node (accounts for node inversion)
    pub inputs: BTreeMap<String, PortConfig>,
    /// The output ports of the node (accounts for node inversion)
    pub outputs: BTreeMap<String, PortConfig>,
}

/// Request payload for partial updates of a specific port
#[derive(Debug, Default, Serialize, Deserialize, ToSchema)]
pub struct UpdatePortRequest {
    /// The new aperture of the port
    pub aperture: Option<Aperture>,
    /// The new coating of the port
    pub coating: Option<CoatingType>,
    /// The new Laser Induced Damage Threshold
    #[schema(value_type = Option<f64>)]
    pub lidt: Option<ValidatedLidt>,
}

/// Request payload to expose an internal node's port to a parent group
#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct AddPortMappingRequest {
    pub internal_node_id: Uuid,
    #[schema(example = "input_1")]
    pub internal_port_name: String,
    #[schema(example = "group_in_1")]
    pub external_port_name: String,
    pub port_type: PortType,
}

/// Query parameters to remove a port mapping
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct RemovePortMapQuery {
    /// External port name of the group port mapping
    #[schema(example = "group_in_1")]
    pub external_port_name: String,
    /// Type of the port (Input or Output)
    pub port_type: PortType,
}

/// Response payload containing the internal-to-external port mappings of a group
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PortMappingsResponse {
    pub inputs: PortMap,
    pub outputs: PortMap,
}

/// Response payload containing lists of available mapped port names
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PortNamesResponse {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

/// Response payload after removing a port map, containing affected connections
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RemovePortMapResponse {
    /// True if a mapping was actually found and removed
    pub port_removed: bool,
    /// Connections that were disconnected as a result
    pub connections: Vec<ConnectInfo>,
    /// UUID of the parent group
    pub parent_group_uuid: Uuid,
}

// ============================================================================
// ANALYZERS
// ============================================================================

/// Request payload to create a new analyzer
#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct NewAnalyzerInfo {
    pub analyzer_type: AnalyzerType,
    #[schema(example = json!([0.0, 0.0]))]
    pub gui_position: (f64, f64),
}

/// Data Transfer Object to securely send an analyzer with its corresponding ID to the client.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct AnalyzerItemDto {
    /// The unique identifier of the analyzer.
    pub id: Uuid,
    /// The actual data of the analyzer.
    pub info: AnalyzerInfo,
}

/// Request payload for partial updates of an analyzer's properties
#[derive(Debug, Default, Serialize, Deserialize, ToSchema, Clone)]
pub struct UpdateAnalyzerInfo {
    // The new Analyzertype including its configuration
    #[schema(value_type = Option<Object>)]
    pub analyzer_type: Option<AnalyzerType>,

    // The new position of the analyzer
    #[schema(example = json!([0.0, 0.0]))]
    pub gui_position: Option<Option<(f64, f64)>>,
}

impl From<AnalyzerInfo> for NewAnalyzerInfo {
    fn from(value: AnalyzerInfo) -> Self {
        let pos = value
            .gui_position()
            .map_or_else(|| (0., 0.), |p| (p.x, p.y));
        Self {
            analyzer_type: value.analyzer_type().clone(),
            gui_position: pos,
        }
    }
}

impl NewAnalyzerInfo {
    #[must_use]
    pub const fn new(analyzer_type: AnalyzerType, gui_position: (f64, f64)) -> Self {
        Self {
            analyzer_type,
            gui_position,
        }
    }
}

// ============================================================================
// MACRO OPERATIONS
// ============================================================================

/// Request payload to group existing nodes into a new sub-group
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct ConvertToGroupRequest {
    /// UUID of the group in which the nodes are currently contained
    pub group_id: Uuid,
    /// List of node UUIDs that should be wrapped into a new group node
    pub nodes_to_convert: Vec<Uuid>,
}

/// Request payload to move nodes between different groups
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct MoveNodesRequest {
    /// UUID of the source group from which the nodes will be removed
    pub source_group_id: Uuid,
    /// UUID of the destination group where nodes will be inserted
    pub target_group_id: Uuid,
    /// List of node UUIDs to move
    pub nodes_to_move: Vec<Uuid>,
}
