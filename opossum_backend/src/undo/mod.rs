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
    analyzers::AnalyzerType,
    opm_document::OpmDocument,
    types::api_types::{
        AnalyzerItemDto, DocumentChange, JumpTarget, NodeEditorPanel, PumpScenarioItemDto,
    },
};
use uuid::Uuid;

use crate::error::BackEndErrorResponse;

mod amplifier_node_commands;
mod analyzer_commands;
mod edge_commands;
mod group_commands;
mod node_commands;
mod port_map_commands;
mod pump_scenario_commands;
mod viewport_commands;

pub use amplifier_node_commands::PatchAmplifierNodes;
pub use analyzer_commands::{PatchAnalyzer, RepositionAnalyzer};
pub use edge_commands::{EdgeSnapshot, UpdateEdgeDistance};
pub use group_commands::{GroupConversion, MoveNodes, ReroutedMapping};
pub use node_commands::{
    CascadedNode, NodeSnapshot, PatchNode, PatchPort, PatchProperty, capture_old_node_request,
};
pub use port_map_commands::{AddPortMap, RemovePortMap};
pub use pump_scenario_commands::{PatchAnalyzerPumpScenarios, PatchPumpScenario};
pub use viewport_commands::SetViewport;

/// A reversible document mutation. See the module docs for the overall design.
#[derive(Clone)]
pub enum Command {
    /// See [`NodeSnapshot`]. Inserts the node.
    AddNode(NodeSnapshot),
    /// See [`NodeSnapshot`]. Removes the node.
    RemoveNode(NodeSnapshot),
    /// See [`PatchNode`].
    PatchNode(Box<PatchNode>),
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
    PatchAnalyzer(Box<PatchAnalyzer>),
    /// See [`RepositionAnalyzer`].
    RepositionAnalyzer(RepositionAnalyzer),
    /// Re-inserts a previously removed pump scenario under its original id.
    AddPumpScenario(PumpScenarioItemDto),
    /// Removes the pump scenario with the given id. Also strips it from every analyzer's selection
    /// as a side effect of [`OpmDocument::remove_pump_scenario`] - a handler that deletes a scenario
    /// a user might have selected has to fold a [`Self::PatchAnalyzerPumpScenarios`] per affected
    /// analyzer into the same undo batch (see `delete_pump_scenario` in
    /// `opossum_backend::pump_scenarios`).
    RemovePumpScenario(PumpScenarioItemDto),
    /// See [`PatchPumpScenario`].
    PatchPumpScenario(PatchPumpScenario),
    /// See [`PatchAnalyzerPumpScenarios`].
    PatchAnalyzerPumpScenarios(PatchAnalyzerPumpScenarios),
    /// See [`PatchAmplifierNodes`]. Replaces the whole document-wide amplifier-candidate set.
    PatchAmplifierNodes(PatchAmplifierNodes),
    /// See [`MoveNodes`]. Moves nodes between two groups; `apply` swaps source/target to build its
    /// inverse and carries `affected_groups` through, so undo/redo can refresh every tab a reroute
    /// touched - not just source and target.
    MoveNodes(MoveNodes),
    /// See [`GroupConversion`]. Converts flat members into the group.
    InsertGroup(GroupConversion),
    /// See [`GroupConversion`]. Converts the group back into flat members.
    ExtractGroup(GroupConversion),
    /// See [`SetViewport`]. Moves the canvas camera (pan/zoom) of a tab; does not touch the document.
    SetViewport(SetViewport),
    /// Applies each sub-command in order; its inverse is the sub-commands' inverses in reverse order,
    /// so undoing/redoing a multi-step user action (paste, cut, multi-node drag) is a single step.
    Batch(Vec<Self>),
}

