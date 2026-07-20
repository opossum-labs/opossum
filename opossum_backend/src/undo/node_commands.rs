//! `apply`/`describe` bodies for the node-lifecycle and node-field-patch [`Command`] variants:
//! [`Command::AddNode`], [`Command::RemoveNode`], [`Command::PatchNode`], [`Command::PatchProperty`],
//! [`Command::PatchPort`].
use nalgebra::Point2;
use opossum_core::{
    core_optics::{NodeAttr, OpticRef},
    error::OpossumError,
    opm_document::OpmDocument,
    prelude::{PortType, Proptype},
    types::api_types::{ConnectInfo, DocumentChange, NodeInfo, UpdateNodeRequest, UpdatePortRequest},
    utils::LockExt,
};
use uuid::Uuid;

use super::Command;
use crate::{error::BackEndErrorResponse, helper_functions::connect_from_info};

/// Inserts `node` (and any `cascaded` reference nodes) into `parent_group_id`, reconnecting
/// `connections` - the node's own connections in `parent_group_id`'s graph at the time it was captured
/// (e.g. by a delete or cut), so undoing a deletion restores both the node and its wiring.
#[derive(Clone)]
pub struct AddNode {
    pub parent_group_id: Uuid,
    pub node: OpticRef,
    pub cascaded: Vec<(Uuid, OpticRef)>,
    pub connections: Vec<ConnectInfo>,
}

/// Removes the node identified by `node`'s own uuid from the graph (cascading to reference nodes that
/// point at it, mirroring `NodeGroup::delete_node`'s existing behavior). `connections` is carried through
/// unchanged so a subsequent undo (via the `AddNode` this produces) can restore it.
#[derive(Clone)]
pub struct RemoveNode {
    pub parent_group_id: Uuid,
    pub node: OpticRef,
    pub cascaded: Vec<(Uuid, OpticRef)>,
    pub connections: Vec<ConnectInfo>,
}

/// Applies `new`'s populated fields to the node's standard properties; `old` mirrors the same
/// `Option`-shape with the values that were in place beforehand. `parent_group_id` is carried only so
/// undo/redo responses can tell the GUI which tab's local mirror to update - it plays no role in applying
/// the patch itself (node lookup is recursive by uuid).
#[derive(Clone)]
pub struct PatchNode {
    pub uuid: Uuid,
    pub parent_group_id: Uuid,
    pub old: UpdateNodeRequest,
    pub new: UpdateNodeRequest,
}

/// Sets a single custom property to `new`; `old` is the value it had before.
#[derive(Clone)]
pub struct PatchProperty {
    pub uuid: Uuid,
    pub parent_group_id: Uuid,
    pub prop_name: String,
    pub old: Proptype,
    pub new: Proptype,
}

/// Applies `new`'s populated fields to one port's config; `old` mirrors the same shape.
#[derive(Clone)]
pub struct PatchPort {
    pub uuid: Uuid,
    pub parent_group_id: Uuid,
    pub port_type: PortType,
    pub port_name: String,
    pub old: UpdatePortRequest,
    pub new: UpdatePortRequest,
}

/// Inserts `cmd.node` (and any `cmd.cascaded` reference nodes) into `cmd.parent_group_id`, returning the
/// [`Command::RemoveNode`] that undoes it.
///
/// # Errors
///
/// Returns an error if `parent_group_id` (or a cascaded node's own parent) doesn't resolve to a group.
pub(super) fn apply_add_node(
    document: &mut OpmDocument,
    cmd: AddNode,
) -> Result<Command, BackEndErrorResponse> {
    let AddNode {
        parent_group_id,
        node,
        cascaded,
        connections,
    } = cmd;
    document
        .scenery_mut()
        .with_group_node_mut(parent_group_id, |g| g.add_node_ref(node.clone()))??;
    for (member_parent, member) in &cascaded {
        document
            .scenery_mut()
            .with_group_node_mut(*member_parent, |g| g.add_node_ref(member.clone()))??;
    }
    for conn in &connections {
        document
            .scenery_mut()
            .with_group_node_mut(parent_group_id, |g| connect_from_info(g, conn))??;
    }
    Ok(Command::RemoveNode(RemoveNode {
        parent_group_id,
        node,
        cascaded,
        connections,
    }))
}

