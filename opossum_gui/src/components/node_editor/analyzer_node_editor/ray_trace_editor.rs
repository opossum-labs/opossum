use crate::OPOSSUM_UI_LOGS;
use crate::components::node_editor::{
    hooks::use_update_signal_with_reactive_prop,
    inputs::{
        input_components::{FlushableTextInput, LabeledSelect},
        select_options_from_enum_iterator,
    },
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};
use dioxus::prelude::*;
use opossum_core::{
    analyzers::raytrace::MissedSurfaceStrategy, picojoule, prelude::*,
    utils::default_from_name::DefaultFromName,
};
use uom::si::energy::picojoule;
use uuid::Uuid;

#[component]
pub fn RayTraceEditor(
    node_id: Uuid,
    ray_trace_config: RayTraceConfig,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut ray_trace_config_sig = use_signal(|| ray_trace_config);
    use_update_signal_with_reactive_prop(ray_trace_config, ray_trace_config_sig);

    rsx! {
        FlushableTextInput {
            id: "rayTraceMaxRefr".to_string(),
            label: "Max refractions".to_string(),
            value: format!("{}", ray_trace_config_sig.read().max_number_of_refractions()),
            r#type: "number",
            step: "1",
            min: "0",
            container_class: "form-floating border-start".to_string(),
            input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
            label_class: "form-label text-secondary".to_string(),
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
            container_class: "form-floating border-start".to_string(),
            input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
            label_class: "form-label text-secondary".to_string(),
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
            container_class: "form-floating border-start".to_string(),
            input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
            label_class: "form-label text-secondary".to_string(),
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
