use crate::components::scenery_editor::{DragStatus, edges::edges_component::EdgeCreation};
use dioxus::html::geometry::euclid::default::{Point2D, Rect};
use opossum_core::{
    gain::GainModel,
    prelude::{AnalyzerType, PortType},
    types::api_types::{ConnectInfo, NewRefNode},
};
use std::{collections::HashSet, path::PathBuf};
use uuid::Uuid;

/// Represents all possible user or system actions within the graphs workspace.
/// These actions drive state updates such as selection, navigation, editing,
/// layout changes, and persistence.
pub enum GraphsWorkspaceAction {
    /// Clears all currently selected nodes in the given graph.
    ClearSelectedNodes {
        /// The ID of the graph.
        graph_id: Uuid,
    },

    /// Navigates to a port that is mapped to a node within a group.
    JumpToMappedPort {
        /// The ID of the mapped node.
        mapped_node_id: Uuid,
        /// The parent group (group ID and port name).
        parent: (Uuid, String),
    },
    /// Sets the currently active tab.
    SetActiveTab(Uuid),

    /// Removes multiple tabs by their IDs.
    RemoveTabs(Vec<Uuid>),

    /// Removes a node from the current selection.
    RemoveFromNodeSelection {
        /// The ID of the graph.
        graph_id: Uuid,
        /// The ID of the node to remove.
        node_id: Uuid,
    },
    /// Resynchronizes frontend state with backend state.
    Refresh,
    /// Adds a node to the current selection.
    AddToNodeSelection {
        /// The ID of the graph.
        graph_id: Uuid,
        /// The ID of the node to add.
        node_id: Uuid,
        /// Indicates whether the node is an optical node.
        is_optical: bool,
    },

    /// Sets the group that nodes can be dropped into during drag-and-drop.
    ///
    /// Contains the group ID and its z-index to determine layering.
    SetDropInGroup(Option<(Uuid, usize)>),

    /// Sets or clears the selection box used for area selection.
    SetSelectionBox(Option<Rect<f64>>),

    /// Updates the current drag status.
    SetDragStatus(DragStatus),

    /// Marks a node as active (focused/selected).
    SetNodeActive {
        /// The ID of the graph.
        graph_id: Uuid,
        /// The ID of the node.
        node_id: Uuid,
        /// Indicates whether the node is an optical node.
        is_optical_node: bool,
        /// Z-index of the node
        z_index: usize,
    },

    /// Handles a node click event, including modifier keys.
    NodeClick {
        /// The ID of the graph.
        graph_id: Uuid,
        /// The ID of the node.
        node_id: Uuid,
        /// Indicates whether the node is an optical node.
        is_optical_node: bool,
        /// Z-index of the node
        z_index: usize,
        /// Whether the Ctrl key was pressed during the click.
        ctrl_pressed: bool,
    },

    /// Sets the zoom level of the graph.
    SetZoom {
        /// The ID of the graph.
        graph_id: Uuid,
        /// The new zoom level.
        zoom: f64,
    },
    /// Sets the pan/shift of the graph viewport.
    SetShift {
        /// The ID of the graph.
        graph_id: Uuid,
        /// The new shift value.
        shift: Point2D<f64>,
    },

    /// Applies a drag movement to the graph or nodes.
    ApplyDrag {
        /// The ID of the graph.
        graph_id: Uuid,
        /// Relative movement delta.
        relative_shift: Point2D<f64>,
        /// Current zoom level.
        current_zoom: f64,
        /// Offset between mouse and graph coordinates.
        mouse_to_graph_shift: Point2D<f64>,
    },

    /// Sets or clears the edge currently being created.
    SetEdgeInCreation {
        /// The ID of the graph.
        graph_id: Uuid,
        /// The edge which is currently drawn
        edge_in_creation: Option<EdgeCreation>,
    },

    /// Requests the editor area dimensions or layout.
    GetEditorArea,

    /// Loads a graph workspace from a file.
    LoadFromFile(PathBuf),

    /// Saves the current graph workspace to a file.
    SaveToFile(PathBuf),

    /// Converts a set of nodes into a group.
    ConvertToGroup {
        /// Nodes to group.
        nodes: Vec<Uuid>,
        /// The ID of the graph.
        graph_id: Uuid,
    },