/// Describes the effect of [`apply_add_node`] in the GUI-facing [`DocumentChange`] shape.
///
/// # Errors
///
/// Returns an error if building a [`NodeInfo`] for `node` or any cascaded node fails.
pub(super) fn describe_add_node(
    cmd: &AddNode,
) -> Result<Vec<DocumentChange>, BackEndErrorResponse> {
    let AddNode {
        parent_group_id,
        node,
        cascaded,
        connections,
    } = cmd;
    let mut changes = vec![DocumentChange::NodeAdded {
        graph_id: *parent_group_id,
        node: Box::new(node_info(node)?),
    }];
    for (member_parent, member) in cascaded {
        changes.push(DocumentChange::NodeAdded {
            graph_id: *member_parent,
            node: Box::new(node_info(member)?),
        });
    }
    for conn in connections {
        changes.push(DocumentChange::EdgeAdded {
            graph_id: *parent_group_id,
            connect_info: conn.clone(),
        });
    }
    Ok(changes)
}

/// Removes the node identified by `cmd.node`'s own uuid from the graph (cascading to reference nodes that
/// point at it, mirroring `NodeGroup::delete_node`'s existing behavior), returning the
/// [`Command::AddNode`] that undoes it.
///
/// # Errors
///
/// Returns an error if `node`'s uuid can't be resolved or the delete itself fails.
pub(super) fn apply_remove_node(
    document: &mut OpmDocument,
    cmd: RemoveNode,
) -> Result<Command, BackEndErrorResponse> {
    let RemoveNode {
        parent_group_id,
        node,
        cascaded,
        connections,
    } = cmd;
    let uuid = node.uuid()?;
    document.scenery_mut().delete_node(uuid)?;
    Ok(Command::AddNode(AddNode {
        parent_group_id,
        node,
        cascaded,
        connections,
    }))
}

/// Describes the effect of [`apply_remove_node`] in the GUI-facing [`DocumentChange`] shape.
///
/// # Errors
///
/// Returns an error if `node`'s or any cascaded node's uuid can't be resolved.
pub(super) fn describe_remove_node(
    cmd: &RemoveNode,
) -> Result<Vec<DocumentChange>, BackEndErrorResponse> {
    let RemoveNode {
        parent_group_id,
        node,
        cascaded,
        connections,
    } = cmd;
    let mut changes = vec![DocumentChange::NodeRemoved {
        graph_id: *parent_group_id,
        uuid: node.uuid()?,
    }];
    for (member_parent, member) in cascaded {
        changes.push(DocumentChange::NodeRemoved {
            graph_id: *member_parent,
            uuid: member.uuid()?,
        });
    }
    for conn in connections {
        changes.push(DocumentChange::EdgeRemoved {
            graph_id: *parent_group_id,
            connect_info: conn.clone(),
        });
    }
    Ok(changes)
}

/// Applies `cmd.new`'s populated fields to the node's standard properties, returning the
/// [`Command::PatchNode`] that undoes it (`old`/`new` swapped).
///
/// # Errors
///
/// Returns an error if `uuid` doesn't resolve to a node.
pub(super) fn apply_patch_node(
    document: &mut OpmDocument,
    cmd: PatchNode,
) -> Result<Command, BackEndErrorResponse> {
    let PatchNode {
        uuid,
        parent_group_id,
        old,
        new,
    } = cmd;
    document
        .scenery_mut()
        .with_node_attr_mut(uuid, |node_attr| apply_node_request(node_attr, &new))??;
    Ok(Command::PatchNode(PatchNode {
        uuid,
        parent_group_id,
        old: new,
        new: old,
    }))
}

/// Describes the effect of a [`Command::PatchNode`] in the GUI-facing [`DocumentChange`] shape.
pub(super) fn describe_patch_node(cmd: &PatchNode) -> Vec<DocumentChange> {
    let PatchNode {
        uuid,
        parent_group_id,
        new,
        ..
    } = cmd;
    vec![DocumentChange::NodePatched {
        graph_id: *parent_group_id,
        uuid: *uuid,
        name: new.name.clone(),
        inverted: new.inverted,
        gui_position: new.gui_position,
    }]
}

/// Sets a single custom property to `cmd.new`, returning the [`Command::PatchProperty`] that undoes it
/// (`old`/`new` swapped).
///
/// # Errors
///
/// Returns an error if `uuid` doesn't resolve to a node.
pub(super) fn apply_patch_property(
    document: &mut OpmDocument,
    cmd: PatchProperty,
) -> Result<Command, BackEndErrorResponse> {
    let PatchProperty {
        uuid,
        parent_group_id,
        prop_name,
        old,
        new,
    } = cmd;
    document
        .scenery_mut()
        .with_node_attr_mut(uuid, |node_attr| {
            node_attr.set_property(&prop_name, new.clone())
        })??;
    Ok(Command::PatchProperty(PatchProperty {
        uuid,
        parent_group_id,
        prop_name,
        old: new,
        new: old,
    }))
}

