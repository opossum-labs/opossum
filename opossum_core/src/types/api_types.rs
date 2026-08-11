use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
};

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    analyzers::Analyzable,
    coatings::CoatingType,
    core_optics::{
        NodeAttrExt,
        optic_ports::{PortConfig, ValidatedLidt},
    },
    gain::active_amp_model,
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
    /// Name of the node's active amplification model, or `None` if it does not amplify (either
    /// because it has no volume at all or because its `amp config` is [`GainModel::None`]).
    ///
    /// This is a display marker, not the configuration itself: it lets a canvas show *that* a
    /// component is an amplifier without fetching every node's properties. The parameters live in
    /// the `amp config` property and are fetched only for the node being edited.
    #[schema(example = "Const")]
    pub amp_model: Option<String>,
}

impl NodeInfo {
    /// Create a `NodeInfo` struct from this [`Analyzable`]
    pub fn from_analyzable(
        node: &dyn Analyzable,
        gui_position: Option<Option<(f64, f64)>>,
    ) -> Self {
        Self {
            uuid: node.node_attr().uuid(),
            name: node.name().to_string(),
            inverted: node.inverted(),
            node_type: node.node_type().to_string(),
            input_ports: node.ports().names(&PortType::Input),
            output_ports: node.ports().names(&PortType::Output),
            gui_position: gui_position.unwrap_or_else(|| node.gui_position().map(|p| (p.x, p.y))),
            isometry: node.isometry(),
            alignment: node.alignment(),
            amp_model: active_amp_model(node.node_attr()),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Lens 1")]
    pub name: Option<String>,

    /// The new inverted status of the node
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = true)]
    pub inverted: Option<bool>,

    /// The new base isometry (position and rotation in 3D space)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub isometry: Option<Option<Isometry>>, // Option<Option> allows explicitly setting null!

    /// The new alignment isometry (local decenter and tilt)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub alignment: Option<Option<Isometry>>, // Option<Option> allows explicitly setting null!

    /// The GUI position on the 2D canvas
    #[serde(skip_serializing_if = "Option::is_none")]
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

/// One entry of a batched GUI-position update.
///
/// Sent at the end of a multi-node drag or an auto-layout pass - grouping every moved
/// node/analyzer into a single request keeps it a single undo/redo step, rather than one per node.
#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct PositionUpdate {
    pub uuid: Uuid,
    /// True for an optical node, false for an analyzer.
    pub is_optical: bool,
    #[schema(example = json!([100.0, 200.0]))]
    pub gui_position: (f64, f64),
}
// ============================================================================
// PORTS & PORT MAPPINGS
// ============================================================================

/// Response payload containing port configurations (Aperture, Coating, LIDT)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct NodePortsResponse {
    /// The input ports of the node (accounts for node inversion)
    pub inputs: BTreeMap<String, PortConfig>,
    /// The output ports of the node (accounts for node inversion)
    pub outputs: BTreeMap<String, PortConfig>,
}

/// Request payload for partial updates of a specific port
#[derive(Debug, Default, Serialize, Deserialize, ToSchema, Clone, PartialEq)]
pub struct UpdatePortRequest {
    /// The new aperture of the port
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aperture: Option<Aperture>,

    /// The new coating of the port
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coating: Option<CoatingType>,

    /// The new Laser Induced Damage Threshold
    #[serde(skip_serializing_if = "Option::is_none")]
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
#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
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

/// Response payload after removing a port map.
///
/// Reports every level of the cascade (the requested mapping, plus any mapping it was itself
/// chained through in an outer group) and whichever live connection the cascade finally tore down.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RemovePortMapResponse {
    /// True if a mapping was actually found and removed
    pub port_removed: bool,
    /// `(group_id, internal_node_id, external_port_name, port_type)` per cascade level actually
    /// removed, innermost (the requested group) first - same shape as
    /// `DeleteNodeResponse::removed_port_mappings`, so the GUI can reuse that exact handling.
    pub removed_port_mappings: Vec<(Uuid, Uuid, String, PortType)>,
    /// Live connection(s) disconnected where the cascade terminated, paired with the group whose
    /// graph held them - empty if the chain was already orphaned (nothing consuming it at the top).
    pub disconnected_connections: Vec<(Uuid, ConnectInfo)>,
}