impl Command {
    /// Collapses a list of commands into a single undo/redo step: `None` if `commands` is empty, the
    /// command itself if there's exactly one (no needless `Batch` wrapper), otherwise a
    /// [`Command::Batch`].
    #[must_use]
    pub fn from_vec(mut commands: Vec<Self>) -> Option<Self> {
        match commands.len() {
            0 => None,
            1 => commands.pop(),
            _ => Some(Self::Batch(commands)),
        }
    }

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
            Self::PatchNode(cmd) => node_commands::apply_patch_node(document, *cmd),
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
            Self::PatchAnalyzer(cmd) => analyzer_commands::apply_patch_analyzer(document, *cmd),
            Self::RepositionAnalyzer(cmd) => {
                analyzer_commands::apply_reposition_analyzer(document, cmd)
            }
            Self::AddPumpScenario(cmd) => Ok(pump_scenario_commands::apply_add_pump_scenario(
                document, cmd,
            )),
            Self::RemovePumpScenario(cmd) => {
                pump_scenario_commands::apply_remove_pump_scenario(document, cmd)
            }
            Self::PatchPumpScenario(cmd) => {
                pump_scenario_commands::apply_patch_pump_scenario(document, cmd)
            }
            Self::PatchAnalyzerPumpScenarios(cmd) => {
                pump_scenario_commands::apply_patch_analyzer_pump_scenarios(document, cmd)
            }
            Self::PatchAmplifierNodes(cmd) => Ok(
                amplifier_node_commands::apply_patch_amplifier_nodes(document, cmd),
            ),
            Self::MoveNodes(cmd) => group_commands::apply_move_nodes(document, cmd),
            Self::InsertGroup(cmd) => group_commands::apply_insert_group(document, cmd),
            Self::ExtractGroup(cmd) => group_commands::apply_extract_group(document, cmd),
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

    /// Whether applying this command needs the transient whole-document rollback backup taken by the
    /// undo/redo handlers (see [`crate::document`]'s `apply_history_step`).
    ///
    /// `true` only for commands whose `apply` performs several fallible sub-steps and so could leave the
    /// document partially mutated if one fails partway: a [`Self::Batch`], a node insert/remove that also
    /// cascades and reconnects edges, a move, a group conversion, or a port-map cascade. Atomic
    /// single-mutation commands - a field patch, a single edge, an analyzer op, or a camera move (which
    /// doesn't touch the document at all) - either succeed or fail before mutating anything, so they don't
    /// need the (whole-document-serialize) safety net. The match is exhaustive rather than a wildcard, so
    /// a newly added variant must be classified deliberately instead of silently defaulting either way.
    pub const fn needs_rollback(&self) -> bool {
        match self {
            // Atomic: a single set/insert/remove/camera-move that can't leave a partial mutation behind.
            Self::PatchNode(_)
            | Self::PatchProperty(_)
            | Self::PatchPort(_)
            | Self::AddEdge(_)
            | Self::RemoveEdge(_)
            | Self::UpdateEdgeDistance(_)
            | Self::AddAnalyzer(_)
            | Self::RemoveAnalyzer(_)
            | Self::PatchAnalyzer(_)
            | Self::RepositionAnalyzer(_)
            | Self::AddPumpScenario(_)
            | Self::RemovePumpScenario(_)
            | Self::PatchPumpScenario(_)
            | Self::PatchAnalyzerPumpScenarios(_)
            | Self::PatchAmplifierNodes(_)
            | Self::SetViewport(_) => false,
            // Multi-step: several fallible sub-steps, so a mid-apply failure could tear the document.
            Self::AddNode(_)
            | Self::RemoveNode(_)
            | Self::AddPortMap(_)
            | Self::RemovePortMap(_)
            | Self::MoveNodes(_)
            | Self::InsertGroup(_)
            | Self::ExtractGroup(_)
            | Self::Batch(_) => true,
        }
    }

