mod connections;
mod core;
mod port_mappings;
mod ports;
mod properties;

use crate::error::BackEndErrorResponse;
use actix_web::web::PathConfig;
use utoipa_actix_web::service_config::ServiceConfig;

pub fn config(cfg: &mut ServiceConfig<'_>) {
    // core CRUD services
    cfg.service(core::post_children);
    cfg.service(core::get_children);
    cfg.service(core::get_node);
    cfg.service(core::patch_node);
    cfg.service(core::delete_node);
    cfg.service(core::post_reference);
    cfg.service(core::get_node_hierarchy);

    // connection CRUD services
    cfg.service(connections::post_connection);
    cfg.service(connections::get_connections);
    cfg.service(connections::update_connection);
    cfg.service(connections::delete_connection);

    // port mammping CRUD mapping services
    cfg.service(port_mappings::get_port_mappings);
    cfg.service(port_mappings::post_port_mapping);
    cfg.service(port_mappings::remove_port_map);

    // property CRUD services
    cfg.service(properties::get_properties);
    cfg.service(properties::patch_property);

    // port CRUD services
    cfg.service(ports::get_ports);
    cfg.service(ports::patch_port);

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
