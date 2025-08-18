use crate::components::node_editor::{
    inputs::{input_components::LabeledSelect, select_options_from_enum_iterator},
    node_config_editor::NodeChangeAction,
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{DefaultFromName, FluenceEstimator};

#[component]
pub fn FluenceEstimatorEditor(
    fluence_estimator: FluenceEstimator,
    property_key: String,
) -> Element {
    let mut fluence_estimator_sig = use_signal(|| fluence_estimator.clone());
    use_set_node_change_property(&property_key, fluence_estimator, fluence_estimator_sig);

    rsx! {
        LabeledSelect {
            id: format!("fluenceEstimatorProperty{property_key}").to_camel_case(),
            label: property_key.to_sentence_case(),
            options: select_options_from_enum_iterator(&*fluence_estimator_sig.read(), None),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(fluence_estimator_type) = FluenceEstimator::default_from_name(
                    val.as_str(),
                ) {
                    fluence_estimator_sig.set(fluence_estimator_type);
                }
            },
        }
    }
}
