use core::f64;

use crate::components::node_editor::{
    inputs::{input_components::{InputParamLabeledInput, LabeledCheckboxInput, LabeledInput, LabeledSelect, RowedInputs}, InputData, InputParam},
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
    let length_sig = use_signal(|| length);

    let checkbox_input = InputData::new(InputParam::Bool("Curved"), format!("curvatureSelectProperty{property_key}").to_camel_case(), on_is_curved_input_change(length_sig), length_sig.read().is_finite().to_string());
    let mut curvature_input = InputData::new(InputParam::Length("Curvature in mm"), format!("curvatureProperty{property_key}").to_camel_case(), on_length_input_change(length_sig), format!("{:.3}",length_sig.read().get::<millimeter>()));
    curvature_input.readonly = length_sig.read().is_infinite();

    use_effect(move || {
        println!("");
        prop_type_sig.set(Proptype::Curvature(*length_sig.read()))
    });

    rsx! {
        RowedInputs{inputs: vec![curvature_input, checkbox_input]}
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
