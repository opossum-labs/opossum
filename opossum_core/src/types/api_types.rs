use std::fmt::Display;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{opm_document::AnalyzerInfo, prelude::AnalyzerType};

/// Structure holding the version information
#[derive(ToSchema, Serialize, Deserialize)]
pub struct VersionInfo {
    /// version of the OPOSSUM API backend
    #[schema(example = "0.1.0")]
    pub backend_version: String,
    /// version of the OPOSSUM library (possibly including the git hash)
    #[schema(example = "0.6.0-18-g80cb67f (2025/02/19 15:29)")]
    pub opossum_version: String,
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
    uuid: Uuid,
    name: String,
    inverted: bool,
    node_type: String,
    input_ports: Vec<String>,
    output_ports: Vec<String>,
    gui_position: Option<(f64, f64)>,
}

impl NodeInfo {
    #[must_use]
    pub const fn new(
        uuid: Uuid,
        name: String,
        inverted: bool,
        node_type: String,
        input_ports: Vec<String>,
        output_ports: Vec<String>,
        gui_position: Option<(f64, f64)>,
    ) -> Self {
        Self {
            uuid,
            name,
            inverted,
            node_type,
            input_ports,
            output_ports,
            gui_position,
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
    pub fn gui_position(&self) -> (f64, f64) {
        self.gui_position
    }
    pub fn referring_node(&self) -> Uuid {
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
}
impl ConnectInfo {
    #[must_use]
    pub const fn new(
        src_uuid: Uuid,
        src_port: String,
        target_uuid: Uuid,
        target_port: String,
        distance: f64,
    ) -> Self {
        Self {
            src_uuid,
            src_port,
            target_uuid,
            target_port,
            distance,
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
    pub fn node_type(&self) -> &str {
        &self.node_type
    }
    pub fn gui_position(&self) -> (f64, f64) {
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

/// Structure holding an error mesaage
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct ErrorResponse {
    /// HTTP status
    #[schema(example = "400")]
    status: u16,
    /// Error category (normally corresponds to `OpossumError` enum)
    #[schema(example = "OpticScenery")]
    category: String,
    /// Description message of the error
    message: String,
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
        Self {
            status: 404, // StatusCode::NOT_FOUND,
            category: "api not found".to_string(),
            message: "the OPOSSUM API endpoint was not found".to_string(),
        }
    }
    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn category(&self) -> &str {
        &self.category
    }
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn message(&self) -> &str {
        &self.message
    }
}
impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
