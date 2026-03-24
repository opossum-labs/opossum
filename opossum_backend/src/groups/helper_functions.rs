use std::collections::HashSet;

use actix_web::web;
use nalgebra::Point2;
use opossum_core::{OpticRef, error::OpmResult, meter, nodes::{ConnectionInfo, NodeGroup}, prelude::PortType, types::api_types::{ConnectInfo, NodeInfo}, utils::LockExt};
use uuid::Uuid;

use crate::{app_state::AppState, error::BackEndErrorResponse};


pub(super) fn collect_node_refs_and_pos(
    data: &web::Data<AppState>,
    nodes_to_convert: &[Uuid],
) -> (Vec<OpticRef>, Point2<f64>) {
    let document = data.document.lock();
    let scenery = document.scenery();
    let mut corner = Point2::new(f64::INFINITY, f64::INFINITY);
    let optic_ref_vec = nodes_to_convert
        .iter()
        .filter_map(|node| {
            scenery.node_recursive(*node).ok().map(|(r, _)| {
                if let Ok(opt_ref) = r.optical_ref.lock_opm() {
                    let pos = opt_ref.gui_position().unwrap();
                    corner.x = corner.x.min(pos.x);
                    corner.y = corner.y.min(pos.y);
                }
                r
            })
        })
        .collect();
    (optic_ref_vec, corner)
}

pub(super) fn collect_group_connections(
    data: &web::Data<AppState>,
    group_id: Uuid,
) -> OpmResult<Vec<ConnectionInfo>> {
    let document = data.document.lock();
    let scenery = document.scenery();

    scenery.with_group_node(group_id, |group| group.connections())
}

pub(super) fn build_reference_map(
    data: &web::Data<AppState>,
    connections: &[ConnectionInfo],
) -> std::collections::HashMap<Uuid, bool> {
    let document = data.document.lock();
    let scenery = document.scenery();

    connections
        .iter()
        .map(|c| {
            let is_ref = scenery
                .with_node_attr(c.target_id, |attr| {
                    attr.properties().get("reference id").is_ok()
                })
                .unwrap_or(false);
            (c.target_id, is_ref)
        })
        .collect()
}

pub(super) fn split_connections(
    connections: &[ConnectionInfo],
    reference_map: &std::collections::HashMap<Uuid, bool>,
    nodes_to_convert: &[Uuid],
) -> (Vec<ConnectInfo>, Vec<ConnectInfo>, Vec<ConnectInfo>) {
    let mut inside = Vec::new();
    let mut input = Vec::new();
    let mut output = Vec::new();

    for c in connections {
        let is_reference = *reference_map.get(&c.target_id).unwrap_or(&false);
        let c_info = ConnectInfo::from_connection_info(c, is_reference);

        let src_inside = nodes_to_convert.contains(&c_info.src_uuid());
        let tgt_inside = nodes_to_convert.contains(&c_info.target_uuid());

        match (src_inside, tgt_inside) {
            (true, true) => inside.push(c_info),
            (true, false) => output.push(c_info),
            (false, true) => input.push(c_info),
            _ => {}
        }
    }

    (inside, input, output)
}

pub(super) fn build_new_group(
    node_refs: Vec<OpticRef>,
    inside_connections: &[ConnectInfo],
    map_input_connections: &[ConnectInfo],
    map_output_connections: &[ConnectInfo],
) -> OpmResult<NodeGroup> {
    let mut new_group = NodeGroup::new("new group");

    for node_ref in node_refs {
        new_group.add_node_ref(node_ref)?;
    }

    for conn in inside_connections {
        new_group.connect_nodes(
            conn.src_uuid(),
            conn.src_port(),
            conn.target_uuid(),
            conn.target_port(),
            meter!(conn.distance()),
        )?;
    }

    for map_out in map_output_connections {
        new_group.map_output_port(map_out.src_uuid(), map_out.src_port(), map_out.src_port())?;
    }

    for map_in in map_input_connections {
        new_group.map_input_port(
            map_in.target_uuid(),
            map_in.target_port(),
            map_in.target_port(),
        )?;
    }

    Ok(new_group)
}

pub(super) fn add_converted_group_to_scenery(
    data: &web::Data<AppState>,
    group_id: Uuid,
    mut nodes_to_convert: Vec<Uuid>,
    new_group: NodeGroup,
    map_input_connections: &[ConnectInfo],
    map_output_connections: &[ConnectInfo],
) -> Result<Uuid, BackEndErrorResponse> {
    let mut document = data.document.lock();
    let scenery = document.scenery_mut();

    while let Some(node) = nodes_to_convert.pop() {
        let deleted = scenery.delete_node(node)?;
        for del_id in &deleted {
            nodes_to_convert.retain(|id| id != del_id);
        }
    }
    
    scenery.with_group_node_mut(group_id, |g| {
        match g.add_node(new_group) {
            Ok(new_group_id) => {
                //connect the output ports and connect within scenery
                for map_out in map_output_connections {
                    g.connect_nodes(
                        map_out.src_uuid(),
                        map_out.src_port(),
                        map_out.target_uuid(),
                        map_out.target_port(),
                        meter!(map_out.distance()),
                    )?;
                }
                //connect the input ports
                for map_in in map_input_connections {
                    g.connect_nodes(
                        map_in.src_uuid(),
                        map_in.src_port(),
                        map_in.target_uuid(),
                        map_in.target_port(),
                        meter!(map_in.distance()),
                    )?;
                }
                Ok(new_group_id)
            }
            Err(e) => Err(BackEndErrorResponse::new(
                404,
                "Opossum",
                &format!("Could not add group node{e}"),
            )),
        }
    })?
}

pub(super) fn create_new_group_node_info(
    data: &web::Data<AppState>,
    new_group_id: Uuid,
    pos: Point2<f64>,
) -> OpmResult<NodeInfo> {
    let document = data.document.lock();
    let scenery = document.scenery();

    let (new_group_ref, _) = scenery.node_recursive(new_group_id)?;
    let new_group_node = new_group_ref.optical_ref.lock_opm()?;

    Ok(NodeInfo::new(
        new_group_id,
        new_group_node.name(),
        new_group_node.inverted(),
        new_group_node.node_type(),
        new_group_node.ports().names(&PortType::Input),
        new_group_node.ports().names(&PortType::Output),
        Some((pos.x, pos.y)),
    ))
}