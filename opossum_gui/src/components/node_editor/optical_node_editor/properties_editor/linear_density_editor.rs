use crate::components::node_editor::{
    CallbackWrapper, inputs::input_components::LabeledInput,
    optical_node_editor::properties_editor::use_set_node_change_property,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::num_per_mm;
use uom::si::{f64::LinearNumberDensity, linear_number_density::per_millimeter};
use uuid::Uuid;

#[component]
pub fn LinearDensityEditor(
    node_id: Uuid,
    linear_density: LinearNumberDensity, 
    property_key: String
) -> Element {
    let linear_density_sig = use_signal(|| linear_density);
    use_set_node_change_property(node_id, &property_key, linear_density, linear_density_sig);
    rsx! {
        LabeledInput {
            id: format!("linearDensityProperty{property_key}").to_camel_case(),
            label: format!("{} in 1/mm", property_key.to_sentence_case()),
            value: format!("{:.3}", linear_density_sig.read().get::<per_millimeter>()),
            r#type: "number",
            onchange: on_linear_density_input_change(linear_density_sig),
        }
    }
}
fn on_linear_density_input_change(mut signal: Signal<LinearNumberDensity>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(linear_density) = e.data.value().parse::<f64>() {
            signal.set(num_per_mm!(linear_density));
        }
    })
}