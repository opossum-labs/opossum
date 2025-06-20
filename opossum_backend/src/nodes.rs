use crate::{app_state::AppState, error::ErrorResponse, utils::update_node_attr};
use actix_web::{
    delete, get, patch, post, put,
    web::{self, Json, PathConfig},
};
use nalgebra::Point2;
use opossum::{
    meter,
    nodes::{NodeAttr, create_node_ref},
    optic_ports::PortType,
};
use serde::{Deserialize, Serialize};
use uom::si::length::meter;
use utoipa::ToSchema;
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct NodeInfo {
    uuid: Uuid,
    name: String,
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
        node_type: String,
        input_ports: Vec<String>,
        output_ports: Vec<String>,
        gui_position: Option<(f64, f64)>,
    ) -> Self {
        Self {
            uuid,
            name,
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
    #[allow(clippy::missing_const_for_fn)]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
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
/// Get all nodes of a group node
///
/// Return a list of all nodes of a group node specified by its UUID.
/// - **Note**: If the `nil` UUID is given (00000000-0000-0000-0000-000000000000), all toplevel nodes are returned.
/// - **Note**: This function searches recursively for the UUID in the whole scenery.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"),
    ),
    responses(
        (status = OK, description = "get all nodes of the group", content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found or not a group node", content_type="application/json")
    )
)]
#[get("/{uuid}/nodes")]
async fn get_subnodes(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<NodeInfo>>, ErrorResponse> {
    let document = data.document.lock().unwrap();
    let scenery = document.scenery().clone();
    drop(document);
    let uuid = path.into_inner();
    let nodes_info: Vec<NodeInfo> = if uuid.is_nil() {
        scenery
            .nodes()
            .iter()
            .map(|n| {
                let node = n.optical_ref.lock().unwrap();
                let name = node.name();
                let node_type = node.node_type();
                let input_ports = node.ports().names(&PortType::Input);
                let output_ports = node.ports().names(&PortType::Output);
                let gui_position = node.gui_position().map(|position| (position.x, position.y));
                drop(node);
                NodeInfo {
                    uuid: n.uuid(),
                    name,
                    node_type,
                    input_ports,
                    output_ports,
                    gui_position,
                }
            })
            .collect()
    } else {
        scenery
            .node_recursive(uuid)?
            .optical_ref
            .lock()
            .unwrap()
            .as_group_mut()?
            .nodes()
            .iter()
            .map(|n| {
                let node = n.optical_ref.lock().unwrap();
                let name = node.name();
                let node_type = node.node_type();
                let input_ports = node.ports().names(&PortType::Input);
                let output_ports = node.ports().names(&PortType::Output);
                let gui_position = node.gui_position().map(|position| (position.x, position.y));
                drop(node);
                NodeInfo {
                    uuid: n.uuid(),
                    name,
                    node_type,
                    input_ports,
                    output_ports,
                    gui_position,
                }
            })
            .collect()
    };
    Ok(Json(nodes_info))
}
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"),
    ),
    responses(
        (status = OK, description = "all connections of the group", body= Vec<ConnectInfo>, content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found or not a group node", content_type="application/json")
    )
)]
#[get("/{uuid}/connections")]
pub async fn get_connections(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<ConnectInfo>>, ErrorResponse> {
    let document = data.document.lock().unwrap();
    let scenery = document.scenery().clone();
    drop(document);
    let uuid = path.into_inner();
    let connect_infos: Vec<ConnectInfo> = if uuid.is_nil() {
        // toplevel group
        scenery
            .connections()
            .iter()
            .map(|c| ConnectInfo::new(c.0, c.1.clone(), c.2, c.3.clone(), c.4.get::<meter>()))
            .collect::<Vec<ConnectInfo>>()
    } else {
        // subgroup
        scenery
            .node_recursive(uuid)?
            .optical_ref
            .lock()
            .unwrap()
            .as_group_mut()?
            .connections()
            .iter()
            .map(|c| ConnectInfo::new(c.0, c.1.clone(), c.2, c.3.clone(), c.4.get::<meter>()))
            .collect::<Vec<ConnectInfo>>()
    };
    Ok(Json(connect_infos))
}
#[derive(Clone, Serialize, Deserialize, ToSchema)]
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
}