/// Applies `cmd.new`'s populated fields to one port's config, returning the [`Command::PatchPort`] that
/// undoes it (`old`/`new` swapped).
///
/// # Errors
///
/// Returns an error if `uuid` doesn't resolve to a node or `port_name` isn't a port of `port_type`.
pub(super) fn apply_patch_port(
    document: &mut OpmDocument,
    cmd: PatchPort,
) -> Result<Command, BackEndErrorResponse> {
    let PatchPort {
        uuid,
        parent_group_id,
        port_type,
        port_name,
        old,
        new,
    } = cmd;
    document
        .scenery_mut()
        .with_node_attr_mut(uuid, |node_attr| {
            let port_map = node_attr.raw_ports_mut().ports_mut(&port_type);
            let Some(port) = port_map.get_mut(&port_name) else {
                return Err(OpossumError::Other(format!(
                    "{port_type} port '{port_name}' not found"
                )));
            };
            if let Some(aperture) = new.aperture.clone() {
                port.aperture = aperture;
            }
            if let Some(coating) = new.coating {
                port.coating = coating;
            }
            if let Some(lidt) = new.lidt {
                port.lidt = lidt;
            }
            Ok(())
        })??;
    Ok(Command::PatchPort(PatchPort {
        uuid,
        parent_group_id,
        port_type,
        port_name,
        old: new,
        new: old,
    }))
}

/// Describes the effect of a [`Command::PatchProperty`] or [`Command::PatchPort`] in the GUI-facing
/// [`DocumentChange`] shape - both are reported the same way, as a details refresh for `uuid`.
pub(super) fn describe_node_details_changed(uuid: &Uuid) -> Vec<DocumentChange> {
    vec![DocumentChange::NodeDetailsChanged { uuid: *uuid }]
}

/// Builds the [`NodeInfo`] DTO for a captured node, mirroring how every other handler in this crate
/// turns an [`OpticRef`] into the response shape the GUI expects.
fn node_info(node: &OpticRef) -> Result<NodeInfo, BackEndErrorResponse> {
    let guard = node.optical_ref.lock_opm()?;
    Ok(NodeInfo::from_analyzable(&*guard, None))
}

/// Applies `new`'s populated fields to `node_attr`, mirroring `patch_node`'s existing field-by-field logic.
fn apply_node_request(
    node_attr: &mut NodeAttr,
    new: &UpdateNodeRequest,
) -> Result<(), OpossumError> {
    if let Some(name) = &new.name {
        node_attr.set_name(name);
    }
    if let Some(inverted) = new.inverted {
        node_attr.set_inverted(inverted);
    }
    if let Some(iso_opt) = new.isometry {
        node_attr.set_isometry_option(iso_opt);
    }
    if let Some(align) = new.alignment {
        node_attr.set_alignment(align);
    }
    if let Some(gui_pos_opt) = new.gui_position {
        node_attr.set_gui_position(gui_pos_opt.map(|(x, y)| Point2::new(x, y)));
    }
    Ok(())
}

/// Builds the `UpdateNodeRequest` describing `node_attr`'s current values for exactly the fields that
/// `new` is about to change - i.e. the request that would undo applying `new`.
#[must_use]
pub fn capture_old_node_request(
    node_attr: &NodeAttr,
    new: &UpdateNodeRequest,
) -> UpdateNodeRequest {
    UpdateNodeRequest {
        name: new.name.as_ref().map(|_| node_attr.name().to_string()),
        inverted: new.inverted.map(|_| node_attr.inverted()),
        isometry: new.isometry.map(|_| node_attr.isometry()),
        // `alignment` (unlike `isometry`/`gui_position`) isn't a double-`Option` in `UpdateNodeRequest`,
        // so there's no way to express "clear it back to unset" - if it was unset before, undo leaves
        // it unchanged rather than clearing it. Pre-existing limitation of the request shape, not new.
        alignment: new.alignment.and_then(|_| *node_attr.alignment()),
        gui_position: new
            .gui_position
            .map(|_| node_attr.gui_position().map(|p| (p.x, p.y))),
    }
}
