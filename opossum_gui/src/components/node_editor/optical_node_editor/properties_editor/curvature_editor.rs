use crate::components::node_editor::{
    inputs::{
        InputData, InputParam,
        input_components::{InputParamLabeledInput, NodeConfigUnitInput},
    },
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};
use approx::relative_ne;
use core::f64;
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::{
    meter,
    prelude::{Proptype, millimeter},
};
use uom::si::f64::Length;
use uuid::Uuid;

#[component]
pub fn CurvatureEditor(
    node_id: Memo<Uuid>,
    curvature: Length,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut curvature_sig = use_signal(|| curvature);
    let mut is_finite_sig = use_signal(|| curvature.is_finite());

    let mut last_finite_curvature = use_signal(|| {
        if curvature.is_finite() {
            curvature
        } else {
            millimeter!(1000.)
        }
    });

    use_effect(move || {
        if *is_finite_sig.read() {
            let last_finite = *last_finite_curvature.read();
            curvature_sig.set(last_finite);
        } else {
            curvature_sig.set(millimeter!(f64::INFINITY));
        }
    });

    use_effect(move || {
        let current_val = curvature_sig.read().value;
        if current_val.is_finite() {
            last_finite_curvature.set(meter!(current_val));
        }
    });

    let prop_key_clone = property_key.clone();
    let on_save = EventHandler::new(move |new_val: Length| {
        on_change.call(NodeChangeEvent {
            node_id: *node_id.read(),
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
                    value: curvature_sig.read().value,
                    base_unit: "m",
                    onchange: move |new_curv: f64| {
                        if relative_ne!(curvature_sig.read().value, new_curv) {
                            curvature_sig.set(meter!(new_curv));
                            on_save.call(meter!(new_curv));
                        }
                    },
                    readonly: !*is_finite_sig.read(),
                }
            }
            div { class: "col-sm",
                CurvatureSelector {
                    is_finite_sig,
                    property_key,
                    on_is_curved_change: move |is_finite| {
                        is_finite_sig.set(is_finite);
                        on_save.call(millimeter!(f64::INFINITY));
                    },
                }
            }
        }
    }
}

#[component]
fn CurvatureSelector(
    is_finite_sig: ReadSignal<bool>,
    property_key: String,
    on_is_curved_change: EventHandler<bool>,
) -> Element {
    let legacy_callback = EventHandler::new(move |e: Event<FormData>| {
        if let Ok(is_finite) = e.data.value().parse::<bool>() {
            on_is_curved_change.call(is_finite);
        }
    });
    let dummy_str_callback = EventHandler::new(|_| {});
    let checkbox_input = InputData::new(
        InputParam::Bool("Curved".into()),
        format!("curvatureSelectProperty{property_key}")
            .to_camel_case()
            .as_str(),
        legacy_callback,
        dummy_str_callback,
        is_finite_sig.read().to_string(),
    );
    rsx! {
        InputParamLabeledInput { input_data: checkbox_input }
    }
}
