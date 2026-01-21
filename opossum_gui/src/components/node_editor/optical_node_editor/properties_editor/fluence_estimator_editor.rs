use crate::components::node_editor::{
    hooks::use_update_signal_with_reactive_prop,
    inputs::{input_components::LabeledSelect, select_options_from_enum_iterator},
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::{
    surface::hit_map::fluence_estimator::FluenceEstimator,
    utils::default_from_name::DefaultFromName,
};
use uuid::Uuid;

#[component]
pub fn FluenceEstimatorEditor(
    node_id: Uuid,
    fluence_estimator: FluenceEstimator,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut fluence_estimator_sig = use_signal(|| fluence_estimator);
    let bound_node_id = use_signal(|| node_id);
    use_update_signal_with_reactive_prop(node_id, bound_node_id);
    use_set_node_change_property(
        *bound_node_id.read(),
        &property_key,
        fluence_estimator,
        fluence_estimator_sig,
        on_change,
    );

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
