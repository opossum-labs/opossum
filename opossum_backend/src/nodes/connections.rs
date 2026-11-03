use actix_web::{
    HttpResponse, delete, get, patch, post,
    web::{self, Json},
};
use opossum_core::{
    meter,
    nodes::{ConnectionInfo, NodeGroup},
    types::api_types::{ConnectInfo, ErrorResponse, UpdateConnectionRequest},
};
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::{
    app_state::AppState, error::BackEndErrorResponse, helper_functions::is_reference_target,
    undo::Command,
};

#[derive(Debug, Deserialize, IntoParams)]
pub struct DeleteConnectionQuery {
    /// UUID of the source node
    pub src_uuid: Uuid,
    /// Name of the source port
    pub src_port: String,
}

#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"),
    ),
    responses(
        (status = OK, description = "all connections of the group", body = Vec<ConnectInfo>, content_type = "application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found or not a group node", content_type = "application/json")
    )
)]
#[allow(clippy::significant_drop_tightening)]
#[get("/{uuid}/connections")]
pub async fn get_connections(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<ConnectInfo>>, BackEndErrorResponse> {
    let document = data.document.lock();
    let scenery = document.scenery();
    let uuid = path.into_inner();
    let connections = scenery.with_group_node(uuid, NodeGroup::connections)?;
    Ok(Json(connect_infos(scenery, &connections)))
}

/// List the connections whose distance places a node
///
/// Returns the connections of a group whose distance places a node when the model is placed: the
/// node a connection leads to is placed along it, or the node it comes from stands in front of
/// where it leads. A distance into a node placed absolutely, or into a reference taking the
/// placement of the node it refers to, places nothing, so there is no point in editing it.
///
/// The model is placed on a copy, as for the 3D view, after the analyzer that view follows by
/// default. A placement that stops part way lists the distances that placed a node before it
/// stopped. A model without a source port or an analyzer places nothing: the list is empty.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"),
    ),
    responses(
        (status = OK, description = "the connections of the group whose distance places a node", body = Vec<ConnectInfo>, content_type = "application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "UUID not found or not a group node", content_type = "application/json")
    )
)]
#[get("/{uuid}/placing_connections")]
pub async fn get_placing_connections(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<ConnectInfo>>, BackEndErrorResponse> {
    let uuid = path.into_inner();
    let document = data.document.lock();
    document.scenery().with_group_node(uuid, |_| ())?;
    let placing = document.placing_connections(None, uuid).unwrap_or_default();
    let listed = connect_infos(document.scenery(), &placing);
    drop(document);
    Ok(Json(listed))
}

/// Returns `connections` of the model `scenery` as the GUI shows them, each marked when it leads
/// into a reference.
fn connect_infos(scenery: &NodeGroup, connections: &[ConnectionInfo]) -> Vec<ConnectInfo> {
    connections
        .iter()
        .map(|c| ConnectInfo::from_connection_info(c, is_reference_target(scenery, c.target_id)))
        .collect()
}

/// Connect two nodes
///
/// Connect two optical nodes by the given connection info.
#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"),
    ),
    request_body = ConnectInfo,
    responses(
        (status = CREATED, description = "node connection created", body = ConnectInfo, content_type = "application/json"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "group UUID not found", content_type = "application/json")
    )
)]
#[post("/{uuid}/connections")]
pub async fn post_connection(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    connect_info: Json<ConnectInfo>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let group_uuid = path.into_inner();
    let mut document = data.document.lock();

    // The core's connect_nodes returns GraphDelta::NodesConnected directly
    let delta = document
        .scenery_mut()
        .with_group_node_mut(group_uuid, |group| {
            group.connect_nodes(
                connect_info.src_uuid(),
                connect_info.src_port(),
                connect_info.target_uuid(),
                connect_info.target_port(),
                meter!(connect_info.distance()),
            )
        })??;

    let is_ref_node = is_reference_target(document.scenery(), connect_info.target_uuid());
    let mut connect_info = connect_info.into_inner();
    connect_info.set_is_reference(is_ref_node);

    data.push_undo(Command::UndoGraph(Box::new(delta)));
    drop(document);

    Ok(HttpResponse::Created().json(connect_info))
}

