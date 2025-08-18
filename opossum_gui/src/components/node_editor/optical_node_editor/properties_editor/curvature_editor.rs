use core::f64;

use crate::components::node_editor::{
    CallbackWrapper,
    inputs::{
        InputData, InputParam,
        input_components::{InputParamLabeledInput, RowedInputs},
    },
    node_config_editor::NodeChangeAction,
    optical_node_editor::properties_editor::use_update_signal_with_reactive_prop,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_backend::{Proptype, millimeter};
use uom::si::{f64::Length, length::millimeter};

#[component]
pub fn CurvatureEditor(curvature: Length, property_key: String) -> Element {
    let curvature_sig = use_signal(|| curvature);
    use_update_signal_with_reactive_prop(curvature, curvature_sig);

    let node_change_handle = use_coroutine_handle::<NodeChangeAction>();
    use_effect({
        let property_key = property_key.clone();
        move || {
            if curvature != *curvature_sig.read() {
                node_change_handle.send(NodeChangeAction::Property(
                    property_key.clone(),
                    Proptype::Curvature(*curvature_sig.read()),
                ));
            }
        }
    });
    rsx! {
        div { class: "row gy-1 gx-2",
            div { class: "col-sm",
                CurvatureInput {
                    curvature,
                    curvature_sig,
                    property_key: property_key.clone(),
                }
            }
            div { class: "col-sm",
                CurvatureSelector { curvature, curvature_sig, property_key }
            }
        }
    }
}

#[component]
fn CurvatureSelector(
    curvature: Length,
    curvature_sig: Signal<Length>,
    property_key: String,
) -> Element {
    let checkbox_input = InputData::new(
        InputParam::Bool("Curved".into()),
        format!("curvatureSelectProperty{property_key}")
            .to_camel_case()
            .as_str(),
        on_is_curved_input_change(curvature_sig),
        curvature_sig.read().is_finite().to_string(),
    );

    rsx! {
        InputParamLabeledInput { input_data: checkbox_input }
    }
}

#[component]
fn CurvatureInput(
    curvature: Length,
    curvature_sig: Signal<Length>,
    property_key: String,
) -> Element {
    let mut curvature_input = InputData::new(
        InputParam::Length(format!("{} in mm", property_key.to_sentence_case())),
        format!("curvatureProperty{property_key}")
            .to_camel_case()
            .as_str(),
        on_length_input_change(curvature_sig),
        format!("{:.3}", curvature_sig.read().get::<millimeter>()),
    );
    curvature_input.readonly = curvature.is_infinite();

    rsx! {
        InputParamLabeledInput { input_data: curvature_input }
    }
}

fn on_is_curved_input_change(mut curvature_sig: Signal<Length>) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(is_finite) = e.data.value().parse::<bool>() {
            if is_finite {
                curvature_sig.set(millimeter!(1000.))
            } else {
                curvature_sig.set(millimeter!(f64::INFINITY))
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
