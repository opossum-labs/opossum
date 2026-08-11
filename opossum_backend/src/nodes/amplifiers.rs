use crate::{
    app_state::AppState, error::BackEndErrorResponse, helper_functions::collect_nodes_recursive,
};
use actix_web::{HttpResponse, get, web};
use opossum_core::{
    core_optics::{NodeAttr, NodeAttrExt, node_attr::HasNodeAttr},
    gain::active_amp_model,
    types::api_types::{AmplifierDto, ErrorResponse},
};
use std::collections::HashMap;
use uuid::Uuid;

/// Get every amplifying node of the whole document
///
/// Walks the document recursively (nested groups included) and returns one entry per node whose
/// `amp config` property holds an active gain model. Passive nodes are not listed.
#[utoipa::path(
    tag = "node",
    responses(
        (status = OK, description = "List of all amplifying nodes in the document", body = Vec<AmplifierDto>),
        (status = INTERNAL_SERVER_ERROR, body = ErrorResponse, description = "Internal tree traversal error")
    )
)]
#[get("/amplifiers")]
pub async fn get_amplifiers(
    data: web::Data<AppState>,
) -> Result<HttpResponse, BackEndErrorResponse> {
    let document = data.document.lock();
    let scenery = document.scenery().clone();
    drop(document);

    let mut collected = Vec::new();
    collect_nodes_recursive(
        &scenery,
        scenery.node_attr().uuid(),
        &|node_attr: &NodeAttr| {
            active_amp_model(node_attr).map(|amp_model| {
                (
                    node_attr.name().to_string(),
                    node_attr.node_type().to_string(),
                    amp_model,
                )
            })
        },
        &mut collected,
    );

    // Resolve each *distinct* group once rather than per amplifier - a subsystem typically holds
    // several of them.
    let mut group_names: HashMap<Uuid, String> = HashMap::new();
    for node in &collected {
        group_names.entry(node.group_id).or_insert_with(|| {
            scenery
                .with_group_node(node.group_id, |group| group.name().to_string())
                .unwrap_or_default()
        });
    }

    let amplifiers: Vec<AmplifierDto> = collected
        .into_iter()
        .map(|node| {
            let (name, node_type, amp_model) = node.value;
            let group_name = group_names.get(&node.group_id).cloned().unwrap_or_default();
            AmplifierDto {
                uuid: node.uuid,
                name,
                node_type,
                group_id: node.group_id,
                group_name,
                amp_model,
            }
        })
        .collect();

    Ok(HttpResponse::Ok().json(amplifiers))
}

#[cfg(test)]
mod test {
    use super::*;
    use actix_web::{App, dev::Service, http::StatusCode, test, web::Data};
    use opossum_core::{
        gain::{AMP_CONFIG, ConstGain, GainModel},
        nodes::{Lens, NodeGroup, Wedge},
    };

    fn create_test_state() -> Data<AppState> {
        Data::new(AppState::default())
    }

    /// The list must reach into nested groups and report which group each amplifier sits in - that
    /// group id is what lets the overview panel open the right tab.
    #[actix_web::test]
    async fn test_get_amplifiers_finds_nested_nodes_with_their_group() {
        let app_state = create_test_state();
        let (root_id, group_id, lens_id, wedge_id) = {
            let mut document = app_state.document.lock();
            let root_id = document.scenery().node_attr().uuid();
            let lens_id = document.scenery_mut().add_node(Lens::default()).unwrap();
            let group_id = document
                .scenery_mut()
                .add_node(NodeGroup::new("subgroup"))
                .unwrap();
            let wedge_id = document
                .scenery_mut()
                .with_group_node_mut(group_id, |group| group.add_node(Wedge::default()).unwrap())
                .unwrap();
            // Only the lens and the wedge amplify; the group and everything else stays passive.
            for id in [lens_id, wedge_id] {
                document
                    .scenery_mut()
                    .with_node_attr_mut(id, |attr| {
                        attr.set_property(AMP_CONFIG, GainModel::Const(ConstGain::default()).into())
                    })
                    .unwrap()
                    .unwrap();
            }
            let ids = (root_id, group_id, lens_id, wedge_id);
            drop(document);
            ids
        };

        let app = test::init_service(App::new().app_data(app_state).service(get_amplifiers)).await;
        let req = test::TestRequest::get().uri("/amplifiers").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let amplifiers: Vec<AmplifierDto> = test::read_body_json(resp).await;

        assert_eq!(amplifiers.len(), 2, "expected exactly the two amplifiers");
        let lens = amplifiers.iter().find(|a| a.uuid == lens_id).unwrap();
        assert_eq!(lens.group_id, root_id);
        assert_eq!(lens.node_type, "lens");
        assert_eq!(lens.amp_model, "Const");
        let wedge = amplifiers.iter().find(|a| a.uuid == wedge_id).unwrap();
        assert_eq!(
            wedge.group_id, group_id,
            "an amplifier inside a group must be reported with that group's id, not the root's"
        );
        assert_eq!(
            wedge.group_name, "subgroup",
            "the group's display name is what the overview's subsystem filter offers"
        );
    }

    /// A document without amplifiers yields an empty list, not an error - and a passive volume node
    /// must not show up just because it *could* amplify.
    #[actix_web::test]
    async fn test_get_amplifiers_skips_passive_nodes() {
        let app_state = create_test_state();
        {
            let mut document = app_state.document.lock();
            document.scenery_mut().add_node(Lens::default()).unwrap();
        }

        let app = test::init_service(App::new().app_data(app_state).service(get_amplifiers)).await;
        let req = test::TestRequest::get().uri("/amplifiers").to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let amplifiers: Vec<AmplifierDto> = test::read_body_json(resp).await;
        assert!(amplifiers.is_empty());
    }
}
