//! Undo/redo command history for the live [`OpmDocument`] held in [`crate::app_state::AppState`].
//!
//! Each [`Command`] variant carries exactly the data needed to reverse one user-initiated document
//! mutation. [`Command::apply`] both performs the effect described by the variant *and* returns the
//! command that undoes it - the same method drives undo, redo, and (for creation-type mutations)
//! restoring a previously captured node/analyzer/group. HTTP handlers for simple field-patch endpoints
//! build a `Command` up front (capturing the old value) and call `apply` to perform the edit; handlers
//! for complex multi-step creation endpoints (paste, convert-to-group) keep their existing bodies and
//! only construct the inverse `Command` to push, since replaying "insert this already-built object back"
//! is uniform regardless of how the object was originally built.
use std::collections::HashSet;

use opossum_core::{
    opm_document::OpmDocument,
    types::api_types::{AnalyzerItemDto, DocumentChange},
};
use uuid::Uuid;

use crate::error::BackEndErrorResponse;

mod analyzer_commands;
mod edge_commands;
mod global_conf_commands;
mod group_commands;
mod node_commands;
mod port_map_commands;
mod viewport_commands;

pub use analyzer_commands::{PatchAnalyzer, RepositionAnalyzer};
pub use edge_commands::{EdgeSnapshot, UpdateEdgeDistance};
pub use global_conf_commands::PatchGlobalConf;
pub use group_commands::{GroupConversion, MoveNodes, ReroutedMapping};
pub use node_commands::{
    NodeSnapshot, PatchNode, PatchPort, PatchProperty, capture_old_node_request,
};
pub use port_map_commands::{AddPortMap, RemovePortMap, mapping_restore_commands};
pub use viewport_commands::SetViewport;

/// A reversible document mutation. See the module docs for the overall design.
#[derive(Clone)]
pub enum Command {
    /// See [`NodeSnapshot`]. Inserts the node.
    AddNode(NodeSnapshot),
    /// See [`NodeSnapshot`]. Removes the node.
    RemoveNode(NodeSnapshot),
    /// See [`PatchNode`].
    PatchNode(PatchNode),
    /// See [`PatchProperty`].
    PatchProperty(PatchProperty),
    /// See [`PatchPort`].
    PatchPort(PatchPort),
    /// See [`EdgeSnapshot`]. Connects the edge.
    AddEdge(EdgeSnapshot),
    /// See [`EdgeSnapshot`]. Disconnects the edge.
    RemoveEdge(EdgeSnapshot),
    /// See [`UpdateEdgeDistance`].
    UpdateEdgeDistance(UpdateEdgeDistance),
    /// See [`AddPortMap`].
    AddPortMap(AddPortMap),
    /// See [`RemovePortMap`].
    RemovePortMap(RemovePortMap),
    /// Re-inserts a previously removed analyzer under its original id.
    AddAnalyzer(AnalyzerItemDto),
    /// Removes the analyzer with the given id.
    RemoveAnalyzer(AnalyzerItemDto),
    /// See [`PatchAnalyzer`].
    PatchAnalyzer(PatchAnalyzer),
    /// See [`RepositionAnalyzer`].
    RepositionAnalyzer(RepositionAnalyzer),
    /// See [`MoveNodes`]. Moves nodes between two groups; `apply` swaps source/target to build its
    /// inverse and carries `affected_groups` through, so undo/redo can refresh every tab a reroute
    /// touched - not just source and target.
    MoveNodes(MoveNodes),
    /// See [`GroupConversion`]. Converts flat members into the group.
    InsertGroup(GroupConversion),
    /// See [`GroupConversion`]. Converts the group back into flat members.
    ExtractGroup(GroupConversion),
    /// See [`PatchGlobalConf`]. Replaces the document's global scenery config.
    PatchGlobalConf(PatchGlobalConf),
    /// See [`SetViewport`]. Moves the canvas camera (pan/zoom) of a tab; does not touch the document.
    SetViewport(SetViewport),
    /// Applies each sub-command in order; its inverse is the sub-commands' inverses in reverse order,
    /// so undoing/redoing a multi-step user action (paste, cut, multi-node drag) is a single step.
    Batch(Vec<Command>),
}

