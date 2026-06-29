use crate::OPOSSUM_UI_LOGS;
use crate::api;
use crate::components::node_editor::{
    analyzer_node_editor::light_data_editor::ray_source_editor::RaySourceEditor,
    inputs::{
        input_components::{FlushableTextInput, LabeledSelect, NodeConfigUnitInput, UnitHandling},
        select_options_from_enum_iterator,
    },
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};

use dioxus::prelude::*;
use opossum_core::{
    analyzers::propagation_strategy::MissedSurfaceStrategy, joule, prelude::*,
    types::api_types::SourcePortDto, utils::default_from_name::DefaultFromName,
};
use uuid::Uuid;

#[component]
pub fn RayTraceEditor(
    node_id: Uuid,
    ray_trace_config: RayTraceConfig,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut available_sources = use_signal(Vec::<SourcePortDto>::new);

    use_future(move || async move {
        if let Ok(sources) = api::get_available_sources().await {
            available_sources.set(sources);
        } else {
            OPOSSUM_UI_LOGS
                .write()
                .add_log("Failed to fetch available source ports from backend.");
        }
    });

    let on_save_max_refractions = {
        let config = ray_trace_config.clone();
        move |val: String| {
            if let Ok(max_refractions) = val.parse::<usize>() {
                let mut local_config = config.clone();
                local_config.set_max_number_of_refractions(max_refractions);
                on_change.call(NodeChangeEvent {
                    node_id,
                    action: NodeChangeAction::AnalyzerType(AnalyzerType::RayTrace(local_config)),
                });
            }
        }
    };

    let on_save_max_bounces = {
        let config = ray_trace_config.clone();
        move |val: String| {
            if let Ok(max_bounces) = val.parse::<usize>() {
                let mut local_config = config.clone();
                local_config.set_max_number_of_bounces(max_bounces);
                on_change.call(NodeChangeEvent {
                    node_id,
                    action: NodeChangeAction::AnalyzerType(AnalyzerType::RayTrace(local_config)),
                });
            }
        }
    };

    let on_change_min_energy = {
        let config = ray_trace_config.clone();
        move |val: f64| {
            if val >= 0.0 {
                let mut local_config = config.clone();
                if local_config.set_min_energy_per_ray(joule!(val)).is_ok() {
                    on_change.call(NodeChangeEvent {
                        node_id,
                        action: NodeChangeAction::AnalyzerType(AnalyzerType::RayTrace(
                            local_config,
                        )),
                    });
                }
            } else {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log("Minimum ray energy must be non-negative.");
            }
        }
    };

    let on_change_missed_strategy = {
        let config = ray_trace_config.clone();
        move |e: Event<FormData>| {
            let val = e.value();
            if let Some(surface_strategy) = MissedSurfaceStrategy::default_from_name(val.as_str()) {
                let mut local_config = config.clone();
                local_config.set_missed_surface_strategy(surface_strategy);
                on_change.call(NodeChangeEvent {
                    node_id,
                    action: NodeChangeAction::AnalyzerType(AnalyzerType::RayTrace(local_config)),
                });
            }
        }
    };

    let sources_list = available_sources.read().clone();

    rsx! {
        div { class: "ray-trace-fields",
            FlushableTextInput {
                id: "rayTraceMaxRefr".to_string(),
                label: "Max refractions".to_string(),
                value: format!("{}", ray_trace_config.max_number_of_refractions()),
                r#type: "number",
                step: "1",
                min: "0",
                container_class: "form-floating border-start".to_string(),
                input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
                label_class: "form-label text-secondary".to_string(),
                on_save: on_save_max_refractions,
            }
            FlushableTextInput {
                id: "rayTraceMaxBounces".to_string(),
                label: "Max bounces".to_string(),
                value: format!("{}", ray_trace_config.max_number_of_bounces()),
                r#type: "number",
                step: "1",
                min: "0",
                container_class: "form-floating border-start".to_string(),
                input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
                label_class: "form-label text-secondary".to_string(),
                on_save: on_save_max_bounces,
            }
            NodeConfigUnitInput {
                id: "rayTraceMinEnergy".to_string(),
                label: "Minimum ray energy".to_string(),
                value: ray_trace_config.min_energy_per_ray().value,
                unit_config: UnitHandling::new("J", true),
                onchange: on_change_min_energy,
            }

            LabeledSelect {
                id: "rayTraceMissedSurf".to_string(),
                label: "Missed-Surface Strategy".to_string(),
                options: select_options_from_enum_iterator(ray_trace_config.missed_surface_strategy(), None),
                onchange: on_change_missed_strategy,
            }

            div { class: "mt-4 border-top pt-3 text-light",
                h6 { class: "text-secondary mb-3", "Sources Definitions" }

                if sources_list.is_empty() {
                    div { class: "text-muted small italic", "No Source Ports found." }
                }

                {
                    sources_list
                        .into_iter()
                        .map(|port| {
                            rsx! {
                                SourcePortCard {
                                    key: "{port.uuid}",
                                    port,
                                    ray_trace_config: ray_trace_config.clone(),
                                    on_change,
                                    analyzer_id: node_id,
                                }
                            }
                        })
                }
            }
        }
    }
}

#[component]
fn SourcePortCard(
    port: SourcePortDto,
    ray_trace_config: RayTraceConfig,
    on_change: EventHandler<NodeChangeEvent>,
    analyzer_id: Uuid, // Cleaned up: Only analyzer_id needed
) -> Element {
    let mut is_collapsed = use_signal(|| true);
    let port_uuid = port.uuid;
    let port_name = port.name;

    let existing_source = ray_trace_config
        .get_source(&port_uuid)
        .map_or_else(RayDataSource::default, |builder| builder.source().clone());

    rsx! {
        div { class: "card bg-dark border-secondary mb-2",
            div {
                class: "card-header bg-secondary py-1 px-2 text-light d-flex justify-content-between align-items-center noselect",
                style: "cursor: pointer;",
                onclick: move |_| is_collapsed.toggle(),

                span { class: "fw-bold small", "{port_name}" }
                span { class: "text-muted small",
                    if is_collapsed() {
                        "▶"
                    } else {
                        "▼"
                    }
                }
            }

            if !is_collapsed() {
                div {
                    key: "{analyzer_id}",
                    class: "card-body p-2 bg-dark text-light",

                    RaySourceEditor {
                        ray_data_builder: existing_source,
                        readonly: false,
                        on_save: move |light_builder| {
                            if let LightDataBuilder::Geometric(updated_builder) = light_builder {
                                let mut updated_config = ray_trace_config.clone();
                                updated_config.map_source(port_uuid, updated_builder.into());

                                on_change
                                    .call(NodeChangeEvent {
                                        node_id: analyzer_id,
                                        action: NodeChangeAction::AnalyzerType(
                                            AnalyzerType::RayTrace(updated_config),
                                        ),
                                    });
                            }
                        },
                    }
                }
            }
        }
    }
}
