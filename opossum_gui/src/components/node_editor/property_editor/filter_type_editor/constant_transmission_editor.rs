use approx::relative_eq;
use dioxus::prelude::*;
use opossum_backend::{FilterTypeBuilder, Property, Proptype};

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{CallbackWrapper, inputs::input_components::LabeledInput},
};

#[component]
pub fn ConstantFilterTypeEditor(
    transmission: f64,
    filter_type_builder_sig: Signal<FilterTypeBuilder>,
) -> Element {
    let transmission_sig = use_signal(|| transmission);
    let property = use_context::<Property>();
    use_effect(move || {
        if relative_eq!(transmission, *transmission_sig.read()) {
            filter_type_builder_sig.set(FilterTypeBuilder::Constant(*transmission_sig.read()));
        }
    });
    rsx! {
        LabeledInput {
            id: "constFilterTypeInput",
            label: "Transmission",
            value: format!("{:.3}", transmission_sig.read()),
            r#type: "number",
            step: Some("0.01"),
            min: Some("0."),
            max: Some("1."),
            onchange: on_transmission_input_change(transmission_sig, property),
        }
    }
}

pub fn on_transmission_input_change(
    mut signal: Signal<f64>,
    property: Property,
) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(val) = e.data.value().parse::<f64>() {
            let last_val = *signal.read();
            match property.validate_proptype(&Proptype::FilterTypeBuilder(
                FilterTypeBuilder::Constant(val),
            )) {
                Ok(()) => signal.set(val),
                Err(e) => {
                    OPOSSUM_UI_LOGS.write().add_log(format!("{e}").as_str());
                    signal.set(last_val);
                }
            }
        }
    })
}