impl Command {
    /// Applies this command to `document` and returns the command that undoes it.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying graph operation fails (e.g. a captured node's parent group
    /// no longer exists, which should not happen given undo/redo's strict LIFO ordering).
    pub fn apply(self, document: &mut OpmDocument) -> Result<Self, BackEndErrorResponse> {
        match self {
            Self::AddNode(cmd) => node_commands::apply_add_node(document, cmd),
            Self::RemoveNode(cmd) => node_commands::apply_remove_node(document, cmd),
            Self::PatchNode(cmd) => node_commands::apply_patch_node(document, cmd),
            Self::PatchProperty(cmd) => node_commands::apply_patch_property(document, cmd),
            Self::PatchPort(cmd) => node_commands::apply_patch_port(document, cmd),
            Self::AddEdge(cmd) => edge_commands::apply_add_edge(document, cmd),
            Self::RemoveEdge(cmd) => edge_commands::apply_remove_edge(document, cmd),
            Self::UpdateEdgeDistance(cmd) => {
                edge_commands::apply_update_edge_distance(document, cmd)
            }
            Self::AddPortMap(cmd) => port_map_commands::apply_add_port_map(document, cmd),
            Self::RemovePortMap(cmd) => port_map_commands::apply_remove_port_map(document, cmd),
            Self::AddAnalyzer(cmd) => Ok(analyzer_commands::apply_add_analyzer(document, cmd)),
            Self::RemoveAnalyzer(cmd) => analyzer_commands::apply_remove_analyzer(document, cmd),
            Self::PatchAnalyzer(cmd) => analyzer_commands::apply_patch_analyzer(document, cmd),
            Self::RepositionAnalyzer(cmd) => {
                analyzer_commands::apply_reposition_analyzer(document, cmd)
            }
            Self::MoveNodes(cmd) => group_commands::apply_move_nodes(document, cmd),
            Self::InsertGroup(cmd) => group_commands::apply_insert_group(document, cmd),
            Self::ExtractGroup(cmd) => group_commands::apply_extract_group(document, cmd),
            Self::PatchGlobalConf(cmd) => {
                Ok(global_conf_commands::apply_patch_global_conf(document, cmd))
            }
            Self::SetViewport(cmd) => Ok(viewport_commands::apply_set_viewport(cmd)),
            Self::Batch(commands) => {
                let mut inverses = Vec::with_capacity(commands.len());
                for command in commands {
                    inverses.push(command.apply(document)?);
                }
                inverses.reverse();
                Ok(Self::Batch(inverses))
            }
        }
    }

