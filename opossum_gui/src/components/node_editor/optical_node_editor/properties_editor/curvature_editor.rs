use crate::components::node_editor::{
    hooks::use_update_signal_with_reactive_prop,
    inputs::{InputData, InputParam, input_components::{InputParamLabeledInput, NodeConfigUnitInput}},
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};
use core::f64;
use approx::relative_ne;
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::{meter, prelude::{Proptype, millimeter}};
use uom::si::f64::Length;
use uuid::Uuid;

#[component]
pub fn CurvatureEditor(
    node_id: Uuid,
    curvature: Length,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let curvature_sig = use_signal(|| curvature);
    let curv_value_memo = use_memo(move || curvature_sig.read().value);
    let mut last_finite_curvature = use_signal(|| {
        if curvature.is_finite() {
            curvature
        } else {
            millimeter!(1000.)
        }
    });

    use_update_signal_with_reactive_prop(curvature, curvature_sig);

    use_effect(move || {
        let current_val = *curvature_sig.read();
        if current_val.is_finite() {
            last_finite_curvature.set(current_val);
        }
    });

    // FIX: Clone property_key FOR the closure
    let prop_key_clone = property_key.clone();
    let on_save = EventHandler::new(move |new_val: Length| {
        on_change.call(NodeChangeEvent {
            node_id,
            action: NodeChangeAction::Property(
                prop_key_clone.clone(),
                Proptype::Curvature(new_val),
            ),
        });
    });

    rsx! {
        div { class: "row gy-1 gx-2",
            div { class: "col-sm",
                NodeConfigUnitInput {
                    id: format!("curvatureProperty{property_key}").to_camel_case().as_str(),
                    label: property_key.to_sentence_case(),
                    value: curv_value_memo,
                    base_unit: "m",
                    onchange: move |new_curv: f64| {
                        if relative_ne!(curvature_sig.read().value, new_curv) {
                            on_save.call(meter!(new_curv));
                        }
                    },
                    readonly: curvature.is_infinite(),
                }
            }
            div { class: "col-sm",
                CurvatureSelector {
                    curvature,
                    curvature_sig,
                    last_finite_curvature,
                    property_key,
                    on_save,
                }
            }
        }
    }
}
// ... (Helper Components CurvatureSelector, CurvatureInput etc. bleiben unverändert wie zuvor) ...
// Hier bitte die Helper aus meinem vorletzten Post einfügen, die waren korrekt.
// Kurzfassung:
#[component]
fn CurvatureSelector(
    curvature: Length,
    curvature_sig: Signal<Length>,
    last_finite_curvature: Signal<Length>,
    property_key: String,
    on_save: EventHandler<Length>,
) -> Element {
    let legacy_callback = on_is_curved_input_change(curvature_sig, last_finite_curvature, on_save);
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
    on_save: EventHandler<Length>,
) -> Element {
    let value_memo = use_memo(move || curvature_sig.read().value);
    rsx! {
        NodeConfigUnitInput {
            id: format!("curvatureProperty{property_key}").to_camel_case().as_str(),
            label: property_key.to_sentence_case(),
            value: value_memo,
            base_unit: "m",
            onchange: move |new_curv: f64| {
                if relative_ne!(curvature_sig.read().value, new_curv) {
                    on_save
                        .call(meter!(new_curv));
                }
            },
            readonly: curvature.is_infinite(),
        }
    }
}

fn on_is_curved_input_change(
    mut curvature_sig: Signal<Length>,
    last_finite_curvature: Signal<Length>,
    on_save: EventHandler<Length>,
) -> EventHandler<Event<FormData>> {
    EventHandler::new(move |e: Event<FormData>| {
        if let Ok(is_finite) = e.data.value().parse::<bool>() {
            let new_val = if is_finite {
                *last_finite_curvature.read()
            } else {
                millimeter!(f64::INFINITY)
            };
            curvature_sig.set(new_val);
            on_save.call(new_val);
        }
    })
}

fn on_length_input_change_str(
    mut signal: Signal<Length>,
    mut last_finite_curvature: Signal<Length>,
    on_save: EventHandler<Length>,
) -> EventHandler<String> {
    EventHandler::new(move |val_str: String| {
        if let Ok(length) = val_str.parse::<f64>() {
            let new_length = millimeter!(length);
            signal.set(new_length);
            last_finite_curvature.set(new_length);
            on_save.call(new_length);
        }
    })
}
