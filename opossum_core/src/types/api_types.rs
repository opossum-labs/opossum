use std::{collections::BTreeMap, fmt::Display};

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    coatings::CoatingType,
    core_optics::optic_ports::{PortConfig, ValidatedLidt},
    nodes::ConnectionInfo,
    opm_document::AnalyzerInfo,
    prelude::{AnalyzerType, Aperture, Isometry, PortMap, PortType, Properties},
};

/// Structure holding the version information
#[derive(ToSchema, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    /// version of the OPOSSUM API backend
    #[schema(example = "0.1.0")]
    pub backend_version: String,
    /// version of the OPOSSUM library (possibly including the git hash)
    #[schema(example = "0.6.0-18-g80cb67f (2025/02/19 15:29)")]
    pub opossum_version: String,
    /// Most current software version on GitHub (`None`, if not accessible)
    pub latest_github_version: Option<String>,
    /// URL of the release informateion (`None`, if not accessible)
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
    pub node_type: String,
    pub description: String,
}
impl Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.node_type)
    }
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug, PartialEq)]
pub struct NodeInfo {
    pub uuid: Uuid,
    pub name: String,
    pub node_type: String,
    pub inverted: bool,
    pub gui_position: Option<(f64, f64)>,
    #[schema(value_type = Option<Object>)]
    pub isometry: Option<Isometry>,
    #[schema(value_type = Option<Object>)]
    pub alignment: Option<Isometry>,
    // Optional: Die Port-Namen als reine Liste (für die GUI-Verbindungen praktisch)
    pub input_ports: Vec<String>,
    pub output_ports: Vec<String>,
}

impl NodeInfo {
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

#[derive(Debug, Serialize, ToSchema)]
pub struct NodePortsResponse {
    /// The input ports of the node (accounts for node inversion)
    pub inputs: BTreeMap<String, PortConfig>,
    /// The output ports of the node (accounts for node inversion)
    pub outputs: BTreeMap<String, PortConfig>,
}
#[derive(Clone, Serialize, Deserialize, ToSchema, Debug, PartialEq, Copy)]
pub struct NewRefNode {
    referring_node: Uuid,
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
/// Connection Information
#[derive(ToSchema, Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct ConnectInfo {
    /// UUID of the source node
    src_uuid: Uuid,
    /// name of the (outgoing) source port
    src_port: String,
    /// UUID of the target node
    target_uuid: Uuid,
    /// name of the (incoming) target port
    target_port: String,
    /// geometric distance between nodes (optical axis) in meters.
    distance: f64,
    /// Flag for reference-node indication. true if target node is a reference node.
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
#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
pub struct NewNode {
    node_type: String,
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
#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct NewAnalyzerInfo {
    pub analyzer_type: AnalyzerType,
    pub gui_position: (f64, f64),
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

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConvertToGroupRequest {
    /// Uuid of the group in which the nodes are currently contained
    pub group_id: Uuid,
    /// List of node uuids that should be converted into a new group node
    pub nodes_to_convert: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MoveNodesRequest {
    /// UUID of the source group from which the nodes will be removed
    pub source_group_id: Uuid,
    /// UUID of the destination group where nodes will be inserted
    pub target_group_id: Uuid,
    /// List of node UUIDs to move
    pub nodes_to_move: Vec<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct ErrorResponse {
    pub status: u16,
    pub category: String,
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
/// Das einheitliche Antwort-Format für die Properties (inklusive der Referenz-Info für die GUI)
#[derive(Debug, Serialize, ToSchema)]
pub struct NodePropertiesResponse {
    #[schema(value_type = Object)] // Versteckt die interne Properties-Struktur vor Utoipa
    pub properties: Properties,
    pub is_reference: bool,
}
/// Request-Objekt für partielle Updates eines Ports
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdatePortRequest {
    /// The new aperture of the port (optional)
    pub aperture: Option<Aperture>,

    /// The new coating of the port (optional)
    pub coating: Option<CoatingType>,

    /// The new Laser Induced Damage Threshold (optional)
    #[schema(value_type = Option<f64>)] // Swagger-Trick für den Type-Alias
    pub lidt: Option<ValidatedLidt>,
}
#[derive(Debug, Deserialize, ToSchema)]
pub struct AddPortMappingRequest {
    pub internal_node_id: Uuid,
    pub internal_port_name: String,
    pub external_port_name: String,
    pub port_type: PortType,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct RemovePortMapQuery {
    /// External port name of the group port mapping
    pub external_port_name: String,
    /// Type of the port (e.g., Input or Output)
    pub port_type: PortType,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PortMappingsResponse {
    pub inputs: PortMap,
    pub outputs: PortMap,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PortNamesResponse {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RemovePortMapResponse {
    pub port_removed: bool,
    pub connections: Vec<ConnectInfo>,
    pub parent_group_uuid: Uuid,
}
