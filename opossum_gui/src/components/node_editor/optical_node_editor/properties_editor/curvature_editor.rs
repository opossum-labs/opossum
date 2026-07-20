use crate::components::node_editor::{
    hooks::use_synced_signal,
    inputs::{
        InputData, InputParam,
        input_components::{InputParamLabeledInput, NodeConfigUnitInput, UnitHandling},
    },
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};
use approx::relative_ne;
use core::f64;
use dioxus::prelude::*;
use inflector::Inflector;
use opossum_core::{meter, prelude::Proptype};
use uom::si::f64::Length;
use uuid::Uuid;

#[component]
pub fn CurvatureEditor(
    node_id: Memo<Uuid>,
    curvature: Length,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let mut curvature_sig = use_synced_signal(curvature);
    let is_finite_sig = use_memo(move || curvature_sig.read().is_finite());
    let mut last_finite_curvature = use_signal(|| {
        if curvature.is_finite() {
            curvature
        } else {
            meter!(1.)
        }
    });

    // Cannot use on_save_proptype_handler here to reduce code duplication because Curvature and Length Proptypes are ambigous
    let on_save = EventHandler::new({
        let property_key = property_key.clone();
        move |new_val: Length| {
            if relative_ne!(curvature_sig.read().value, new_val.value, epsilon = 0.0) {
                on_change.call(NodeChangeEvent {
                    node_id: *node_id.read(),
                    action: NodeChangeAction::Property(
                        property_key.clone(),
                        Proptype::Curvature(new_val),
                    ),
                });
                curvature_sig.set(new_val);
                if new_val.value.is_finite() {
                    last_finite_curvature.set(meter!(new_val.value));
                }
            }
        }
    });

    rsx! {
        div { class: "row gy-1 gx-2",
            div { class: "col-sm",
                NodeConfigUnitInput {
                    id: format!("curvatureProperty{property_key}").to_camel_case().as_str(),
                    label: property_key.to_sentence_case(),
                    value: curvature_sig.read().value,
                    unit_config: UnitHandling::new("m", true),
                    onchange: move |new_curv: f64| {
                        on_save.call(meter!(new_curv));
                    },
                    readonly: readonly || !*is_finite_sig.read(),
                }
            }
            div { class: "col-sm",
                CurvatureSelector {
                    is_finite_sig,
                    property_key,
                    readonly,
                    on_is_curved_change: move |is_finite| {
                        let new_val = if is_finite {
                            *last_finite_curvature.read()
                        } else {
                            meter!(f64::INFINITY)
                        };
                        on_save.call(new_val);
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
    readonly: bool,
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
        readonly,
    );
    rsx! {
        InputParamLabeledInput { input_data: checkbox_input }
    }
}
