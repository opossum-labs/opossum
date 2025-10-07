#![allow(clippy::derive_partial_eq_without_eq)]

use crate::components::node_editor::CallbackWrapper;
use crate::components::node_editor::inputs::input_components::{LabeledInput, LabeledSelect};
use crate::components::node_editor::inputs::select_options_from_enum_iterator;
use crate::components::node_editor::node_config_editor::NodeChangeAction;
use crate::components::node_editor::optical_node_editor::general_editor::NodeTypeInput;
use crate::components::scenery_editor::NodeElement;
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use opossum_backend::{
    AnalyzerType, DefaultFromName, FluenceEstimator, GhostFocusConfig, MissedSurfaceStrategy,
    RayTraceConfig, picojoule,
};
use uom::si::energy::picojoule;

#[component]
pub fn AnalyzerNodeEditor() -> Element {
    let node_element_sig = use_context::<Signal<Option<NodeElement>>>();
    let resource_future = use_resource(move || async move {
        let node = node_element_sig.read();
        if let Some(node) = &*(node) {
            match api::get_analyzer_info(node.id()).await {
                Ok(analyzer_info) => Some(analyzer_info),
                Err(err_str) => {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                    None
                }
            }
        } else {
            None
        }
    });

    match &*resource_future.read_unchecked() {
        Some(Some(analyzer_info)) => {
            rsx! {
                div {
                    h6 { "Analyzer Configuration" }
                    div {
                        class: "accordion accordion-borderless bg-dark ",
                        id: "accordionAnalyzerConfig",
                        NodeTypeInput {
                            node_type: format!("{}", analyzer_info.analyzer_type()),
                            label: "Analyzer Type",
                        }
                        {
                            match analyzer_info.analyzer_type().clone() {
                                AnalyzerType::Energy => rsx! {},
                                AnalyzerType::RayTrace(ray_trace_config) => {
                                    rsx! {
                                        RayTraceEditor { ray_trace_config }
                                    }
                                }
                                AnalyzerType::GhostFocus(ghost_focus_config) => {
                                    rsx! {
                                        GhostFocusEditor { ghost_focus_config }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => {
            rsx! {
                div { "No node selected" }
            }
        }
    }
}

#[component]
pub fn RayTraceEditor(ray_trace_config: RayTraceConfig) -> Element {
    let mut ray_trace_config_sig = use_signal(|| ray_trace_config);
    let node_config_processor = use_coroutine_handle::<NodeChangeAction>();
    use_effect(move || {
        if ray_trace_config != *ray_trace_config_sig.read() {
            node_config_processor.send(NodeChangeAction::AnalyzerType(AnalyzerType::RayTrace(
                *ray_trace_config_sig.read(),
            )));
        }
    });

    rsx! {
        LabeledInput {
            id: "rayTraceAnalyzerConfigMaxRefractions",
            label: "Max refractions",
            value: format!("{}", ray_trace_config_sig.read().max_number_of_refractions()),
            onchange: CallbackWrapper::new(move |e: Event<FormData>| {
                if let Ok(max_refractions) = e.data.parsed::<usize>() {
                    ray_trace_config_sig.write().set_max_number_of_refractions(max_refractions);
                }
            }),
            r#type: "number",
            step: Some("1"),
            min: Some("0"),
        }
        LabeledInput {
            id: "rayTraceAnalyzerConfigMaxBounces",
            label: "Max bounces",
            value: format!("{}", ray_trace_config_sig.read().max_number_of_bounces()),
            onchange: CallbackWrapper::new(move |e: Event<FormData>| {
                if let Ok(max_bounces) = e.data.parsed::<usize>() {
                    ray_trace_config_sig.write().set_max_number_of_bounces(max_bounces);
                }
            }),
            r#type: "number",
            step: Some("1"),
            min: Some("0"),
        }
        LabeledInput {
            id: "rayTraceAnalyzerConfigMinRayEnergy",
            label: "Minimum ray energy in pJ",
            value: format!("{}", ray_trace_config_sig.read().min_energy_per_ray().get::<picojoule>()),
            onchange: CallbackWrapper::new(move |e: Event<FormData>| {
                let old_value = ray_trace_config_sig.read().min_energy_per_ray();
                if let Ok(min_ray_energy) = e.data.parsed::<f64>() {
                    if min_ray_energy < 0.0 {
                        OPOSSUM_UI_LOGS
                            .write()
                            .add_log("Minimum ray energy must be non-negative.");
                        ray_trace_config_sig
                            .write()
                            .set_min_energy_per_ray(old_value)
                            .unwrap_or_else(|err| {
                                OPOSSUM_UI_LOGS.write().add_log(&err.to_string());
                            });
                    } else {
                        ray_trace_config_sig
                            .write()
                            .set_min_energy_per_ray(picojoule!(min_ray_energy))
                            .unwrap_or_else(|err| {
                                OPOSSUM_UI_LOGS.write().add_log(&err.to_string());
                            });
                    }
                }
            }),
            r#type: "number",
            step: Some("1."),
            min: Some("0."),
        }
        LabeledSelect {
            id: "rayTracingAnalyzerMissedSurfaceStrategy",
            label: "Missed-Surface Strategy",
            options: select_options_from_enum_iterator(
                ray_trace_config_sig.read().missed_surface_strategy(),
                None,
            ),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(surface_strategy) = MissedSurfaceStrategy::default_from_name(
                    val.as_str(),
                ) {
                    ray_trace_config_sig.write().set_missed_surface_strategy(surface_strategy);
                }
            },
        }
    }
}

#[component]
pub fn GhostFocusEditor(ghost_focus_config: GhostFocusConfig) -> Element {
    let mut ghost_focus_config_sig = use_signal(|| ghost_focus_config.clone());
    let node_config_processor = use_coroutine_handle::<NodeChangeAction>();
    use_effect(move || {
        if ghost_focus_config != *ghost_focus_config_sig.read() {
            node_config_processor.send(NodeChangeAction::AnalyzerType(AnalyzerType::GhostFocus(
                ghost_focus_config_sig.read().clone(),
            )));
        }
    });

    rsx! {
        LabeledInput {
            id: "ghostFocusAnalyzerConfigMaxBounces",
            label: "Max Bounces",
            value: format!("{}", ghost_focus_config_sig.read().max_bounces()),
            onchange: CallbackWrapper::new(move |e: Event<FormData>| {
                if let Ok(max_bounces) = e.data.parsed::<usize>() {
                    ghost_focus_config_sig.write().set_max_bounces(max_bounces);
                }
            }),
            r#type: "number",
            step: Some("1"),
            min: Some("0"),
        }
        LabeledSelect {
            id: "ghostFocusAnalyzerConfigFluenceEstimator",
            label: "Fluence Estimator",
            options: select_options_from_enum_iterator(
                ghost_focus_config_sig.read().fluence_estimator(),
                None,
            ),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(fluence_estimator) = FluenceEstimator::default_from_name(
                    val.as_str(),
                ) {
                    ghost_focus_config_sig.write().set_fluence_estimator(fluence_estimator);
                }
            },
        }
    }
}