    /// Where the GUI should focus after applying this command's undo/redo (see [`JumpTarget`]): the tab,
    /// the node to select if the change is about one, and the node-editor panel to open if it belongs to
    /// one. Computed once here from the command, so the GUI needn't reconstruct it from the individual
    /// `DocumentChange`s. `root_id` is the scenery root uuid, used for analyzers (which live at the root).
    #[must_use]
    // One arm per command variant, each a small `JumpTarget` literal - long but flat and uniform.
    #[allow(clippy::too_many_lines)]
    pub fn jump_target(&self, root_id: Uuid) -> Option<JumpTarget> {
        match self {
            Self::PatchNode(patch_node) => Some(JumpTarget {
                graph_id: patch_node.parent_group_id,
                node: Some(patch_node.uuid),
                panel: node_commands::panel_for_update(&patch_node.new),
                source_port: None,
            }),
            Self::PatchProperty(PatchProperty {
                uuid,
                parent_group_id,
                ..
            }) => Some(JumpTarget {
                graph_id: *parent_group_id,
                node: Some(*uuid),
                panel: Some(NodeEditorPanel::Properties),
                source_port: None,
            }),
            Self::PatchPort(PatchPort {
                uuid,
                parent_group_id,
                ..
            }) => Some(JumpTarget {
                graph_id: *parent_group_id,
                node: Some(*uuid),
                panel: Some(NodeEditorPanel::PortConfig),
                source_port: None,
            }),
            Self::AddNode(cmd) | Self::RemoveNode(cmd) => Some(
                JumpTarget::new_from_graph_and_node_id(cmd.parent_group_id, cmd.node.uuid().ok()?),
            ),
            Self::AddEdge(cmd) | Self::RemoveEdge(cmd) => {
                Some(JumpTarget::new_from_graph_id(cmd.group_id))
            }
            Self::UpdateEdgeDistance(cmd) => Some(JumpTarget::new_from_graph_id(cmd.group_id)),
            Self::AddPortMap(cmd) => Some(JumpTarget::new_from_graph_id(cmd.group_id)),
            Self::RemovePortMap(cmd) => Some(JumpTarget::new_from_graph_id(cmd.group_id)),
            Self::AddAnalyzer(cmd) | Self::RemoveAnalyzer(cmd) => {
                Some(JumpTarget::new_from_graph_and_node_id(root_id, cmd.id))
            }
            Self::PatchAnalyzer(cmd) => Some(JumpTarget {
                graph_id: root_id,
                node: Some(cmd.id),
                panel: None,
                // Focus the exact source-port card whose mapping this patch changed, so undo/redo of an
                // analyzer source change opens and scrolls to it (see `changed_source_port`). `None` when
                // the change wasn't a source mapping (e.g. the analyzer type itself changed).
                source_port: changed_source_port(&cmd.old, &cmd.new),
            }),
            Self::RepositionAnalyzer(cmd) => {
                Some(JumpTarget::new_from_graph_and_node_id(root_id, cmd.id))
            }
            Self::MoveNodes(cmd) => Some(JumpTarget::new_from_graph_id(cmd.focus_group_id)),
            Self::InsertGroup(cmd) | Self::ExtractGroup(cmd) => {
                Some(JumpTarget::new_from_graph_id(cmd.parent_group_id))
            }
            // An operating point (and which analyzer runs in it) is not an object on the canvas:
            // there is nothing to jump to and no node to select, so undoing a scenario edit leaves
            // the view where the user left it. Same for the candidate set: it names nodes but isn't
            // itself a canvas object, and it can name several at once, so there is no single node to
            // focus.
            Self::AddPumpScenario(_)
            | Self::RemovePumpScenario(_)
            | Self::PatchPumpScenario(_)
            | Self::PatchAnalyzerPumpScenarios(_)
            | Self::PatchAmplifierNodes(_) => None,
            Self::SetViewport(cmd) => Some(JumpTarget::new_from_graph_id(cmd.to.graph_id)),
            Self::Batch(commands) => batch_jump_target(commands, root_id),
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
            Self::PatchProperty(PatchProperty {
                uuid,
                parent_group_id,
                ..
            })
            | Self::PatchPort(PatchPort {
                uuid,
                parent_group_id,
                ..
            }) => node_commands::describe_node_details_changed(*parent_group_id, *uuid),
            Self::AddEdge(cmd) => vec![DocumentChange::EdgeAdded {
                graph_id: cmd.group_id,
                connect_info: cmd.connect_info.clone(),
            }],
            Self::RemoveEdge(cmd) => vec![DocumentChange::EdgeRemoved {
                graph_id: cmd.group_id,
                connect_info: cmd.connect_info.clone(),
            }],
            Self::UpdateEdgeDistance(cmd) => vec![DocumentChange::EdgeUpdated {
                graph_id: cmd.group_id,
                connect_info: cmd.new.clone(),
            }],
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
            Self::AddAnalyzer(cmd) => vec![DocumentChange::AnalyzerAdded {
                analyzer: Box::new(cmd.clone()),
            }],
            Self::RemoveAnalyzer(cmd) => vec![DocumentChange::AnalyzerRemoved { id: cmd.id }],
            // The pump-scenario-selection patch changes only that selection, not the analyzer's own
            // config or position - the same refetch signal as any other analyzer detail change
            // covers both.
            Self::PatchAnalyzer(patch_analyzer) => {
                vec![DocumentChange::AnalyzerChanged {
                    id: patch_analyzer.id,
                }]
            }
            Self::PatchAnalyzerPumpScenarios(PatchAnalyzerPumpScenarios { id, .. }) => {
                vec![DocumentChange::AnalyzerChanged { id: *id }]
            }
            Self::AddPumpScenario(cmd) => vec![DocumentChange::PumpScenarioAdded {
                scenario: cmd.clone(),
            }],
            Self::RemovePumpScenario(cmd) => {
                vec![DocumentChange::PumpScenarioRemoved { id: cmd.id }]
            }
            Self::PatchPumpScenario(PatchPumpScenario { id, .. }) => {
                vec![DocumentChange::PumpScenarioChanged { id: *id }]
            }
            Self::PatchAmplifierNodes(_) => vec![DocumentChange::AmplifierNodesChanged],
            // Reports the position `apply` will set (`new_pos`), so the GUI moves the analyzer on the
            // canvas rather than only refreshing the details panel.
            Self::RepositionAnalyzer(cmd) => vec![DocumentChange::AnalyzerMoved {
                id: cmd.id,
                gui_position: cmd.new_pos,
            }],
            Self::MoveNodes(cmd) => group_commands::describe_move_nodes(cmd),
            Self::InsertGroup(GroupConversion {
                parent_group_id,
                affected_groups,
                ..
            }) => {
                // Inserting (re)creates the group, so refreshing its own tab is wanted.
                group_commands::describe_group_structure_change(
                    parent_group_id,
                    affected_groups,
                    None,
                )
            }
            Self::ExtractGroup(GroupConversion {
                parent_group_id,
                affected_groups,
                group,
                ..
            }) => {
                // Extracting *deletes* the group node, so its uuid is passed as `dissolved_group`:
                // excluded from the refresh set (refreshing a gone group 400s on its fetch GETs) and
                // turned into a `GraphClosed` so the GUI closes the dissolved group's own tab.
                group_commands::describe_group_structure_change(
                    parent_group_id,
                    affected_groups,
                    group.uuid().ok(),
                )
            }
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

/// The [`JumpTarget`] for a [`Command::Batch`]: the sub-command target with the highest focus priority (a
/// node-editor panel beats a node selection beats a bare tab), with ties broken by a stable key so undo and
/// redo agree - a batch reverses its sub-commands between the two directions, so a position-based "first"
/// would otherwise pick a different target each way.
fn batch_jump_target(commands: &[Command], root_id: Uuid) -> Option<JumpTarget> {
    let best = commands
        .iter()
        .filter_map(|command| command.jump_target(root_id))
        .max_by_key(|target| {
            // A specific sub-location (a node-editor panel, or an analyzer source-port card) outranks a
            // bare node selection, which outranks a bare tab. So a source-port deletion's batch (a re-added
            // source node + the analyzer source-mapping restore) focuses the analyzer's source card, not
            // the re-added canvas node.
            let has_detail = target.panel.is_some() || target.source_port.is_some();
            let priority = 2 * u8::from(has_detail) + u8::from(target.node.is_some());
            (priority, std::cmp::Reverse((target.graph_id, target.node)))
        })?;
    // A batch that focuses a specific node, panel, or source card (a paste, a node deletion - even one that
    // cascaded port maps away, or a source-port deletion) jumps to it. Only a purely-structural batch
    // consults the port-map cascade origin, so undo/redo of a port-map removal lands on the group the user
    // actually acted on.
    if best.node.is_some() || best.panel.is_some() || best.source_port.is_some() {
        return Some(best);
    }
    if let Some(graph_id) = port_map_cascade_origin(commands) {
        return Some(JumpTarget::new_from_graph_id(graph_id));
    }
    Some(best)
}

/// The source-port uuid whose analyzer mapping changed between `old` and `new` (added, removed, or a
/// changed builder), if the two are the same analyzer variant and exactly a source mapping changed.
///
/// Returns `None` when the analyzer *type* changed wholesale (no single source to focus). Used by
/// [`Command::jump_target`] to point an analyzer undo/redo at the exact source-port card that moved.
fn changed_source_port(old: &AnalyzerType, new: &AnalyzerType) -> Option<Uuid> {
    match (old, new) {
        (AnalyzerType::Energy(o), AnalyzerType::Energy(n)) => o.first_differing_source(n),
        (AnalyzerType::RayTrace(o), AnalyzerType::RayTrace(n)) => o.first_differing_source(n),
        (AnalyzerType::GhostFocus(o), AnalyzerType::GhostFocus(n)) => o.first_differing_source(n),
        _ => None,
    }
}

/// For a port-map cascade [`Command::Batch`] (a `RemovePortMap` tears down its own level plus every
/// ancestor that re-exposes it), the group whose *own* mapping was directly added/removed - the group the
/// user acted on. That's the innermost level: the one port-map command's `group_id` that isn't any level's
/// `parent_group_id`. Order-invariant, so undo and redo agree. `None` if the batch has no port-map commands.
fn port_map_cascade_origin(commands: &[Command]) -> Option<Uuid> {
    let mut levels: Vec<(Uuid, Uuid)> = Vec::new();
    collect_port_map_levels(commands, &mut levels);
    let parents: HashSet<Uuid> = levels.iter().map(|(_, parent)| *parent).collect();
    levels
        .iter()
        .map(|(group_id, _)| *group_id)
        .find(|group_id| !parents.contains(group_id))
}

/// Collects every port-map level `(group_id, parent_group_id)` from `commands`, descending into nested
/// [`Command::Batch`]es. A port-map removal that has been redone and then undone nests its single-level
/// `AddPortMap` inside a per-command `Batch` (`apply_remove_port_map` always returns a `Batch`, even for one
/// level), so a top-level-only scan would find nothing and lose the cascade's origin - making the jump drift
/// on the second undo. Recursing keeps the origin the same regardless of how the batch got nested.
fn collect_port_map_levels(commands: &[Command], out: &mut Vec<(Uuid, Uuid)>) {
    for command in commands {
        match command {
            Command::AddPortMap(m) => out.push((m.group_id, m.parent_group_id)),
            Command::RemovePortMap(m) => out.push((m.group_id, m.parent_group_id)),
            Command::Batch(sub) => collect_port_map_levels(sub, out),
            _ => {}
        }
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
            DocumentChange::GraphNeedsRefresh { graph_id, .. } => Some(*graph_id),
            _ => None,
        })
        .collect();

    let mut seen_refresh = HashSet::new();
    changes
        .into_iter()
        .filter(|change| match change {
            DocumentChange::GraphNeedsRefresh { graph_id, .. } => seen_refresh.insert(*graph_id),
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
            | DocumentChange::AnalyzerMoved { .. }
            | DocumentChange::PumpScenarioAdded { .. }
            | DocumentChange::PumpScenarioRemoved { .. }
            | DocumentChange::PumpScenarioChanged { .. }
            | DocumentChange::AmplifierNodesChanged
            | DocumentChange::GraphClosed { .. }
            | DocumentChange::ViewportChanged { .. } => true,
        })
        .collect()
}

/// Builds a `GraphNeedsRefresh` for each id in `ids`, deduplicated.
fn refresh_changes(ids: impl IntoIterator<Item = Uuid>) -> Vec<DocumentChange> {
    let mut ids: Vec<Uuid> = ids.into_iter().collect();
    ids.sort();
    ids.dedup();
    ids.into_iter()
        .map(|graph_id| DocumentChange::GraphNeedsRefresh { graph_id })
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use opossum_core::types::api_types::{UpdateNodeRequest, Viewport};
    use uuid::Uuid;

    /// The rollback backup is a whole-document serialize, so it must be taken only where a multi-step
    /// `apply` could tear the document partway through, and skipped for atomic commands - above all the
    /// very frequent `SetViewport` camera undos. Pins the classification so a change can't silently
    /// regress it (the exhaustive match also forces any new variant to be classified deliberately).
    #[test]
    fn needs_rollback_classifies_atomic_and_multistep_commands() {
        let viewport = |zoom| Viewport {
            graph_id: Uuid::new_v4(),
            zoom,
            shift: (0.0, 0.0),
        };
        // Atomic: no whole-document backup needed.
        assert!(
            !Command::SetViewport(SetViewport {
                from: viewport(1.0),
                to: viewport(2.0),
                coalescing: false,
            })
            .needs_rollback(),
            "a camera move never touches the document, so it needs no rollback"
        );
        assert!(
            !Command::PatchNode(Box::new(PatchNode {
                uuid: Uuid::new_v4(),
                parent_group_id: Uuid::new_v4(),
                old: UpdateNodeRequest::default(),
                new: UpdateNodeRequest::default(),
            }))
            .needs_rollback(),
            "a single-field patch can't leave a partial mutation"
        );
        // Multi-step: backup needed.
        assert!(
            Command::Batch(vec![]).needs_rollback(),
            "a batch chains several fallible sub-steps, so it needs rollback"
        );
    }

    /// The backend names the undo/redo focus target authoritatively (tab, node, panel) so the GUI doesn't
    /// reconstruct it. Pins the mapping for the field-patch commands (the panel cases) and an edge (tab
    /// only), which is the exact behavior the port-config-undo bug hinged on.
    #[test]
    fn jump_target_names_the_panel_node_and_tab() {
        use opossum_core::{
            prelude::PortType,
            types::api_types::{ConnectInfo, NodeEditorPanel, UpdatePortRequest},
        };

        let root = Uuid::new_v4();
        let graph = Uuid::new_v4();
        let node = Uuid::new_v4();

        // A port-config change opens Port Config on its node/tab - identically for aperture, coating, LIDT.
        let port = Command::PatchPort(PatchPort {
            uuid: node,
            parent_group_id: graph,
            port_type: PortType::Input,
            port_name: "input_1".to_string(),
            old: UpdatePortRequest::default(),
            new: UpdatePortRequest::default(),
        })
        .jump_target(root)
        .unwrap();
        assert_eq!(port.graph_id, graph);
        assert_eq!(port.node, Some(node));
        assert_eq!(port.panel, Some(NodeEditorPanel::PortConfig));

        // A name change belongs to General.
        let name = Command::PatchNode(Box::new(PatchNode {
            uuid: node,
            parent_group_id: graph,
            old: UpdateNodeRequest::default(),
            new: UpdateNodeRequest {
                name: Some("x".to_string()),
                ..Default::default()
            },
        }))
        .jump_target(root)
        .unwrap();
        assert_eq!(name.panel, Some(NodeEditorPanel::General));

        // A gui_position-only patch (a canvas drag) selects the node but opens no panel.
        let pos = Command::PatchNode(Box::new(PatchNode {
            uuid: node,
            parent_group_id: graph,
            old: UpdateNodeRequest::default(),
            new: UpdateNodeRequest {
                gui_position: Some(Some((1.0, 2.0))),
                ..Default::default()
            },
        }))
        .jump_target(root)
        .unwrap();
        assert_eq!(pos.node, Some(node));
        assert_eq!(pos.panel, None);

        // An edge change names the tab but no node/panel.
        let edge = Command::AddEdge(EdgeSnapshot {
            group_id: graph,
            connect_info: ConnectInfo::new(
                Uuid::new_v4(),
                "output_1".to_string(),
                Uuid::new_v4(),
                "input_1".to_string(),
                0.1,
                false,
            ),
        })
        .jump_target(root)
        .unwrap();
        assert_eq!(edge.graph_id, graph);
        assert_eq!(edge.node, None);
        assert_eq!(edge.panel, None);
    }

    /// A `Batch`'s focus is its highest-priority sub-command (a node selection beats a bare edge tab), so a
    /// paste (add node + reconnect edges) focuses the pasted node.
    #[test]
    fn batch_jump_target_prefers_the_node_over_the_edge() {
        use opossum_core::{nodes::create_node_ref, types::api_types::ConnectInfo};

        let root = Uuid::new_v4();
        let graph = Uuid::new_v4();
        let node_ref = create_node_ref("dummy").unwrap();
        let node_id = node_ref.uuid().unwrap();
        let jump = Command::Batch(vec![
            Command::AddNode(NodeSnapshot {
                parent_group_id: graph,
                node: node_ref,
                cascaded: Vec::new(),
                connections: Vec::new(),
            }),
            Command::AddEdge(EdgeSnapshot {
                group_id: graph,
                connect_info: ConnectInfo::new(
                    Uuid::new_v4(),
                    "output_1".to_string(),
                    Uuid::new_v4(),
                    "input_1".to_string(),
                    0.1,
                    false,
                ),
            }),
        ])
        .jump_target(root)
        .unwrap();
        assert_eq!(
            jump.node,
            Some(node_id),
            "the batch should focus the added node, not the edge's tab"
        );
    }

    /// The analyzer editor has no `NodeEditorPanel`, so an undo/redo of an analyzer source-mapping change is
    /// pointed at the exact source-port card via `JumpTarget::source_port`. Pins that a `PatchAnalyzer`
    /// whose source map changed names the analyzer node and the changed source, and that in a batch (a
    /// source-port node delete: re-add the node + restore the analyzer mapping) that source card outranks
    /// the bare re-added node.
    #[test]
    fn jump_target_names_the_changed_analyzer_source() {
        use opossum_core::{
            analyzers::energy::EnergyConfig,
            nodes::create_node_ref,
            prelude::{AnalyzerType, EnergyDataBuilder},
        };

        let root = Uuid::new_v4();
        let analyzer = Uuid::new_v4();
        let source = Uuid::new_v4();
        let mut with_source = EnergyConfig::default();
        with_source.map_source(source, EnergyDataBuilder::default());

        // A source-mapping change (here the source was removed; undo would restore it) focuses the analyzer
        // node and that exact source-port card, at the root tab where analyzers live.
        let patch = Command::PatchAnalyzer(Box::new(PatchAnalyzer {
            id: analyzer,
            old: AnalyzerType::Energy(with_source),
            new: AnalyzerType::Energy(EnergyConfig::default()),
        }));
        let jump = patch.clone().jump_target(root).unwrap();
        assert_eq!(jump.graph_id, root, "analyzers live at the root scenery");
        assert_eq!(jump.node, Some(analyzer));
        assert_eq!(
            jump.source_port,
            Some(source),
            "the jump must name the source-port card whose mapping changed"
        );

        // In a batch, the analyzer's source card wins over the re-added canvas source node.
        let source_node = create_node_ref("dummy").unwrap();
        let batch = Command::Batch(vec![
            Command::AddNode(NodeSnapshot {
                parent_group_id: root,
                node: source_node,
                cascaded: Vec::new(),
                connections: Vec::new(),
            }),
            patch,
        ])
        .jump_target(root)
        .unwrap();
        assert_eq!(
            batch.node,
            Some(analyzer),
            "the batch should focus the analyzer, not the re-added source node"
        );
        assert_eq!(
            batch.source_port,
            Some(source),
            "the batch should focus the analyzer's changed source card"
        );
    }
}
