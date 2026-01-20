use core::f64;

use crate::components::node_editor::{
    CallbackWrapper,
    inputs::{InputData, InputParam, input_components::InputParamLabeledInput},
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
    optical_node_editor::properties_editor::use_update_signal_with_reactive_prop,
};
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::prelude::{Proptype, millimeter};
use uom::si::{f64::Length, length::millimeter};
use uuid::Uuid;

#[component]
pub fn CurvatureEditor(
    node_id: Uuid,
    curvature: Length,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let curvature_sig = use_signal(|| curvature);
    let mut last_finite_curvature = use_signal(|| {
        if curvature.is_finite() {
            curvature
        } else {
            millimeter!(1000.)
        }
    });
    let bound_node_id = use_signal(|| node_id);
    use_update_signal_with_reactive_prop(node_id, bound_node_id);
    use_update_signal_with_reactive_prop(curvature, curvature_sig);
    use_effect(move || {
        let current_val = *curvature_sig.read();
        if current_val.is_finite() {
            last_finite_curvature.set(current_val);
        }
    });
    use_effect({
        let property_key = property_key.clone();
        move || {
            if curvature != *curvature_sig.read() {
                on_change.call(NodeChangeEvent {
                    node_id: *bound_node_id.peek(),
                    action: NodeChangeAction::Property(
                        property_key.clone(),
                        Proptype::Curvature(*curvature_sig.read()),
                    ),
                });
            }
        }
    });

    rsx! {
        div { class: "row gy-1 gx-2",
            div { class: "col-sm",
                CurvatureInput {
                    curvature,
                    curvature_sig,
                    last_finite_curvature,
                    property_key: property_key.clone(),
                }
            }
            div { class: "col-sm",
                CurvatureSelector {
                    curvature,
                    curvature_sig,
                    last_finite_curvature,
                    property_key,
                }
            }
        }
    }
}

// --- Helper Components & Functions (bleiben weitgehend gleich) ---

#[component]
fn CurvatureSelector(
    curvature: Length,
    curvature_sig: Signal<Length>,
    last_finite_curvature: Signal<Length>,
    property_key: String,
) -> Element {
    let checkbox_input = InputData::new(
        InputParam::Bool("Curved".into()),
        format!("curvatureSelectProperty{property_key}")
            .to_camel_case()
            .as_str(),
        on_is_curved_input_change(curvature_sig, last_finite_curvature),
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
    last_finite_curvature: Signal<Length>,
    property_key: String,
) -> Element {
    let mut curvature_input = InputData::new(
        InputParam::Length(format!("{} in mm", property_key.to_sentence_case())),
        format!("curvatureProperty{property_key}")
            .to_camel_case()
            .as_str(),
        on_length_input_change(curvature_sig, last_finite_curvature),
        format!("{:.3}", curvature_sig.read().get::<millimeter>()),
    );
    curvature_input.readonly = curvature.is_infinite();

    rsx! {
        InputParamLabeledInput { input_data: curvature_input }
    }
}

fn on_is_curved_input_change(
    mut curvature_sig: Signal<Length>,
    last_finite_curvature: Signal<Length>,
) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(is_finite) = e.data.value().parse::<bool>() {
            if is_finite {
                curvature_sig.set(*last_finite_curvature.read());
            } else {
                curvature_sig.set(millimeter!(f64::INFINITY));
            }
        }
    })
}

fn on_length_input_change(
    mut signal: Signal<Length>,
    mut last_finite_curvature: Signal<Length>,
) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(length) = e.data.value().parse::<f64>() {
            let new_length = millimeter!(length);
            signal.set(new_length);
            last_finite_curvature.set(new_length);
        }
    })
}