/// Response payload after deleting a node.
///
/// Contains any external connections that were disconnected as a side effect - because they
/// depended on a port mapping of the deleted node (or of a node cascade-deleted alongside it)
/// that no longer exists.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DeleteNodeResponse {
    /// UUIDs of all nodes actually deleted (the target plus any cascaded reference nodes)
    pub deleted_nodes: Vec<Uuid>,
    /// Connections disconnected as a side effect, paired with the group they lived in
    pub disconnected_connections: Vec<(Uuid, ConnectInfo)>,
    /// `(group_id, node_id, external_port_name, port_type)` tuples for each port mapping removed as a
    /// side effect - lets the GUI prune exactly the affected entries from its own cached port-map list
    /// (instead of clearing and refetching a whole group's mappings, which would also drop still-valid
    /// mappings of other, untouched nodes) and shrink the group's own displayed port handles precisely.
    pub removed_port_mappings: Vec<(Uuid, Uuid, String, PortType)>,
    /// UUIDs of the analyzers deleted as part of the same selection. Analyzers live at document level
    /// (not in the scenery graph), so they're reported separately from `deleted_nodes`. Empty for a
    /// pure-node delete (e.g. the single-node `delete_node` endpoint).
    pub deleted_analyzers: Vec<Uuid>,
}

/// Response payload after moving nodes into a different group, reporting any connections rerouted (not
/// disconnected) as a side effect, plus which groups' port-map/exposed-port displays changed.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MoveNodesResponse {
    /// Connections newly created (a boundary sibling reconnected through a fresh port mapping on the
    /// destination group, or a direct reconnect when the other endpoint already lived there), paired
    /// with the group each lives in.
    pub new_connections: Vec<(Uuid, ConnectInfo)>,
    /// Connections torn down as a side effect - always alongside a matching `new_connections` entry that
    /// restores the same logical link through a new route.
    pub removed_connections: Vec<(Uuid, ConnectInfo)>,
    /// Groups whose port-map/exposed-port display changed and need a GUI refresh - always includes the
    /// destination group; includes the source group (or a higher ancestor) only when a pre-existing
    /// mapping was rerouted there.
    pub port_map_groups_changed: Vec<Uuid>,
    /// `(group_id, internal_node_id, external_port_name, port_type)` per port-map entry removed with no
    /// replacement under the same external name - same shape as
    /// [`DeleteNodeResponse::removed_port_mappings`], so the GUI can prune exactly that entry with the
    /// same precise handling, since a purely additive refresh wouldn't otherwise notice a key that's
    /// simply gone.
    pub removed_port_mappings: Vec<(Uuid, Uuid, String, PortType)>,
}

/// Response payload after converting nodes into a new subgroup.
///
/// Reports the new group plus anything that changed as a side effect - shaped like
/// [`MoveNodesResponse`] since converting is conceptually "create an empty group, then move the
/// selected nodes into it," reusing the same reroute machinery.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConvertToGroupResponse {
    /// The newly created group node.
    pub new_group: NodeInfo,
    /// Connections newly created (a boundary sibling reconnected through a fresh port mapping on the
    /// new group, or a direct reconnect when the other endpoint already lived there), paired with the
    /// group each lives in.
    pub new_connections: Vec<(Uuid, ConnectInfo)>,
    /// Connections torn down as a side effect - always alongside a matching `new_connections` entry
    /// that restores the same logical link through a new route.
    pub removed_connections: Vec<(Uuid, ConnectInfo)>,
    /// Groups whose port-map/exposed-port display changed and need a GUI refresh - always includes
    /// the new group; includes the source group too whenever a pre-existing mapping of a converted
    /// node was rerouted through the new group.
    pub port_map_groups_changed: Vec<Uuid>,
    /// `(group_id, internal_node_id, external_port_name, port_type)` per port-map entry removed with no
    /// replacement under the same external name. Always empty for this endpoint today (the equivalent
    /// of a move's "collapse" case, which converting into a brand-new, always-empty group can never
    /// trigger) - kept for parity with `MoveNodesResponse` so the GUI can share one code path.
    pub removed_port_mappings: Vec<(Uuid, Uuid, String, PortType)>,
}

