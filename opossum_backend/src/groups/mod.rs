use actix_web::{
    delete, get,
    web::{self, Json, PathConfig},
};
use opossum_core::{
    prelude::{PortMap, PortType},
    types::api_types::ConnectInfo,
};
use utoipa_actix_web::service_config::ServiceConfig;
use uuid::Uuid;

use crate::{app_state::AppState, error::BackEndErrorResponse};

pub mod helper_functions;

use serde::Deserialize;
use utoipa::IntoParams;

#[derive(Debug, Deserialize, IntoParams)]
pub struct RemovePortMapQuery {
    /// External port name of the group port mapping
    pub external_port_name: String,
    /// Type of the port (e.g., Input or Output)
    pub port_type: PortType,
}

// #[utoipa::path(tag = "group",
//     params(
//         ("uuid" = Uuid, Path, description = "Uuid of a group whose ports should be sent"),
//     ),
//     responses(
//         (status = OK, description = "Node ports successfully sent!"),
//         (status = BAD_REQUEST, body = BackEndErrorResponse, description = "UUID not found", content_type="application/json")
//     )
// )]
// #[get("/{uuid}/ports")]
// pub async fn get_group_ports(
//     data: web::Data<AppState>,
//     path: web::Path<Uuid>,
// ) -> Result<Json<(Vec<String>, Vec<String>)>, BackEndErrorResponse> {
//     let group_id = path.into_inner();

//     let ports = data
//         .document
//         .lock()
//         .scenery_mut()
//         .with_group_node_mut(group_id, |g| {
//             let ports = g.ports();
//             let inputs = ports
//                 .ports(&PortType::Input)
//                 .keys()
//                 .cloned()
//                 .collect::<Vec<String>>();
//             let outputs = ports
//                 .ports(&PortType::Output)
//                 .keys()
//                 .cloned()
//                 .collect::<Vec<String>>();
//             Ok::<(Vec<String>, Vec<String>), BackEndErrorResponse>((inputs, outputs))
//         })??;
//     Ok(Json(ports))
// }

pub fn config(cfg: &mut ServiceConfig<'_>) {
    // cfg.service(get_group_ports);
    cfg.app_data(PathConfig::default().error_handler(|err, _req| {
        BackEndErrorResponse::new(400, "parse error", &err.to_string()).into()
    }));
}
