mod amplifiers;
mod connections;
mod core;
pub mod port_mappings;
mod ports;
mod properties;

use crate::error::BackEndErrorResponse;
use actix_web::web::PathConfig;
use utoipa_actix_web::service_config::ServiceConfig;

pub fn config(cfg: &mut ServiceConfig<'_>) {
    // Document-wide queries. These must be registered before the `/{uuid}` routes below, otherwise
    // their literal path segment would be swallowed as a (then unparseable) node UUID.
    cfg.service(amplifiers::get_amplifiers);

    // core CRUD services
    cfg.service(core::post_children);
    cfg.service(core::get_children);
    cfg.service(core::get_node);
    cfg.service(core::patch_node);
    cfg.service(core::delete_node);
    cfg.service(core::delete_nodes);
    cfg.service(core::post_reference);
    cfg.service(core::get_node_hierarchy);
    cfg.service(core::get_reference_nodes);

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
