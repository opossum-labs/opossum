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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use actix_web::{App, dev::Service, http::StatusCode, test, web};
    use utoipa_actix_web::AppExt;

    #[actix_web::test]
    async fn test_root_config_mounts_all_scopes() {
        let app_state = web::Data::new(AppState::default());
        let app = test::init_service(
            App::new()
                .into_utoipa_app()
                .app_data(app_state)
                .configure(root_config)
                .into_app(),
        )
        .await;

        // 1. Verify general scope endpoint
        let req_version = test::TestRequest::get().uri("/api/version").to_request();
        let resp_version = app.call(req_version).await.unwrap();
        assert_eq!(resp_version.status(), StatusCode::OK);

        // 2. Verify document scope endpoint
        let req_doc = test::TestRequest::get()
            .uri("/api/document/root_uuid")
            .to_request();
        let resp_doc = app.call(req_doc).await.unwrap();
        assert_eq!(resp_doc.status(), StatusCode::OK);

        // 3. Verify analyzers scope endpoint
        let req_analyzers = test::TestRequest::get().uri("/api/analyzers").to_request();
        let resp_analyzers = app.call(req_analyzers).await.unwrap();
        assert_eq!(resp_analyzers.status(), StatusCode::OK);
    }
}
