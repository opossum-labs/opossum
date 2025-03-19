pub use crate::{general, scenery};
use utoipa_actix_web::{scope, service_config::ServiceConfig};

pub fn root_config(cfg: &mut ServiceConfig<'_>) {
    cfg.service(scope("/api/scenery").configure(scenery::config));
    cfg.service(scope("/api").configure(general::config));
}
