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
    nodes::GraphDelta,
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
mod graph_commands;
mod group_commands;
mod node_commands;
mod port_map_commands;
mod pump_scenario_commands;
mod viewport_commands;

pub use amplifier_node_commands::PatchAmplifierNodes;
pub use analyzer_commands::{PatchAnalyzer, PatchAnalyzerName, RepositionAnalyzer};
pub use edge_commands::{EdgeSnapshot, UpdateEdgeDistance};
pub use group_commands::{GroupConversion, MoveNodes, ReroutedMapping};
pub use node_commands::{
    CascadedNode, NodeSnapshot, PatchNode, PatchPort, PatchProperty, capture_old_node_request,
};
pub use port_map_commands::{AddPortMap, RemovePortMap};
pub use pump_scenario_commands::{PatchAnalyzerPumpScenarios, PatchPumpScenario};
pub use viewport_commands::SetViewport;

/// A reversible document mutation.
#[derive(Clone)]
pub enum Command {
    /// Reverts a graph mutation using core's central `apply_undo` dispatcher.
    UndoGraph(Box<GraphDelta>),
    /// Re-executes a graph mutation during redo.
    RedoGraph(Box<GraphDelta>),

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
    /// See [`PatchAnalyzerName`].
    PatchAnalyzerName(PatchAnalyzerName),
    /// Re-inserts a previously removed pump scenario under its original id.
    AddPumpScenario(PumpScenarioItemDto),
    /// Removes the pump scenario with the given id.
    RemovePumpScenario(PumpScenarioItemDto),
    /// See [`PatchPumpScenario`].
    PatchPumpScenario(PatchPumpScenario),
    /// See [`PatchAnalyzerPumpScenarios`].
    PatchAnalyzerPumpScenarios(PatchAnalyzerPumpScenarios),
    /// See [`PatchAmplifierNodes`]. Replaces the whole document-wide amplifier-candidate set.
    PatchAmplifierNodes(PatchAmplifierNodes),
    /// See [`MoveNodes`]. Moves nodes between two groups.
    MoveNodes(MoveNodes),
    /// See [`GroupConversion`]. Converts flat members into the group.
    InsertGroup(GroupConversion),
    /// See [`GroupConversion`]. Converts the group back into flat members.
    ExtractGroup(GroupConversion),
    /// See [`SetViewport`]. Moves the canvas camera (pan/zoom) of a tab; does not touch the document.
    SetViewport(SetViewport),
    /// Applies each sub-command in order; its inverse is the sub-commands' inverses in reverse order.
    Batch(Vec<Self>),
}

impl Command {
    /// Collapses a list of commands into a single undo/redo step.
    #[must_use]
    pub fn from_vec(mut commands: Vec<Self>) -> Option<Self> {
        match commands.len() {
            0 => None,
            1 => commands.pop(),
            _ => Some(Self::Batch(commands)),
        }
    }

