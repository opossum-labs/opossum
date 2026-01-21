#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::node_editor::{
    inputs::{
        input_components::{LabeledInput, LabeledSelect},
        select_options_from_enum_iterator,
    },
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
    optical_node_editor::{
        general_editor::NodeTypeInput, properties_editor::use_update_signal_with_reactive_prop,
    },
};
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use opossum_core::{
    analyzers::raytrace::MissedSurfaceStrategy, picojoule, prelude::*,
    surface::hit_map::fluence_estimator::FluenceEstimator,
    utils::default_from_name::DefaultFromName,
};
use uom::si::energy::picojoule;
use uuid::Uuid;

#[component]
pub fn AnalyzerNodeEditor(node_id: Uuid, on_change: EventHandler<NodeChangeEvent>) -> Element {
    let resource_future = use_resource(move || async move {
        match api::get_analyzer_info(node_id).await {
            Ok(analyzer_info) => Some(analyzer_info),
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                None
            }
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
                                        RayTraceEditor { node_id, ray_trace_config, on_change }
                                    }
                                }
                                AnalyzerType::GhostFocus(ghost_focus_config) => {
                                    rsx! {
                                        GhostFocusEditor { node_id, ghost_focus_config, on_change }
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
pub fn RayTraceEditor(
    node_id: Uuid,
    ray_trace_config: RayTraceConfig,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut ray_trace_config_sig = use_signal(|| ray_trace_config);
    use_update_signal_with_reactive_prop(ray_trace_config, ray_trace_config_sig);
    let bound_node_id = use_signal(|| node_id);
    use_update_signal_with_reactive_prop(node_id, bound_node_id);

    use_effect(move || {
        if ray_trace_config != *ray_trace_config_sig.read() {
            on_change.call(NodeChangeEvent {
                node_id: *bound_node_id.peek(),
                action: NodeChangeAction::AnalyzerType(AnalyzerType::RayTrace(
                    *ray_trace_config_sig.read(),
                )),
            });
        }
    });

    rsx! {
        LabeledInput {
            id: "rayTraceAnalyzerConfigMaxRefractions",
            label: "Max refractions",
            value: format!("{}", ray_trace_config_sig.read().max_number_of_refractions()),
            onchange: move |e: Event<FormData>| {
                if let Ok(max_refractions) = e.data.value().parse::<usize>() {
                    ray_trace_config_sig.write().set_max_number_of_refractions(max_refractions);
                }
            },
            r#type: "number",
            step: Some("1"),
            min: Some("0"),
        }
        LabeledInput {
            id: "rayTraceAnalyzerConfigMaxBounces",
            label: "Max bounces",
            value: format!("{}", ray_trace_config_sig.read().max_number_of_bounces()),
            onchange: move |e: Event<FormData>| {
                if let Ok(max_bounces) = e.data.value().parse::<usize>() {
                    ray_trace_config_sig.write().set_max_number_of_bounces(max_bounces);
                }
            },
            r#type: "number",
            step: Some("1"),
            min: Some("0"),
        }
        LabeledInput {
            id: "rayTraceAnalyzerConfigMinRayEnergy",
            label: "Minimum ray energy in pJ",
            value: format!("{}", ray_trace_config_sig.read().min_energy_per_ray().get::<picojoule>()),
            onchange: move |e: Event<FormData>| {
                let old_value = ray_trace_config_sig.read().min_energy_per_ray();
                if let Ok(min_ray_energy) = e.data.value().parse::<f64>() {
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
            },
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
pub fn GhostFocusEditor(
    node_id: Uuid,
    ghost_focus_config: GhostFocusConfig,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut ghost_focus_config_sig = use_signal(|| ghost_focus_config.clone());
    use_update_signal_with_reactive_prop(ghost_focus_config.clone(), ghost_focus_config_sig);
    let bound_node_id = use_signal(|| node_id);
    use_update_signal_with_reactive_prop(node_id, bound_node_id);

    use_effect(move || {
        if ghost_focus_config != *ghost_focus_config_sig.read() {
            on_change.call(NodeChangeEvent {
                node_id: *bound_node_id.peek(),
                action: NodeChangeAction::AnalyzerType(AnalyzerType::GhostFocus(
                    ghost_focus_config_sig.read().clone(),
                )),
            });
        }
    });

    rsx! {
        LabeledInput {
            id: "ghostFocusAnalyzerConfigMaxBounces",
            label: "Max Bounces",
            value: format!("{}", ghost_focus_config_sig.read().max_bounces()),
            onchange: move |e: Event<FormData>| {
                if let Ok(max_bounces) = e.data.value().parse::<usize>() {
                    ghost_focus_config_sig.write().set_max_bounces(max_bounces);
                }
            },
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
