use core::f64;

use crate::components::node_editor::{
    inputs::{input_components::{InputParamLabeledInput, LabeledCheckboxInput, LabeledInput, LabeledSelect}, InputData, InputParam},
    CallbackWrapper,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{millimeter, Proptype};
use uom::si::{f64::Length, length::millimeter};

#[component]
pub fn CurvatureEditor(
    length: Length,
    property_key: String,
    prop_type_sig: Signal<Proptype>,
) -> Element {
    let select_id = format!("lengthProperty{property_key}").to_camel_case();
    let select_label = property_key.to_sentence_case();
    let length_sig = use_signal(|| length);

    let checkbox_input = InputData::new(InputParam::Bool("Curved"), select_id.clone(), on_is_curved_input_change(length_sig), length_sig.read().is_finite().to_string());
    
    use_effect(move || {
        prop_type_sig.set((*length_sig.read()).into())
    });

    rsx! {
        InputParamLabeledInput {input_data: checkbox_input}
        LabeledInput {
                        id: format!("lengthProperty{property_key}").to_camel_case(),
                        label: format!("{:.3} in mm", property_key.to_sentence_case()),
                        value: format!("{:.3}", length_sig.read().get::<millimeter>()),
                        r#type: "number",
                        readonly: length_sig.read().is_infinite(),
                        onchange: on_length_input_change(length_sig),
                    }
    }
}

fn on_is_curved_input_change(mut signal: Signal<Length>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(is_finite) = e.data.value().parse::<bool>() {
            if is_finite{
                signal.set(millimeter!(1000.));
            }
            else{
                signal.set(millimeter!(f64::INFINITY));
            }
          
        }
    })
}

fn on_length_input_change(mut signal: Signal<Length>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(length) = e.data.value().parse::<f64>() {
            signal.set(millimeter!(length));
        }
    })
}
