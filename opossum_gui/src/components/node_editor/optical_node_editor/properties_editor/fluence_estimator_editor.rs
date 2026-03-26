use crate::components::node_editor::{
    inputs::{input_components::LabeledSelect, select_options_from_enum_iterator},
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::on_save_proptype_handler,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::{
    geometry::hit_map::fluence_estimator::FluenceEstimator,
    utils::default_from_name::DefaultFromName,
};
use uuid::Uuid;

#[component]
pub fn FluenceEstimatorEditor(
    node_id: Memo<Uuid>,
    fluence_estimator: FluenceEstimator,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let fluence_estimator_sig = use_signal(|| fluence_estimator);
    let on_save = on_save_proptype_handler(
        fluence_estimator_sig,
        property_key.clone(),
        on_change,
        node_id.into(),
    );

    rsx! {
        LabeledSelect {
            id: format!("fluenceEstimatorProperty{property_key}").to_camel_case(),
            label: property_key.to_sentence_case(),
            options: select_options_from_enum_iterator(&*fluence_estimator_sig.read(), None),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(fluence_estimator_type) = FluenceEstimator::default_from_name(
                    val.as_str(),
                ) {
                    on_save.call(fluence_estimator_type);
                }
            },
        }
    }
}
