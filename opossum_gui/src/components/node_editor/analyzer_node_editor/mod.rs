#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::node_editor::{
    inputs::{
        // WICHTIG: FlushableTextInput importieren, LabeledInput entfernen (oder behalten falls nötig)
        input_components::{FlushableTextInput, LabeledSelect},
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
    // Sync Prop -> Signal: Wichtig für den Node-Wechsel!
    use_update_signal_with_reactive_prop(ray_trace_config, ray_trace_config_sig);

    rsx! {
        // NEU: FlushableTextInput statt LabeledInput
        FlushableTextInput {
            id: "rayTraceMaxRefr".to_string(),
            label: "Max refractions".to_string(),
            value: format!("{}", ray_trace_config_sig.read().max_number_of_refractions()),
            r#type: "number",
            step: "1",
            min: "0",
            // Callback liefert jetzt String (kein Event<FormData>)
            on_save: move |val: String| {
                if let Ok(max_refractions) = val.parse::<usize>() {
                    ray_trace_config_sig.write().set_max_number_of_refractions(max_refractions);
                    on_change
                        .call(NodeChangeEvent {
                            node_id,
                            action: NodeChangeAction::AnalyzerType(
                                AnalyzerType::RayTrace(*ray_trace_config_sig.read()),
                            ),
                        });
                }
            },
        }
        FlushableTextInput {
            id: "rayTraceMaxBounces".to_string(),
            label: "Max bounces".to_string(),
            value: format!("{}", ray_trace_config_sig.read().max_number_of_bounces()),
            r#type: "number",
            step: "1",
            min: "0",
            on_save: move |val: String| {
                if let Ok(max_bounces) = val.parse::<usize>() {
                    ray_trace_config_sig.write().set_max_number_of_bounces(max_bounces);
                    on_change
                        .call(NodeChangeEvent {
                            node_id,
                            action: NodeChangeAction::AnalyzerType(
                                AnalyzerType::RayTrace(*ray_trace_config_sig.read()),
                            ),
                        });
                }
            },
        }
        FlushableTextInput {
            id: "rayTraceMinEnergy".to_string(),
            label: "Minimum ray energy in pJ".to_string(),
            value: format!("{}", ray_trace_config_sig.read().min_energy_per_ray().get::<picojoule>()),
            r#type: "number",
            step: "1.",
            min: "0.",
            on_save: move |val: String| {
                let old_value = ray_trace_config_sig.read().min_energy_per_ray();
                if let Ok(min_ray_energy) = val.parse::<f64>() {
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
                        let update_result = ray_trace_config_sig
                            .write()
                            .set_min_energy_per_ray(picojoule!(min_ray_energy));

                        match update_result {
                            Ok(()) => {
                                on_change
                                    .call(NodeChangeEvent {
                                        node_id,
                                        action: NodeChangeAction::AnalyzerType(
                                            AnalyzerType::RayTrace(*ray_trace_config_sig.read()),
                                        ),
                                    });
                            }
                            Err(err) => OPOSSUM_UI_LOGS.write().add_log(&err.to_string()),
                        }
                    }
                }
            },
        }
        // Selects brauchen kein Flushable, da sie sofort feuern
        LabeledSelect {
            id: "rayTraceMissedSurf".to_string(),
            label: "Missed-Surface Strategy".to_string(),
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
                    on_change
                        .call(NodeChangeEvent {
                            node_id,
                            action: NodeChangeAction::AnalyzerType(
                                AnalyzerType::RayTrace(*ray_trace_config_sig.read()),
                            ),
                        });
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
    use_update_signal_with_reactive_prop(ghost_focus_config, ghost_focus_config_sig);

    rsx! {
        FlushableTextInput {
            id: "ghostFocusMaxBounces".to_string(),
            label: "Max Bounces".to_string(),
            value: format!("{}", ghost_focus_config_sig.read().max_bounces()),
            r#type: "number",
            step: "1",
            min: "0",
            on_save: move |val: String| {
                if let Ok(max_bounces) = val.parse::<usize>() {
                    ghost_focus_config_sig.write().set_max_bounces(max_bounces);
                    on_change
                        .call(NodeChangeEvent {
                            node_id,
                            action: NodeChangeAction::AnalyzerType(
                                AnalyzerType::GhostFocus(ghost_focus_config_sig.read().clone()),
                            ),
                        });
                }
            },
        }
        LabeledSelect {
            id: "ghostFocusFluence".to_string(),
            label: "Fluence Estimator".to_string(),
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
                    on_change
                        .call(NodeChangeEvent {
                            node_id,
                            action: NodeChangeAction::AnalyzerType(
                                AnalyzerType::GhostFocus(ghost_focus_config_sig.read().clone()),
                            ),
                        });
                }
            },
        }
    }
}
