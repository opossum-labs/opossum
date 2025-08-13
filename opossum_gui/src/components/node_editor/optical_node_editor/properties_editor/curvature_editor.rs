use core::f64;

use crate::components::node_editor::{
    CallbackWrapper,
    inputs::{InputData, InputParam, input_components::RowedInputs},
    node_config_editor::NodeChangeAction,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{Proptype, millimeter};
use uom::si::{f64::Length, length::millimeter};

#[component]
pub fn CurvatureEditor(
    curvature: Length,
    property_key: String,
    node_change: Signal<Option<NodeChangeAction>>,
) -> Element {
    let curvature_sig = use_signal(|| curvature);

    use_effect({
        let property_key = property_key.clone();
        move || {
            if curvature != *curvature_sig.read() {
                node_change.set(Some(NodeChangeAction::Property(
                    property_key.clone(),
                    Proptype::Curvature(*curvature_sig.read()),
                )));
            }
        }
    });

    let checkbox_input = InputData::new(
        InputParam::Bool("Curved".into()),
        format!("curvatureSelectProperty{property_key}")
            .to_camel_case()
            .as_str(),
        on_is_curved_input_change(curvature_sig),
        curvature_sig.read().is_finite().to_string(),
    );
    let mut curvature_input = InputData::new(
        InputParam::Length(format!("{} in mm", property_key.to_sentence_case())),
        format!("curvatureProperty{property_key}")
            .to_camel_case()
            .as_str(),
        on_length_input_change(curvature_sig),
        format!("{:.3}", curvature_sig.read().get::<millimeter>()),
    );
    curvature_input.readonly = curvature_sig.read().is_infinite();

    rsx! {
        RowedInputs { inputs: vec![curvature_input, checkbox_input] }
    }
}

fn on_is_curved_input_change(mut signal: Signal<Length>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(is_finite) = e.data.value().parse::<bool>() {
            if is_finite {
                signal.set(millimeter!(1000.));
            } else {
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