    /// Describes the effect of applying this command, in the GUI-facing [`DocumentChange`] shape.
    ///
    /// Call this *before* consuming the command via [`Self::apply`] (or on a clone) - it describes
    /// what applying `self` is about to do, which is what actually happened once `apply` returns.
    ///
    /// # Errors
    ///
    /// Returns an error if building a captured node's [`opossum_core::types::api_types::NodeInfo`]
    /// fails (its `OpticRef` lock cannot be acquired) - only the node-carrying variants
    /// ([`Self::AddNode`]/[`Self::RemoveNode`] and batches containing them) are fallible.
    pub fn describe(&self) -> Result<Vec<DocumentChange>, BackEndErrorResponse> {
        Ok(match self {
            Self::AddNode(cmd) => node_commands::describe_add_node(cmd)?,
            Self::RemoveNode(cmd) => node_commands::describe_remove_node(cmd)?,
            Self::PatchNode(cmd) => node_commands::describe_patch_node(cmd),
            Self::PatchProperty(PatchProperty { uuid, .. })
            | Self::PatchPort(PatchPort { uuid, .. }) => {
                node_commands::describe_node_details_changed(uuid)
            }
            Self::AddEdge(cmd) => edge_commands::describe_add_edge(cmd),
            Self::RemoveEdge(cmd) => edge_commands::describe_remove_edge(cmd),
            Self::UpdateEdgeDistance(cmd) => edge_commands::describe_update_edge_distance(cmd),
            Self::AddPortMap(AddPortMap {
                group_id,
                parent_group_id,
                ..
            })
            | Self::RemovePortMap(RemovePortMap {
                group_id,
                parent_group_id,
                ..
            }) => port_map_commands::describe(group_id, parent_group_id),
            Self::AddAnalyzer(cmd) => analyzer_commands::describe_add_analyzer(cmd),
            Self::RemoveAnalyzer(cmd) => analyzer_commands::describe_remove_analyzer(&cmd.id),
            Self::PatchAnalyzer(PatchAnalyzer { id, .. })
            | Self::RepositionAnalyzer(RepositionAnalyzer { id, .. }) => {
                analyzer_commands::describe_analyzer_changed(id)
            }
            Self::MoveNodes(cmd) => group_commands::describe_move_nodes(cmd),
            Self::InsertGroup(GroupConversion {
                parent_group_id,
                affected_groups,
                ..
            })
            | Self::ExtractGroup(GroupConversion {
                parent_group_id,
                affected_groups,
                ..
            }) => group_commands::describe_group_structure_change(parent_group_id, affected_groups),
            Self::PatchGlobalConf(_) => global_conf_commands::describe_patch_global_conf(),
            Self::SetViewport(cmd) => viewport_commands::describe_set_viewport(cmd),
            Self::Batch(commands) => {
                let mut changes = Vec::new();
                for command in commands {
                    changes.extend(command.describe()?);
                }
                dedup_against_full_refreshes(changes)
            }
        })
    }
}

/// A `GraphNeedsRefresh { graph_id }` re-fetches everything in that tab (nodes, edges, port maps), so
/// any other change in the same batch that also targets `graph_id` would be double-applied once the
/// refresh has already picked it up - most visibly for list-valued state like `GraphStore.edges`, where
/// re-adding an already-present connection produces two identically-keyed elements and crashes the GUI's
/// keyed-list diffing. Keeps at most one `GraphNeedsRefresh` per `graph_id` and drops every tab-scoped
/// change (node/edge changes) whose `graph_id` is covered by one; analyzer/detail changes aren't
/// tab-scoped the same way and are left alone.
fn dedup_against_full_refreshes(changes: Vec<DocumentChange>) -> Vec<DocumentChange> {
    let refreshed: HashSet<Uuid> = changes
        .iter()
        .filter_map(|change| match change {
            DocumentChange::GraphNeedsRefresh { graph_id } => Some(*graph_id),
            _ => None,
        })
        .collect();

    let mut seen_refresh = HashSet::new();
    changes
        .into_iter()
        .filter(|change| match change {
            DocumentChange::GraphNeedsRefresh { graph_id } => seen_refresh.insert(*graph_id),
            DocumentChange::NodeAdded { graph_id, .. }
            | DocumentChange::NodeRemoved { graph_id, .. }
            | DocumentChange::NodePatched { graph_id, .. }
            | DocumentChange::EdgeAdded { graph_id, .. }
            | DocumentChange::EdgeRemoved { graph_id, .. }
            | DocumentChange::EdgeUpdated { graph_id, .. } => !refreshed.contains(graph_id),
            DocumentChange::NodeDetailsChanged { .. }
            | DocumentChange::AnalyzerAdded { .. }
            | DocumentChange::AnalyzerRemoved { .. }
            | DocumentChange::AnalyzerChanged { .. }
            | DocumentChange::ViewportChanged { .. } => true,
        })
        .collect()
}
