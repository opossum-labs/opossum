mod connections;
mod core;
mod port_mappings;
mod properties;

use crate::{app_state::AppState, error::BackEndErrorResponse};
use actix_web::{
    get,
    web::{self, Json, PathConfig},
};
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

#[utoipa::path(tag = "node",
    params(
        ("uuid" = Uuid, Path, description = "UUID of the node"),
    ),
    responses(
        (status = OK, description = "get the group hierarchy of a node", content(("application/json"))),
        (status = BAD_REQUEST, body = BackEndErrorResponse, description = "node with UUID not found", content_type="application/json")
    )
)]
#[get("/{uuid}/hierarchy")]
async fn get_node_hierarchy(
    data: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> Result<Json<Vec<(Uuid, String)>>, BackEndErrorResponse> {
    let node_id = path.into_inner();
    let document = data.document.lock();
    let scenery = document.scenery();
    let mut group_hierarchy = scenery.get_node_hierarchy_bottom_up(node_id)?;
    drop(document);
    group_hierarchy.reverse();

    Ok(Json(group_hierarchy))
}

// /// Modify node properties
// ///
// /// Modify the properties (`NodeAttr`) of a node specified by its UUID.
// /// - **Note**: This functino also searches the node recursively in the whole scenery.
// #[utoipa::path(tag = "node",
//     responses(
//         (status = OK, description = "node properties updated", content_type="application/json"),
//         (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
//     )
// )]
// #[patch("/{uuid}/properties")]
// #[allow(clippy::significant_drop_tightening)]
// async fn patch_properties(
//     data: web::Data<AppState>,
//     path: web::Path<Uuid>,
//     updated_props: Json<serde_json::Value>,
// ) -> Result<Json<NodeAttr>, BackEndErrorResponse> {
//     let uuid = path.into_inner();
//     let update_json = updated_props.into_inner();
//     let mut document = data.document.lock();
//     document
//         .scenery_mut()
//         .with_node_attr_mut(uuid, |node_attr| {
//             update_node_attr(node_attr, &update_json).map_or_else(
//                 |_| {
//                     Err(BackEndErrorResponse::new(
//                         404,
//                         "Opossum",
//                         "uuid not found in nodes",
//                     ))
//                 },
//                 |attr| {
//                     *node_attr = attr;
//                     Ok(web::Json(node_attr.clone()))
//                 },
//             )
//         })
//         .map_err(|_| BackEndErrorResponse::new(404, "Opossum", "uuid not found in nodes"))?
// }

pub fn config(cfg: &mut ServiceConfig<'_>) {
    // core CRUD services
    cfg.service(core::post_children);
    cfg.service(core::get_children);
    cfg.service(core::patch_node);
    cfg.service(core::delete_node);
    cfg.service(core::post_reference);

    // connection CRUD services
    cfg.service(connections::post_connection);
    cfg.service(connections::get_connections);
    cfg.service(connections::update_connection);
    cfg.service(connections::delete_connection);

    // port CRUD mapping services
    cfg.service(port_mappings::get_port_mappings);
    cfg.service(port_mappings::post_port_mapping);
    cfg.service(port_mappings::remove_port_map);

    // property CRUD services
    cfg.service(properties::post_node_property);
    cfg.service(properties::get_properties_ron);
    cfg.service(properties::get_properties_json);

    cfg.service(get_node_hierarchy);

    cfg.app_data(PathConfig::default().error_handler(|err, _req| {
        BackEndErrorResponse::new(400, "parse error", &err.to_string()).into()
    }));
}

// #[cfg(test)]
// mod test {
//     use crate::{app_state::AppState, error::BackEndErrorResponse};
//     use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
//     use uuid::Uuid;

//     #[actix_web::test]
//     async fn get_node() {
//         let app_state = Data::new(AppState::default());
//         let app = test::init_service(
//             App::new()
//                 .app_data(app_state)
//                 .service(super::get_properties_json),
//         )
//         .await;
//         let req = test::TestRequest::get()
//             .uri(&format!("/{}/properties", Uuid::new_v4()))
//             .to_request();
//         let resp = app.call(req).await.unwrap();
//         let e: BackEndErrorResponse = test::read_body_json(resp).await;
//         assert_eq!(e.error_response().status(), StatusCode::BAD_REQUEST);
//         assert_eq!(e.error_response().category(), "OpticScenery");
//     }
// }