    /// Applies this command to `document` and returns the command that undoes it.
    pub fn apply(self, document: &mut OpmDocument) -> Result<Self, BackEndErrorResponse> {
        match self {
            Self::UndoGraph(delta) => graph_commands::apply_undo_graph(document, *delta),
            Self::RedoGraph(delta) => graph_commands::apply_redo_graph(document, *delta),
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
            Self::PatchAnalyzerName(cmd) => {
                analyzer_commands::apply_patch_analyzer_name(document, cmd)
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

    /// Whether applying this command needs the transient whole-document rollback backup.
    pub const fn needs_rollback(&self) -> bool {
        match self {
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
            | Self::PatchAnalyzerName(_)
            | Self::AddPumpScenario(_)
            | Self::RemovePumpScenario(_)
            | Self::PatchPumpScenario(_)
            | Self::PatchAnalyzerPumpScenarios(_)
            | Self::PatchAmplifierNodes(_)
            | Self::SetViewport(_) => false,
            Self::UndoGraph(_)
            | Self::RedoGraph(_)
            | Self::AddNode(_)
            | Self::RemoveNode(_)
            | Self::AddPortMap(_)
            | Self::RemovePortMap(_)
            | Self::MoveNodes(_)
            | Self::InsertGroup(_)
            | Self::ExtractGroup(_)
            | Self::Batch(_) => true,
        }
    }

    /// Where the GUI should focus after applying this command's undo/redo.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn jump_target(&self, root_id: Uuid) -> Option<JumpTarget> {
        match self {
            Self::UndoGraph(delta) => {
                graph_commands::jump_target_for_graph_delta(delta, true, root_id)
            }
            Self::RedoGraph(delta) => {
                graph_commands::jump_target_for_graph_delta(delta, false, root_id)
            }
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
                JumpTarget::new_from_graph_and_node_id(cmd.parent_group_id, cmd.node.uuid()),
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
                source_port: changed_source_port(&cmd.old, &cmd.new),
            }),
            Self::RepositionAnalyzer(cmd) => {
                Some(JumpTarget::new_from_graph_and_node_id(root_id, cmd.id))
            }
            Self::PatchAnalyzerName(cmd) => {
                Some(JumpTarget::new_from_graph_and_node_id(root_id, cmd.id))
            }
            Self::MoveNodes(cmd) => Some(JumpTarget::new_from_graph_id(cmd.focus_group_id)),
            Self::InsertGroup(cmd) | Self::ExtractGroup(cmd) => {
                Some(JumpTarget::new_from_graph_id(cmd.parent_group_id))
            }
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
    pub fn describe(&self) -> Result<Vec<DocumentChange>, BackEndErrorResponse> {
        Ok(match self {
            Self::UndoGraph(delta) => graph_commands::describe_graph_delta(delta, true),
            Self::RedoGraph(delta) => graph_commands::describe_graph_delta(delta, false),
            Self::AddNode(cmd) => node_commands::describe_add_node(cmd),
            Self::RemoveNode(cmd) => node_commands::describe_remove_node(cmd),
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
            Self::RepositionAnalyzer(cmd) => vec![DocumentChange::AnalyzerMoved {
                id: cmd.id,
                gui_position: cmd.new_pos,
            }],
            Self::PatchAnalyzerName(cmd) => vec![DocumentChange::AnalyzerRenamed {
                id: cmd.id,
                name: cmd.new.clone(),
            }],
            Self::MoveNodes(cmd) => group_commands::describe_move_nodes(cmd),
            Self::InsertGroup(GroupConversion {
                parent_group_id,
                affected_groups,
                ..
            }) => group_commands::describe_group_structure_change(
                parent_group_id,
                affected_groups,
                None,
            ),
            Self::ExtractGroup(GroupConversion {
                parent_group_id,
                affected_groups,
                group,
                ..
            }) => group_commands::describe_group_structure_change(
                parent_group_id,
                affected_groups,
                Some(group.uuid()),
            ),
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

fn batch_jump_target(commands: &[Command], root_id: Uuid) -> Option<JumpTarget> {
    let best = commands
        .iter()
        .filter_map(|command| command.jump_target(root_id))
        .max_by_key(|target| {
            let has_detail = target.panel.is_some() || target.source_port.is_some();
            let priority = 2 * u8::from(has_detail) + u8::from(target.node.is_some());
            (priority, std::cmp::Reverse((target.graph_id, target.node)))
        })?;

    if best.node.is_some() || best.panel.is_some() || best.source_port.is_some() {
        return Some(best);
    }
    if let Some(graph_id) = port_map_cascade_origin(commands) {
        return Some(JumpTarget::new_from_graph_id(graph_id));
    }
    Some(best)
}

fn changed_source_port(old: &AnalyzerType, new: &AnalyzerType) -> Option<Uuid> {
    match (old, new) {
        (AnalyzerType::Energy(o), AnalyzerType::Energy(n)) => o.first_differing_source(n),
        (AnalyzerType::RayTrace(o), AnalyzerType::RayTrace(n)) => o.first_differing_source(n),
        (AnalyzerType::GhostFocus(o), AnalyzerType::GhostFocus(n)) => o.first_differing_source(n),
        _ => None,
    }
}

fn port_map_cascade_origin(commands: &[Command]) -> Option<Uuid> {
    let mut levels: Vec<(Uuid, Uuid)> = Vec::new();
    collect_port_map_levels(commands, &mut levels);
    let parents: HashSet<Uuid> = levels.iter().map(|(_, parent)| *parent).collect();
    levels
        .iter()
        .map(|(group_id, _)| *group_id)
        .find(|group_id| !parents.contains(group_id))
}

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
            | DocumentChange::AnalyzerRenamed { .. }
            | DocumentChange::PumpScenarioAdded { .. }
            | DocumentChange::PumpScenarioRemoved { .. }
            | DocumentChange::PumpScenarioChanged { .. }
            | DocumentChange::AmplifierNodesChanged
            | DocumentChange::GraphClosed { .. }
            | DocumentChange::ViewportChanged { .. } => true,
        })
        .collect()
}

fn refresh_changes(ids: impl IntoIterator<Item = Uuid>) -> Vec<DocumentChange> {
    let mut ids: Vec<Uuid> = ids.into_iter().collect();
    ids.sort();
    ids.dedup();
    ids.into_iter()
        .map(|graph_id| DocumentChange::GraphNeedsRefresh { graph_id })
        .collect()
}