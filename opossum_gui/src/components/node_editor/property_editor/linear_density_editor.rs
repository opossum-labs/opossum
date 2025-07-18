use crate::components::node_editor::{inputs::input_components::LabeledInput, CallbackWrapper};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{num_per_mm, Proptype};
use uom::si::{f64::LinearNumberDensity, linear_number_density::per_millimeter};

#[component]
pub fn LinearDensityEditor(
    linear_density: LinearNumberDensity,
    property_key: String,
    prop_type_sig: Signal<Proptype>,
) -> Element {
    rsx! {
        LabeledInput {
            id: format!("linearDensityProperty{property_key}").to_camel_case(),
            label: format!("{} in 1/mm", property_key.to_sentence_case()),
            value: format!("{}", linear_density.get::<per_millimeter>()),
            r#type: "number",
            onchange: on_linear_density_input_change(prop_type_sig),
        }
    }
}

fn on_linear_density_input_change(mut signal: Signal<Proptype>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(length) = e.data.value().parse::<f64>() {
            signal.set(Proptype::LinearDensity(num_per_mm!(length)));
        }
    })
}