/// Response payload after duplicating the copy cache into a group.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PasteNodesResponse {
    /// Freshly created optical nodes, keyed by the group each was inserted into - the paste
    /// target itself, plus one entry per nested group a pasted group brought along.
    pub pasted_nodes: HashMap<Uuid, Vec<NodeInfo>>,
    /// Freshly created analyzers - only ever non-empty when pasting into the scenery root, since
    /// analyzers live at document level.
    pub pasted_analyzers: Vec<AnalyzerItemDto>,
    /// Re-created connections between pasted nodes, keyed by the group each lives in.
    pub pasted_connections: HashMap<Uuid, Vec<ConnectInfo>>,
}

/// A node relocated to a *different* group by a cut+paste move.
///
/// Its uuid is unchanged; the GUI removes it from `from_group_id`'s tab and adds `node` (carrying its
/// updated position) to `to_group_id`'s tab.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RelocatedNode {
    /// The group the node was cut out of.
    pub from_group_id: Uuid,
    /// The group the node was pasted into.
    pub to_group_id: Uuid,
    /// The relocated node, with its new (shifted) `gui_position` already applied.
    pub node: NodeInfo,
}

/// Response payload after a UUID-preserving cut+paste - a *move* of the copy cache into a target group plus
/// a reposition to the paste location.
///
/// Unlike a duplicate [`PasteNodesResponse`], no new nodes are created and no originals are deleted: the
/// *same* nodes are relocated and/or repositioned, so a reference keyed on a cut node's uuid stays valid
/// with no remapping. A cut node, however, arrives **bare**: unlike a drag-and-drop move (which reroutes
/// links across the boundary, see [`MoveNodesResponse`]), a cut **cascade-deletes** every connection and
/// port mapping it carried, so `removed_connections` / `removed_port_mappings` describe that teardown and
/// `new_connections` is always empty. Nodes that were already in the target group are only repositioned
/// (the common "cut and paste in the same scenery" case) and keep their links.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CutNodesResponse {
    /// Nodes moved to a different group (same uuid), each with its updated `NodeInfo` for the target tab.
    pub relocated_nodes: Vec<RelocatedNode>,
    /// New GUI positions for cut nodes/analyzers that stayed in their group (and every cut analyzer, which
    /// can only ever be repositioned at the scenery root) - the GUI updates each element's position in
    /// place. Relocated nodes are *not* listed here; their position rides along in `relocated_nodes`.
    pub repositioned: Vec<PositionUpdate>,
    /// Always empty for a cut (it cascade-deletes rather than rerouting, so nothing is newly connected);
    /// kept for shape-compatibility with [`MoveNodesResponse::new_connections`].
    pub new_connections: Vec<(Uuid, ConnectInfo)>,
    /// Connections cascade-deleted from the cut nodes (their direct edges plus any terminal edge a
    /// torn-down port-map chain consumed), paired with their group - same shape/handling as
    /// [`MoveNodesResponse::removed_connections`].
    pub removed_connections: Vec<(Uuid, ConnectInfo)>,
    /// Groups whose port-map/exposed-port display changed and need a GUI refresh - same shape as
    /// [`MoveNodesResponse::port_map_groups_changed`].
    pub port_map_groups_changed: Vec<Uuid>,
    /// `(group_id, internal_node_id, external_port_name, port_type)` per port-map entry cascade-deleted
    /// from a cut node - same shape/handling as [`MoveNodesResponse::removed_port_mappings`].
    pub removed_port_mappings: Vec<(Uuid, Uuid, String, PortType)>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub analyzer_type: Option<AnalyzerType>,

    // The new position of the analyzer
    #[serde(skip_serializing_if = "Option::is_none")]
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
/// The payload returned to the GUI after successfully loading an OPM file.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoadDocumentResponse {
    pub name: String,
    pub needs_autolayout: bool,
}
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, Eq, PartialEq)]
pub struct SourcePortDto {
    pub uuid: Uuid,
    pub name: String,
}