/// Update a connection distance
///
/// Updates only the geometric distance of an existing connection identified by its source node and port.
#[utoipa::path(
    tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group node"),
    ),
    request_body = UpdateConnectionRequest,
    responses(
        (status = NO_CONTENT, description = "Node connection successfully updated"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "Group UUID not found or connection invalid", content_type = "application/json")
    )
)]
#[patch("/{uuid}/connections")]
pub async fn update_connection(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    req_body: Json<UpdateConnectionRequest>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let group_uuid = path.into_inner();
    let update_req = req_body.into_inner();
    let mut document = data.document.lock();

    // Core captures old and new distance inside GraphDelta::ConnectionDistanceChanged
    let delta = document
        .scenery_mut()
        .with_group_node_mut(group_uuid, |group| {
            group.update_connection_distance(
                update_req.src_uuid,
                &update_req.src_port,
                meter!(update_req.distance),
            )
        })??;

    data.push_undo(Command::UndoGraph(Box::new(delta)));
    drop(document);

    Ok(HttpResponse::NoContent().finish())
}

/// Disconnect two nodes
///
/// Removes the connection originating from the specified source node and port.
#[utoipa::path(
    tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the group containing the connection"),
        DeleteConnectionQuery
    ),
    responses(
        (status = NO_CONTENT, description = "node connection successfully deleted"),
        (status = BAD_REQUEST, body = ErrorResponse, description = "group UUID not found or disconnection failed", content_type = "application/json")
    )
)]
#[delete("/{uuid}/connections")]
pub async fn delete_connection(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
    query: web::Query<DeleteConnectionQuery>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let group_uuid = path.into_inner();
    let query = query.into_inner();
    let mut document = data.document.lock();

    // Core captures connection details inside GraphDelta::NodesDisconnected
    let delta = document
        .scenery_mut()
        .with_group_node_mut(group_uuid, |group| {
            group.disconnect_nodes(query.src_uuid, &query.src_port)
        })??;

    data.push_undo(Command::UndoGraph(Box::new(delta)));
    drop(document);

    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod test {
    use super::*;
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};

    fn create_test_state() -> Data<AppState> {
        Data::new(AppState::default())
    }

    #[actix_web::test]
    async fn test_get_connections_invalid_uuid() {
        let app_state = create_test_state();
        let app = test::init_service(App::new().app_data(app_state).service(get_connections)).await;
        let req = test::TestRequest::get()
            .uri(&format!("/{}/connections", Uuid::new_v4()))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    /// A state whose model feeds a lens from a source, and a reference to the lens from a source
    /// with an automatic position 50 mm in front of it, traced by a ray trace analyzer.
    ///
    /// # Returns
    ///
    /// The state, the uuid of the model's root group and the pairs of source and target of the
    /// connections whose distance places a node: both.
    fn state_with_a_source_in_front_of_a_reference() -> (Data<AppState>, Uuid, Vec<(Uuid, Uuid)>) {
        use opossum_core::{
            analyzers::{AnalyzerType, RayTraceConfig},
            core_optics::{
                OpticNode,
                node_attr::{HasNodeAttr, NodePositioning},
            },
            light::lightdata::ray_data_builder::RayDataBuilder,
            millimeter,
            nodes::{Lens, NodeGroup, NodeReference, SourcePort},
            opm_document::OpmDocument,
        };

        let mut scenery = NodeGroup::default();
        let first = scenery.add_node(SourcePort::default()).unwrap();
        let lens = scenery.add_node(Lens::default()).unwrap();
        let reference = NodeReference::from_node(&scenery.node(lens).unwrap()).unwrap();
        let reference = scenery.add_node(reference).unwrap();
        let mut second = SourcePort::default();
        second
            .set_positioning(NodePositioning::Automatic(None))
            .unwrap();
        let second = scenery.add_node(second).unwrap();
        scenery
            .connect_nodes(first, "output_1", lens, "input_1", millimeter!(100.0))
            .unwrap();
        scenery
            .connect_nodes(second, "output_1", reference, "input_1", millimeter!(50.0))
            .unwrap();
        let root = scenery.node_attr().uuid();
        let mut document = OpmDocument::new(scenery);
        let mut config = RayTraceConfig::default();
        for source in [first, second] {
            config.map_source(source, RayDataBuilder::default());
        }
        document.add_analyzer(AnalyzerType::RayTrace(config));
        let app_state = create_test_state();
        *app_state.document.lock() = document;
        (app_state, root, vec![(first, lens), (second, reference)])
    }

    /// Ask `app_state` for the connections of the group `uuid` whose distance places a node.
    async fn request_placing_connections(
        app_state: Data<AppState>,
        uuid: Uuid,
    ) -> actix_web::dev::ServiceResponse {
        let app = test::init_service(
            App::new()
                .app_data(app_state)
                .service(get_placing_connections),
        )
        .await;
        let req = test::TestRequest::get()
            .uri(&format!("/{uuid}/placing_connections"))
            .to_request();
        app.call(req).await.unwrap()
    }

    /// Every connection of a group is listed with its distance in meters, marked when it leads
    /// into a reference.
    #[actix_web::test]
    async fn the_connections_of_a_group_are_listed() {
        let (app_state, root, _) = state_with_a_source_in_front_of_a_reference();
        let app = test::init_service(App::new().app_data(app_state).service(get_connections)).await;
        let req = test::TestRequest::get()
            .uri(&format!("/{root}/connections"))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let listed: Vec<ConnectInfo> = test::read_body_json(resp).await;
        let mut seen: Vec<(bool, f64)> = listed
            .iter()
            .map(|c| (c.targets_reference(), c.distance()))
            .collect();
        seen.sort_by_key(|(into_reference, _)| *into_reference);
        assert_eq!(seen.len(), 2);
        assert!(!seen[0].0 && (seen[0].1 - 0.1).abs() < 1e-12, "{seen:?}");
        assert!(seen[1].0 && (seen[1].1 - 0.05).abs() < 1e-12, "{seen:?}");
    }

    /// The connections whose distance places a node are listed with the distance they have, and
    /// marked when they lead into a reference.
    #[actix_web::test]
    async fn the_connections_whose_distance_places_a_node_are_listed() {
        let (app_state, root, expected) = state_with_a_source_in_front_of_a_reference();
        let resp = request_placing_connections(app_state, root).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let listed: Vec<ConnectInfo> = test::read_body_json(resp).await;
        let mut pairs: Vec<(Uuid, Uuid)> = listed
            .iter()
            .map(|c| (c.src_uuid(), c.target_uuid()))
            .collect();
        pairs.sort();
        let mut expected = expected;
        expected.sort();
        assert_eq!(pairs, expected);
        let into_reference = listed
            .iter()
            .find(|c| c.targets_reference())
            .expect("the connection into the reference is listed");
        assert!((into_reference.distance() - 0.05).abs() < 1e-12);
    }

    /// A model that cannot be placed, here for lack of an analyzer, places nothing by any
    /// distance: the list is empty. A group the model does not have is an error.
    #[actix_web::test]
    async fn no_connection_is_listed_when_the_model_cannot_be_placed() {
        let (app_state, root, _) = state_with_a_source_in_front_of_a_reference();
        app_state.document.lock().analyzers_mut().clear();
        let resp = request_placing_connections(app_state.clone(), root).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let listed: Vec<ConnectInfo> = test::read_body_json(resp).await;
        assert_eq!(listed, Vec::<ConnectInfo>::new());

        let resp = request_placing_connections(app_state, Uuid::new_v4()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_delete_connection_invalid_uuid() {
        let app_state = create_test_state();
        let app =
            test::init_service(App::new().app_data(app_state).service(delete_connection)).await;
        let req = test::TestRequest::delete()
            .uri(&format!(
                "/{}/connections?src_uuid={}&src_port=out",
                Uuid::new_v4(),
                Uuid::new_v4()
            ))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
