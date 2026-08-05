//! `apply`/`describe` bodies for the group port-mapping [`Command`] variants: [`Command::AddPortMap`],
//! [`Command::RemovePortMap`].
use opossum_core::{
    opm_document::OpmDocument,
    types::api_types::{AddPortMappingRequest, DocumentChange, RemovePortMapQuery},
};
use uuid::Uuid;

use super::{Command, EdgeSnapshot, refresh_changes};
use crate::{
    error::BackEndErrorResponse,
    helper_functions::{PortMapCascadeRemoval, RemovedPortMapLevel, map_port},
};

/// Exposes an internal node's port as an external port on `group_id`. `parent_group_id` (`group_id`'s own
/// parent) is carried only so undo/redo responses can tell the GUI which tab renders `group_id`'s node
/// box (and therefore its exposed-port list) - a group's ports are drawn on its node as shown in its
/// *parent's* tab, not inside the group's own tab.
#[derive(Clone)]
pub struct AddPortMap {
    pub group_id: Uuid,
    pub parent_group_id: Uuid,
    pub request: AddPortMappingRequest,
}

impl From<&RemovedPortMapLevel> for AddPortMap {
    /// Builds the mapping-restoring command for one level torn down by
    /// [`crate::helper_functions::remove_port_map_cascade`]. Takes a reference since callers
    /// still need the levels afterward (e.g. to build a response).
    fn from(level: &RemovedPortMapLevel) -> Self {
        Self {
            group_id: level.group_id,
            parent_group_id: level.parent_group_id,
            request: AddPortMappingRequest {
                internal_node_id: level.internal_node_id,
                internal_port_name: level.internal_port_name.clone(),
                external_port_name: level.external_port_name.clone(),
                port_type: level.port_type,
            },
        }
    }
}

impl From<&PortMapCascadeRemoval> for Command {
    /// Builds the single undo step that restores everything a cascade removal tore down: one
    /// [`Command::AddPortMap`] per removed level, innermost first (each level's own restoration
    /// depends on the next-inner level already existing, since an outer level's "internal port"
    /// is the inner group's own currently-mapped external name), then one [`Command::AddEdge`]
    /// per live connection the cascade disconnected.
    fn from(cascade: &PortMapCascadeRemoval) -> Self {
        let mut inverse =
            Vec::with_capacity(cascade.levels.len() + cascade.disconnected_connections.len());
        for level in &cascade.levels {
            inverse.push(Self::AddPortMap(level.into()));
        }
        for (owning_group_id, connect_info) in &cascade.disconnected_connections {
            inverse.push(Self::AddEdge(EdgeSnapshot {
                group_id: *owning_group_id,
                connect_info: connect_info.clone(),
            }));
        }
        Self::Batch(inverse)
    }
}

/// Removes an external port mapping from `group_id`, disconnecting anything wired to it first.
#[derive(Clone)]
pub struct RemovePortMap {
    pub group_id: Uuid,
    pub parent_group_id: Uuid,
    pub query: RemovePortMapQuery,
}

/// Exposes an internal node's port as an external port on `cmd.group_id`, returning the
/// [`Command::RemovePortMap`] that undoes it.
///
/// # Errors
///
/// Returns an error if `group_id` doesn't resolve to a group, or the internal node/port doesn't exist.
pub(super) fn apply_add_port_map(
    document: &mut OpmDocument,
    cmd: AddPortMap,
) -> Result<Command, BackEndErrorResponse> {
    let AddPortMap {
        group_id,
        parent_group_id,
        request,
    } = cmd;
    document.scenery_mut().with_group_node_mut(group_id, |g| {
        map_port(
            g,
            request.port_type,
            request.internal_node_id,
            &request.internal_port_name,
            &request.external_port_name,
        )
    })??;
    Ok(Command::RemovePortMap(RemovePortMap {
        group_id,
        parent_group_id,
        query: RemovePortMapQuery {
            external_port_name: request.external_port_name,
            port_type: request.port_type,
        },
    }))
}

/// Removes an external port mapping from `cmd.group_id`, cascading outward through any group it's
/// itself chained through (see [`crate::helper_functions::remove_port_map_cascade`]) until it
/// reaches and disconnects a live connection, or runs out of chain. Returns a [`Command::Batch`]
/// of one [`Command::AddPortMap`] per level removed (innermost first) plus one
/// [`Command::AddEdge`] per connection torn down, since undoing the removal means restoring all
/// of it.
///
/// A bare [`Command::RemovePortMap`] is only ever constructed as the undo of an [`AddPortMap`] -
/// and adding a mapping can never have anything chained onto it yet (mapping requires the port
/// not already be connected), so by LIFO undo/redo ordering, anything chained onto it afterward
/// must already be undone before this ever runs. The cascade discovered here is therefore always
/// exactly the 1 level `cmd` itself names - this only calls the shared multi-level helper to avoid
/// duplicating its logic, not because more than 1 level is actually expected here.
///
/// # Errors
///
/// Returns an error if `group_id` doesn't resolve to a group or `query.external_port_name` isn't
/// currently mapped.
pub(super) fn apply_remove_port_map(
    document: &mut OpmDocument,
    cmd: RemovePortMap,
) -> Result<Command, BackEndErrorResponse> {
    let RemovePortMap {
        group_id,
        parent_group_id,
        query,
    } = cmd;
    let RemovePortMapQuery {
        external_port_name,
        port_type,
    } = query;

    let Some(cascade) = crate::helper_functions::remove_port_map_cascade(
        document.scenery_mut(),
        group_id,
        &external_port_name,
        port_type,
    )?
    else {
        return Err(BackEndErrorResponse::new(
            400,
            "Opossum",
            &format!("Port mapping '{external_port_name}' not found"),
        ));
    };
    debug_assert_eq!(
        cascade.levels.len(),
        1,
        "a bare RemovePortMap must only ever discover exactly the 1 level it names - see this \
         function's own doc comment"
    );
    debug_assert_eq!(
        cascade.levels.first().map(|l| l.parent_group_id),
        Some(parent_group_id),
        "captured parent_group_id must match the group's actual current parent"
    );

    Ok(Command::from(&cascade))
}

/// Describes the effect of a [`Command::AddPortMap`] or [`Command::RemovePortMap`] in the GUI-facing
/// [`DocumentChange`] shape. A port-map change touches two tabs, like `MoveNodes` touches two: the
/// group's own tab (where `mapped_ports` drives the "mapped" symbol on the internal node's port) and its
/// parent's tab (where the group's own exposed-port list is rendered on its node box). Which tab to jump to
/// is decided by the step's [`crate::undo::Command::jump_target`], not per-refresh.
pub(super) fn describe(group_id: &Uuid, parent_group_id: &Uuid) -> Vec<DocumentChange> {
    refresh_changes([*group_id, *parent_group_id])
}
