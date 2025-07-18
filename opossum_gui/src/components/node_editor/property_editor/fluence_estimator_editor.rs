use crate::components::node_editor::inputs::{
    input_components::LabeledSelect, select_options_from_enum_iterator,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{DefaultFromName, FluenceEstimator, Proptype};

#[component]
pub fn FluenceEstimatorEditor(
    fluence_estimator: FluenceEstimator,
    property_key: String,
    prop_type_sig: Signal<Proptype>,
) -> Element {
    let select_id = format!("fluenceEstimatorProperty{property_key}").to_camel_case();
    let select_label = property_key.to_sentence_case();

    rsx! {
        LabeledSelect {
            id: select_id,
            label: select_label,
            options: select_options_from_enum_iterator(&fluence_estimator, None),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(fluence_estimator_type) = FluenceEstimator::default_from_name(
                    val.as_str(),
                ) {
                    prop_type_sig.set(fluence_estimator_type.into());
                }
            },
        }
    }
}
