pub mod helper_functions;

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
