use crate::{analyzers, document, general, nodes, operations, pump_scenarios};
use utoipa_actix_web::{scope, service_config::ServiceConfig};

pub fn root_config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(scope("/api/document").configure(document::config));
    cfg.service(scope("/api/nodes").configure(nodes::config));
    cfg.service(scope("/api/analyzers").configure(analyzers::config));
    cfg.service(scope("/api/pump_scenarios").configure(pump_scenarios::config));
    cfg.service(scope("/api/operations").configure(operations::config));
    cfg.service(scope("/api").configure(general::config));
}
