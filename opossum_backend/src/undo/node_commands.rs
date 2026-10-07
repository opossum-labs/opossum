//! `apply`/`describe` bodies for node-field-patch [`Command`] variants:
//! [`Command::PatchNode`], [`Command::PatchProperty`], [`Command::PatchPort`].

use nalgebra::Point2;
use opossum_core::{
    core_optics::{NodeAttr, node_attr::NodePositioning},
    error::OpossumError,
    opm_document::OpmDocument,
    prelude::{PortType, Proptype},
    types::api_types::{
        DocumentChange, NodeEditorPanel, PositioningRequest, UpdateNodeRequest, UpdatePortRequest,
    },
};
use uuid::Uuid;

use super::Command;
use crate::error::BackEndErrorResponse;

/// Applies `new`'s populated fields to the node's standard properties; `old` mirrors the same
/// `Option`-shape with the values that were in place beforehand.
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

/// Applies `cmd.new`'s populated fields to the node's standard properties, returning the
/// [`Command::PatchNode`] that undoes it (`old`/`new` swapped).
///
/// # Errors
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
        .with_node_attr_mut(uuid, |node_attr| apply_node_request(node_attr, &new))?;
    Ok(Command::PatchNode(Box::new(PatchNode {
        uuid,
        parent_group_id,
        old: new,
        new: old,
    })))
}

/// Determines which node-editor sidebar panel (if any) `new`'s populated field(s) belong to.
pub(super) const fn panel_for_update(new: &UpdateNodeRequest) -> Option<NodeEditorPanel> {
    if new.alignment.is_some() {
        Some(NodeEditorPanel::Alignment)
    } else if new.positioning.is_some() {
        Some(NodeEditorPanel::Positioning)
    } else if new.name.is_some() || new.inverted.is_some() {
        Some(NodeEditorPanel::General)
    } else {
        None
    }
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
/// [`DocumentChange`] shape.
pub(super) fn describe_node_details_changed(graph_id: Uuid, uuid: Uuid) -> Vec<DocumentChange> {
    vec![DocumentChange::NodeDetailsChanged { uuid, graph_id }]
}

/// Applies `new`'s populated fields to `node_attr`.
fn apply_node_request(node_attr: &mut NodeAttr, new: &UpdateNodeRequest) {
    if let Some(name) = &new.name {
        node_attr.set_name(name);
    }
    if let Some(inverted) = new.inverted {
        node_attr.set_inverted(inverted);
    }
    if let Some(positioning_req) = &new.positioning {
        let domain_positioning = match positioning_req {
            PositioningRequest::Absolute(iso) => NodePositioning::Absolute(*iso),
            PositioningRequest::Automatic => NodePositioning::Automatic(None),
        };
        node_attr.set_positioning(domain_positioning);
    }
    if let Some(align_opt) = new.alignment {
        node_attr.set_alignment_option(align_opt);
    }
    if let Some(gui_pos_opt) = new.gui_position {
        node_attr.set_gui_position(gui_pos_opt.map(|(x, y)| Point2::new(x, y)));
    }
}

/// Builds the `UpdateNodeRequest` describing `node_attr`'s current values for fields that
/// `new` is about to change.
#[must_use]
pub fn capture_old_node_request(
    node_attr: &NodeAttr,
    new: &UpdateNodeRequest,
) -> UpdateNodeRequest {
    UpdateNodeRequest {
        name: new.name.as_ref().map(|_| node_attr.name().to_string()),
        inverted: new.inverted.map(|_| node_attr.inverted()),
        positioning: new
            .positioning
            .as_ref()
            .map(|_| match node_attr.positioning() {
                NodePositioning::Absolute(iso) => PositioningRequest::Absolute(*iso),
                NodePositioning::Automatic(_) => PositioningRequest::Automatic,
            }),
        alignment: new.alignment.map(|_| *node_attr.alignment()),
        gui_position: new
            .gui_position
            .map(|_| node_attr.gui_position().map(|p| (p.x, p.y))),
    }
}