    /// Adds a new root scenery tab.
    AddRootSceneryTab {
        /// Name of the scenery
        name: String,
    },
    /// Maps a group-level port to a specific node port.
    MapNodePort {
        /// The type of the port (e.g. input or output).
        port_type: PortType,
        /// The name of the port on the group.
        group_port_name: String,
        /// The name of the port on the mapped node.
        mapped_node_port_name: String,
        /// The ID of the node whose port is being mapped.
        mapped_node_id: Uuid,
        /// The ID of the group containing the port mapping.
        group_id: Uuid,
    },

    /// Removes an existing mapping between a group port and a node port.
    RemovePortMap {
        /// The ID of the group containing the mapping.
        group_id: Uuid,
        /// The name of the group port whose mapping should be removed.
        group_port_name: String,
        /// The type of the port (e.g. input or output).
        port_type: PortType,
    },

    /// Deletes the root scenery and all associated data.
    DeleteRootScenery,

    /// Opens a tab displaying the contents of a group.
    OpenGroupTab {
        /// The ID of the group to open.
        group_id: Uuid,
        /// The display name of the group.
        group_name: String,
    },

    /// Adds a new optical node to the specified graph.
    AddOpticNode {
        /// The type identifier of the node to create.
        node_type: String,
        /// The ID of the graph where the node will be added.
        graph_id: Uuid,
    },

    /// Adds a new reference node to the specified graph.
    AddOpticReference {
        /// The reference node definition to insert.
        new_ref_node: NewRefNode,
        /// The ID of the graph where the node will be added.
        graph_id: Uuid,
    },

    /// Adds a new analyzer node to the specified graph.
    AddAnalyzer {
        /// The type of analyzer to create.
        analyzer_type: AnalyzerType,
        /// The ID of the graph where the analyzer will be added.
        graph_id: Uuid,
    },

    /// Optimizes the layout of nodes within the graph.
    OptimizeLayout {
        /// The ID of the graph to optimize.
        graph_id: Uuid,
    },
    /// Centers the graph within the viewport.
    ///
    /// If `save_changes` is true, the updated position is persisted.
    CenterGraph {
        /// The ID of the graph to center.
        graph_id: Uuid,
        /// Whether the new position should be saved.
        save_changes: bool,
        /// Whether the camera move should become an undo step. True for user-triggered centering
        /// (Layout menu, double-middle-click); false for programmatic centering (the graph view's
        /// mount effect on new project / file load), which must not make Undo available on a
        /// fresh document.
        record_undo: bool,
    },

    /// Adjusts zoom and position so the entire graph fits within the viewport.
    ///
    /// If `save_changes` is true, the updated state is persisted.
    ZoomToFit {
        /// The ID of the graph to adjust.
        graph_id: Uuid,
        /// Whether the new zoom/position should be saved.
        save_changes: bool,
        /// Whether this fit should be folded into the immediately preceding edit's undo step
        /// instead of becoming its own. True for the fitting that Auto Layout runs right after
        /// re-positioning the nodes, so a single undo reverts both the layout and the fit; false
        /// for a user-triggered zoom-to-fit, which is its own undo step.
        merge_into_previous_undo: bool,
    },

    /// Replaces all edges in the graph with a new set of connections.
    // UpdateEdges {
    //     /// The complete list of connections to apply.
    //     connections: Vec<ConnectInfo>,
    //     /// The ID of the graph being updated.
    //     graph_id: Uuid,
    // },

    /// Updates a single edge in the graph.
    UpdateEdge {
        /// The connection data representing the updated edge.
        connection: ConnectInfo,
        /// The ID of the graph containing the edge.
        graph_id: Uuid,
    },

    /// Deletes a specific edge from the graph.
    DeleteEdge {
        /// The connection identifying the edge to delete.
        connection: ConnectInfo,
        /// The ID of the graph containing the edge.
        graph_id: Uuid,
    },

    /// Adds a new edge to the graph.
    AddEdge {
        /// The connection describing the new edge.
        new_edge: ConnectInfo,
        /// The ID of the graph where the edge will be added.
        graph_id: Uuid,
    },

    /// Sets whether a node is inverted.
    InvertNode {
        /// Whether the node should be inverted.
        inverted: bool,
        /// The ID of the graph containing the node.
        graph_id: Uuid,
        /// The ID of the node to modify.
        node_id: Uuid,
    },