/// One amplifying node of the document, for the amplifier overview panel.
///
/// Only nodes whose `amp config` is active appear as such an entry, so `amp_model` is a plain
/// `String` rather than an `Option` - see [`crate::gain::active_amp_model`].
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, Eq, PartialEq)]
pub struct AmplifierDto {
    pub uuid: Uuid,
    #[schema(example = "Main amplifier")]
    pub name: String,
    #[schema(example = "lens")]
    pub node_type: String,
    /// The group the node lives in. The overview panel needs it to open the right tab when the user
    /// asks to be taken to the node, and to offer filtering by subsystem.
    pub group_id: Uuid,
    /// Display name of that group (the document's own name for the root scenery).
    #[schema(example = "Frontend")]
    pub group_name: String,
    /// Display name of the node's active amplification model.
    #[schema(example = "Const")]
    pub amp_model: String,
}

// ============================================================================
// UNDO / REDO
// ============================================================================

/// Which of the node-editor sidebar's 5 accordion sections (`OpticalNodeEditor`) a document change
/// belongs to, so undo/redo can auto-select the node and open the panel whose value it just changed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, ToSchema)]
pub enum NodeEditorPanel {
    General,
    PortConfig,
    Properties,
    Positioning,
    Alignment,
}

/// Where the GUI should focus after an undo/redo, computed once by the backend from the command it
/// reversed.
///
/// Lets the GUI switch tab -> select node -> open panel directly, instead of reconstructing the target
/// from the individual [`DocumentChange`]s (which is order-sensitive and unreliable).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, ToSchema)]
pub struct JumpTarget {
    /// The tab (graph) the change lives in - always known.
    pub graph_id: Uuid,
    /// The node to select, if the change is about a specific one (`None` for an edge, a structural
    /// change, or a global-config change).
    pub node: Option<Uuid>,
    /// The node-editor panel to open, if the change belongs to one (`None` for a canvas-only or
    /// structural change).
    pub panel: Option<NodeEditorPanel>,
    /// The analyzer source-port card to open and scroll to, if the change was to an analyzer's source
    /// mapping (`None` otherwise). The analyzer editor has no [`NodeEditorPanel`] of its own, so this
    /// addresses the specific per-source card (keyed by the source port's uuid) directly.
    pub source_port: Option<Uuid>,
}

impl JumpTarget {
    #[must_use]
    pub const fn new_from_graph_id(graph_id: Uuid) -> Self {
        Self {
            graph_id,
            node: None,
            panel: None,
            source_port: None,
        }
    }
    #[must_use]
    pub const fn new_from_graph_and_node_id(graph_id: Uuid, node_id: Uuid) -> Self {
        Self {
            graph_id,
            node: Some(node_id),
            panel: None,
            source_port: None,
        }
    }
}

