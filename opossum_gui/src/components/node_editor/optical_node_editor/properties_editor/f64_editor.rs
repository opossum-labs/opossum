use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        CallbackWrapper, inputs::input_components::LabeledInput,
        optical_node_editor::properties_editor::use_set_node_change_property,
    },
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::prelude::{Property, Proptype};

#[component]
pub fn F64Editor(float64: f64, property_key: String, property: Property) -> Element {
    let float64_sig = use_signal(|| float64);

    use_set_node_change_property(&property_key, float64, float64_sig);

    rsx! {
        LabeledInput {
            id: format!("float64Property{property_key}").to_camel_case(),
            label: format!("{}", property_key.to_sentence_case()),
            value: format!("{:.3}", float64),
            r#type: "number",
            onchange: on_float64_input_change(float64_sig, property),
        }
    }
}

pub fn on_float64_input_change(mut signal: Signal<f64>, property: Property) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(val) = e.data.value().parse::<f64>() {
            let last_val = *signal.read();
            match property.validate_proptype(&Proptype::F64(val)) {
                Ok(()) => signal.set(val),
                Err(e) => {
                    OPOSSUM_UI_LOGS.write().add_log(format!("{e}").as_str());
                    signal.set(last_val);
                }
            }
        }
    })
}