/// Add a new node to a group node
///
/// This function adds a new optical node to a group node specified by its UUID.
/// - **Note**: If the `nil` UUID is given (00000000-0000-0000-0000-000000000000), the node is added to the toplevel group.
/// - The node type as well as the coordinates of the corresponding GUI element must be given.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the optical node"),
    ),
    request_body(content = NewNode,
        description = "type and GUI position of node the optical node to be created",
        content_type = "application/json",
        example ="{\"node_type\": \"dummy\", \"gui_position\": [0.0,0.0]}"
    ),
    responses(
        (status = OK, body= NodeInfo, description = "Node successfully created", content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "Node of the given type not found, UUID not found, no group node", content_type="application/json")
    )
)]
#[post("/{uuid}/nodes")]
async fn post_subnode(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    node_type: web::Json<NewNode>,
) -> Result<Json<NodeInfo>, ErrorResponse> {
    let new_node_info = node_type.into_inner();
    let new_node_ref = create_node_ref(&new_node_info.node_type)?;
    let mut node = new_node_ref.optical_ref.lock().unwrap();
    let node_attr = node.node_attr_mut();
    node_attr.set_gui_position(Some(Point2::new(
        new_node_info.gui_position.0,
        new_node_info.gui_position.1,
    )));
    drop(node);
    let mut document = data.document.lock().unwrap();
    let uuid = path.into_inner();
    let scenery = document.scenery_mut();
    let new_node_uuid = if uuid.is_nil() {
        scenery.add_node_ref(new_node_ref.clone())?
    } else {
        scenery
            .node_recursive(uuid)?
            .optical_ref
            .lock()
            .unwrap()
            .as_group_mut()?
            .add_node_ref(new_node_ref.clone())?
    };
    drop(document);
    let node = new_node_ref.optical_ref.lock().unwrap();
    let gui_position = node.gui_position().map(|position| (position.x, position.y));
    let node_info = NodeInfo {
        uuid: new_node_uuid,
        name: node.name(),
        node_type: node.node_type(),
        input_ports: node.ports().names(&PortType::Input),
        output_ports: node.ports().names(&PortType::Output),
        gui_position,
    };
    drop(node);
    Ok(Json(node_info))
}
/// Update the GUI position of an optical or analyzer node
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the optical or analyzer node"),
    ),
    request_body(content = (f64,f64),
        description = "updated GUI position",
        content_type = "application/json",
        example= "[1.0, 2.0]"
    ),
    responses(
        (status = OK, description = "Node position successfully updated"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[post("/position/{uuid}")]
async fn post_node_position(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    position: web::Json<(f64, f64)>,
) -> Result<(), ErrorResponse> {
    let uuid = path.into_inner();
    let position = position.into_inner();
    let position = Point2::new(position.0, position.1);
    let mut document = data.document.lock().unwrap();
    match document.scenery().node_recursive(uuid) {
        Ok(node_ref) => {
            node_ref
                .optical_ref
                .lock()
                .unwrap()
                .node_attr_mut()
                .set_gui_position(Some(position));
            Ok(())
        }
        _ => document.analyzers_mut().get_mut(&uuid).map_or_else(
            || {
                Err(ErrorResponse::new(
                    404,
                    "Opossum",
                    "uuid not found in nodes or analyzers",
                ))
            },
            |analyzer| {
                analyzer.set_gui_position(Some(position));
                Ok(())
            },
        ),
    }
}
/// Delete a node
///
/// This function deletes a node. It also deletes reference nodes which refer to this node.
/// A list of UUIDs of the effectively deleted nodes is returned.
#[utoipa::path(tag = "node",
responses(
    (status = OK, body= Vec<Uuid>, description = "UUIDs of the deleted nodes", content_type="application/json"),
    (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
))]
#[delete("/{uuid}/nodes")]
async fn delete_subnode(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<Uuid>>, ErrorResponse> {
    let uuid = path.into_inner();
    let mut document = data.document.lock().unwrap();
    let scenery = document.scenery_mut();
    let deleted_nodes = scenery.delete_node(uuid)?;
    drop(document);
    Ok(web::Json(deleted_nodes))
}
/// Get all properties of the specified node
///
/// Return all properties (`NodeAttr`) of the node specified by its UUID.
/// - **Note**: This function only returns `NodeAttr`, even for group nodes.
///   A possible `graph` structure is omitted.
/// - **Note**: This function searches the node recursively in the whole scenery.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the optical node"),
    ),
    responses(
        (status = OK, description = "get all node properties", content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/properties")]
async fn get_properties(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<NodeAttr>, ErrorResponse> {
    let uuid = path.into_inner();
    let document = data.document.lock().unwrap();
    let node_attr = document
        .scenery()
        .node_recursive(uuid)?
        .optical_ref
        .lock()
        .unwrap()
        .node_attr()
        .clone();
    drop(document);
    Ok(web::Json(node_attr))
}
/// Modify node properties
///
/// Modify the properties (`NodeAttr`) of a node specified by its UUID.
/// - **Note**: This functino also searches the node recursively in the whole scenery.
#[utoipa::path(tag = "node",
    responses(
        (status = OK, description = "node properties updated", content_type="application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found", content_type="application/json")
    )
)]
#[patch("/{uuid}/properties")]
#[allow(clippy::significant_drop_tightening)]
async fn patch_properties(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    updated_props: Json<serde_json::Value>,
) -> Result<Json<NodeAttr>, ErrorResponse> {
    let uuid = path.into_inner();
    let document = data.document.lock().unwrap();
    let node = document.scenery().node_recursive(uuid)?;
    drop(document);
    let mut optic_ref = node.optical_ref.lock().unwrap();
    let node_attr = optic_ref.node_attr_mut();
    let update_json = updated_props.into_inner();
    *node_attr = update_node_attr(node_attr, &update_json)?;
    Ok(web::Json(node_attr.clone()))
}
/// Connection Information
#[derive(ToSchema, Clone, PartialEq, Serialize, Deserialize)]
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
/// Connect two nodes
///
/// Connect to optical nodes by the given connection info.
#[utoipa::path(tag = "node")]
#[post("/connection")]
async fn post_connection(
    data: web::Data<AppState>,
    connect_info: Json<ConnectInfo>,
) -> Result<Json<ConnectInfo>, ErrorResponse> {
    let mut document = data.document.lock().unwrap();
    let scenery = document.scenery_mut();
    scenery.connect_nodes(
        connect_info.src_uuid,
        &connect_info.src_port,
        connect_info.target_uuid,
        &connect_info.target_port,
        meter!(connect_info.distance),
    )?;
    drop(document);
    Ok(connect_info)
}
/// Disconnect two nodes
#[utoipa::path(tag = "node")]
#[delete("/connection")]
async fn delete_connection(
    data: web::Data<AppState>,
    connect_info: Json<ConnectInfo>,
) -> Result<Json<ConnectInfo>, ErrorResponse> {
    let mut document = data.document.lock().unwrap();
    let scenery = document.scenery_mut();
    scenery.disconnect_nodes(connect_info.src_uuid, &connect_info.src_port)?;
    drop(document);
    Ok(connect_info)
}
/// Update a connection distance
#[utoipa::path(tag = "node")]
#[put("/connection")]
async fn update_distance(
    data: web::Data<AppState>,
    connect_info: Json<ConnectInfo>,
) -> Result<Json<ConnectInfo>, ErrorResponse> {
    let mut document = data.document.lock().unwrap();
    let scenery = document.scenery_mut();
    scenery.update_connection_distance(
        connect_info.src_uuid,
        &connect_info.src_port,
        meter!(connect_info.distance),
    )?;
    drop(document);
    Ok(connect_info)
}
pub fn config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(get_subnodes);
    cfg.service(post_subnode);
    cfg.service(delete_subnode);
    cfg.service(post_node_position);

    cfg.service(get_properties);
    cfg.service(patch_properties);

    cfg.service(post_connection);
    cfg.service(delete_connection);
    cfg.service(get_connections);
    cfg.service(update_distance);

    cfg.app_data(PathConfig::default().error_handler(|err, _req| {
        ErrorResponse::new(400, "parse error", &err.to_string()).into()
    }));
}

#[cfg(test)]
mod test {
    use crate::{app_state::AppState, error::ErrorResponse};
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use uuid::Uuid;

    #[actix_web::test]
    async fn get_node() {
        let app_state = Data::new(AppState::default());
        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(super::get_properties),
        )
        .await;
        let req = test::TestRequest::get()
            .uri(&format!("/{}/properties", Uuid::new_v4()))
            .to_request();
        let resp = app.call(req).await.unwrap();
        let e: ErrorResponse = test::read_body_json(resp).await;
        assert_eq!(e.status(), StatusCode::BAD_REQUEST);
        assert_eq!(e.category(), "OpticScenery");
    }
}
