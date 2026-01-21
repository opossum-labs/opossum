use crate::components::node_editor::{
    inputs::{InputData, InputParam, input_components::InputParamLabeledInput},
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
    optical_node_editor::properties_editor::use_update_signal_with_reactive_prop,
};
use core::f64;
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

// --- Helper Components & Functions ---

#[component]
fn CurvatureSelector(
    curvature: Length,
    curvature_sig: Signal<Length>,
    last_finite_curvature: Signal<Length>,
    property_key: String,
) -> Element {
    // Checkbox nutzt den klassischen EventHandler<Event<FormData>>
    let legacy_callback = on_is_curved_input_change(curvature_sig, last_finite_curvature);
    // Dummy für String-Callback (wird bei Checkbox nicht genutzt)
    let dummy_str_callback = EventHandler::new(|_| {});

    let checkbox_input = InputData::new(
        InputParam::Bool("Curved".into()),
        format!("curvatureSelectProperty{property_key}")
            .to_camel_case()
            .as_str(),
        legacy_callback,
        dummy_str_callback,
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
    // Text Input nutzt den neuen String-Callback für FlushableTextInput
    let str_callback = on_length_input_change_str(curvature_sig, last_finite_curvature);
    // Dummy für Legacy-Callback (wird bei Length Input nicht mehr genutzt)
    let dummy_legacy_callback = EventHandler::new(|_| {});

    let mut curvature_input = InputData::new(
        InputParam::Length(format!("{} in mm", property_key.to_sentence_case())),
        format!("curvatureProperty{property_key}")
            .to_camel_case()
            .as_str(),
        dummy_legacy_callback,
        str_callback,
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
) -> EventHandler<Event<FormData>> {
    EventHandler::new(move |e: Event<FormData>| {
        if let Ok(is_finite) = e.data.value().parse::<bool>() {
            if is_finite {
                curvature_sig.set(*last_finite_curvature.read());
            } else {
                curvature_sig.set(millimeter!(f64::INFINITY));
            }
        }
    })
}
fn on_length_input_change_str(
    mut signal: Signal<Length>,
    mut last_finite_curvature: Signal<Length>,
) -> EventHandler<String> {
    EventHandler::new(move |val_str: String| {
        if let Ok(length) = val_str.parse::<f64>() {
            let new_length = millimeter!(length);
            signal.set(new_length);
            last_finite_curvature.set(new_length);
        }
    })
}