/// One user-visible effect of an undo/redo step.
///
/// Shaped so the GUI can react to it the same way it reacts to the corresponding normal
/// create/update/delete call - see each variant's matching endpoint/DTO. `GraphNeedsRefresh` is
/// the fallback for structural operations (port mapping, moving nodes between groups,
/// grouping/ungrouping) where reconstructing a fully precise incremental diff isn't worth the
/// complexity: the GUI just re-fetches that one tab's nodes/edges/port-maps, instead of every
/// open tab the way a whole-document reload would.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub enum DocumentChange {
    /// Mirrors `POST /api/nodes/{uuid}/children`'s response.
    NodeAdded { graph_id: Uuid, node: Box<NodeInfo> },
    /// Mirrors `DELETE /api/nodes/{uuid}`'s response (a single uuid here; cascaded reference-node
    /// deletions are reported as their own separate `NodeRemoved` entries).
    NodeRemoved { graph_id: Uuid, uuid: Uuid },
    /// Mirrors `PATCH /api/nodes/{uuid}`. Only `name`/`inverted`/`gui_position` are mirrored into the
    /// GUI's canvas state; a non-`None` `name` or `inverted` should also be applied to every node that
    /// references `uuid` (the same fan-out `PATCH .../name` already does), which the GUI resolves via
    /// `GET /api/nodes/{uuid}/references` exactly as it does for a normal rename.
    NodePatched {
        graph_id: Uuid,
        uuid: Uuid,
        name: Option<String>,
        inverted: Option<bool>,
        gui_position: Option<Option<(f64, f64)>>,
    },
    /// A custom property or port config changed. Not mirrored anywhere in the GUI's canvas state - if
    /// `uuid` is the currently selected node, the properties panel should simply re-fetch it.
    NodeDetailsChanged { uuid: Uuid, graph_id: Uuid },
    /// Mirrors `POST /api/nodes/{uuid}/connections`.
    EdgeAdded {
        graph_id: Uuid,
        connect_info: ConnectInfo,
    },
    /// Mirrors `DELETE /api/nodes/{uuid}/connections`.
    EdgeRemoved {
        graph_id: Uuid,
        connect_info: ConnectInfo,
    },
    /// Mirrors `PATCH /api/nodes/{uuid}/connections` (distance only).
    EdgeUpdated {
        graph_id: Uuid,
        connect_info: ConnectInfo,
    },
    /// Mirrors `POST /api/analyzers`.
    AnalyzerAdded { analyzer: AnalyzerItemDto },
    /// Mirrors `DELETE /api/analyzers/{uuid}`.
    AnalyzerRemoved { id: Uuid },
    /// The analyzer's config changed (not its position); the properties panel should re-fetch it.
    AnalyzerChanged { id: Uuid },
    /// The analyzer moved on the canvas to `gui_position`. Emitted when undoing/redoing an analyzer
    /// reposition, so the GUI can move it back (a details refetch alone doesn't touch canvas state).
    AnalyzerMoved { id: Uuid, gui_position: (f64, f64) },
    /// One tab's nodes/edges/port-maps should be re-fetched from scratch (see the type's own doc
    /// comment for which operations use this).
    GraphNeedsRefresh { graph_id: Uuid },
    /// The group `graph_id` no longer exists (e.g. undoing a convert-to-group dissolves it), so its
    /// tab should be closed if open. Distinct from a node removal in the *parent* view - the parent
    /// is refreshed separately; this closes the dissolved group's own tab.
    GraphClosed { graph_id: Uuid },
    /// The canvas camera (pan/zoom) of `graph_id` should move to `zoom`/`shift`. Emitted when undoing or
    /// redoing a viewport change (see `Command::SetViewport`); purely a GUI/camera effect, never touches
    /// the document model.
    ViewportChanged {
        graph_id: Uuid,
        zoom: f64,
        shift: (f64, f64),
    },
}

/// A per-tab canvas viewport (pan/zoom).
///
/// Purely a GUI concern - never part of the saved `.opm` document - but round-tripped through
/// the backend so a camera move can be a reversible entry on the undo stack (see
/// `Command::SetViewport`).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ToSchema)]
pub struct Viewport {
    /// The tab (graph) the viewport belongs to.
    pub graph_id: Uuid,
    /// Zoom factor.
    pub zoom: f64,
    /// Pan offset `(x, y)` in screen space.
    pub shift: (f64, f64),
}

/// Body of `POST /api/document/viewport_change`.
///
/// A camera move from `before` to `after`, plus whether it may coalesce with an immediately
/// preceding *coalescing* move on the same tab into one undo step. Set `coalesce: true` for
/// scroll-zoom ticks (a whole burst = one step) and `false` for discrete gestures (pan, center,
/// zoom-to-fit) so different gesture types stay separate undo steps.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct ViewportChangeRequest {
    /// The viewport before the gesture (undo restores this).
    pub before: Viewport,
    /// The viewport after the gesture (redo restores this).
    pub after: Viewport,
    /// Whether this move may merge with a preceding coalescing move on the same tab.
    pub coalesce: bool,
    /// Whether this move should be folded into the immediately preceding edit's undo entry (if that
    /// entry is a batch) instead of pushing its own. Set for Auto Layout's post-layout fit, so a
    /// single undo reverts both the node re-positioning and the fit.
    pub merge_into_previous: bool,
}

/// Response returned by `POST /api/document/undo` and `POST /api/document/redo`.
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct UndoRedoResponse {
    pub changes: Vec<DocumentChange>,
    /// Where the GUI should focus after applying `changes` (see [`JumpTarget`]); `None` when the step has
    /// no meaningful focus (e.g. a global-config change).
    pub jump: Option<JumpTarget>,
    pub can_undo: bool,
    pub can_redo: bool,
}