    /// Sets the display name of a node.
    SetNodeName {
        /// The new name of the node.
        name: String,
        /// The ID of the graph containing the node.
        graph_id: Uuid,
        /// The ID of the node to rename.
        node_id: Uuid,
        /// Whether this change should trigger persistence.
        needs_saving: bool,
    },

    /// Copies a set of nodes to the clipboard.
    CopyNodes {
        /// The IDs of the nodes to copy.
        nodes: HashSet<Uuid>,
    },

    /// Cuts (copies and removes) a set of nodes.
    CutNodes {
        /// The IDs of the nodes to cut.
        nodes: HashSet<Uuid>,
    },

    /// Pastes nodes into a graph at a given position.
    PasteNode {
        /// The target position in graph coordinates.
        pos: Point2D<f64>,
        /// The ID of the graph where nodes will be pasted.
        graph_id: Uuid,
    },

    /// Synchronizes one or more nodes' positions with an external update (e.g. the end of a drag, or
    /// an auto-layout pass). Batched into a single request/undo-step, even for a multi-node drag.
    SyncNodePositions {
        /// The moved nodes: id, whether it's an optical node (vs. an analyzer), and its new position.
        moves: Vec<(Uuid, bool, Point2D<f64>)>,
    },

    /// Deletes a whole selection - optical nodes and analyzers alike - together as a single undo step
    /// (see the backend's `delete_nodes`, which classifies each id and folds every removal into one
    /// undo entry). A single-node delete is just a one-element selection.
    DeleteNodes {
        /// The IDs of the selected nodes to delete.
        node_ids: Vec<Uuid>,
        /// The ID of the graph containing the nodes.
        graph_id: Uuid,
    },

    /// Moves nodes from one graph into another graph or group.
    DropNodesIntoGroup {
        /// The IDs of the nodes to move.
        nodes: Vec<Uuid>,
        /// The ID of the source graph.
        from_graph_id: Uuid,
        /// The ID of the destination graph or group.
        to_graph_id: Uuid,
    },

    /// Sets a volume node's `amp config` property, turning it into an amplifier or back into a
    /// passive component. An ordinary property patch - the node type does not change.
    ///
    /// Legacy path: kept for the properties panel's direct `amp config` edit (currently not
    /// reachable through any widget), superseded for everything user-facing by
    /// [`Self::SetScenarioGainModel`]. Does **not** touch the canvas marker (that now reflects the
    /// active pump scenario, not this property).
    SetAmpConfig {
        /// The ID of the node whose amplification model is being set.
        node_id: Uuid,
        /// The ID of the graph containing the node.
        graph_id: Uuid,
        /// The model to set. `GainModel::None` makes the node passive again.
        model: GainModel,
    },

    /// Sets which pump scenario the canvas and the context menu currently reflect - a GUI-only
    /// choice (see [`crate::ACTIVE_PUMP_SCENARIO`]), not a document edit. Refetches and bulk-syncs
    /// every open tab's amplifier markers to match.
    SetActivePumpScenario(Option<Uuid>),

    /// Sets the gain model a node runs with within one pump scenario - what the context menu's
    /// amplifier toggle sends. Unlike [`Self::SetAmpConfig`] this does not touch any node property;
    /// it patches the scenario, and mirrors the canvas marker only if `scenario_id` is the active
    /// scenario.
    SetScenarioGainModel {
        /// The scenario being edited.
        scenario_id: Uuid,
        /// The node whose gain model in that scenario is being set.
        node_id: Uuid,
        /// The graph containing the node, needed to update its canvas marker.
        graph_id: Uuid,
        /// The model to set. `GainModel::None` takes the node out of the scenario again.
        model: GainModel,
    },

    /// Brings a node into view: makes its graph the active tab (opening it first if needed) and
    /// selects it. Used by the amplifier overview to take the user to a listed node, which may sit
    /// in a group whose tab isn't even open.
    RevealNode {
        /// The ID of the node to reveal.
        node_id: Uuid,
        /// The ID of the graph containing the node.
        graph_id: Uuid,
    },

    /// Undoes the last checkpointed document edit.
    Undo,
    /// Redoes the last undone document edit.
    Redo,
}
