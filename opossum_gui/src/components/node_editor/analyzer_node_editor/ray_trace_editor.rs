use crate::OPOSSUM_UI_LOGS;
use crate::components::node_editor::inputs::input_components::{NodeConfigUnitInput, UnitHandling};
use crate::components::node_editor::{
    inputs::{
        input_components::{FlushableTextInput, LabeledSelect},
        select_options_from_enum_iterator,
    },
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};
// Import the RaySourceEditor
use crate::components::node_editor::optical_node_editor::properties_editor::light_data_editor::ray_source_editor::RaySourceEditor;
use crate::api;

use dioxus::prelude::*;
use opossum_core::analyzers::propagation_strategy::MissedSurfaceStrategy;
use opossum_core::types::api_types::SourcePortDto;
use opossum_core::{joule, prelude::*, utils::default_from_name::DefaultFromName};
use uom::si::f64::Energy;
use uuid::Uuid;

#[component]
pub fn RayTraceEditor(
    node_id: Memo<Uuid>,
    ray_trace_config: RayTraceConfig,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut ray_trace_config_sig = use_signal(|| ray_trace_config);
    let mut available_sources = use_signal(Vec::<SourcePortDto>::new);

    // Fetch the globally available SourcePorts from the backend recursively on mount
    use_future(move || async move {
        if let Ok(sources) = api::get_available_sources().await {
            available_sources.set(sources);
        } else {
            OPOSSUM_UI_LOGS
                .write()
                .add_log("Failed to fetch available source ports from backend.");
        }
    });

    let ray_trace_config_handler = EventHandler::new(move |ray_trace_config: RayTraceConfig| {
        on_change.call(NodeChangeEvent {
            node_id: *node_id.read(),
            action: NodeChangeAction::AnalyzerType(AnalyzerType::RayTrace(ray_trace_config)),
        });
    });

    let max_refractions_handler = EventHandler::new(move |max_refractions: usize| {
        ray_trace_config_sig
            .write()
            .set_max_number_of_refractions(max_refractions);
        ray_trace_config_handler.call((*ray_trace_config_sig.read()).clone());
    });
    let max_bounces_handler = EventHandler::new(move |max_bounces: usize| {
        ray_trace_config_sig
            .write()
            .set_max_number_of_bounces(max_bounces);
        ray_trace_config_handler.call((*ray_trace_config_sig.read()).clone());
    });
    let min_ray_energy_handler = EventHandler::new(move |min_ray_energy: Energy| {
        if ray_trace_config_sig
            .write()
            .set_min_energy_per_ray(min_ray_energy)
            .is_ok()
        {
            ray_trace_config_handler.call((*ray_trace_config_sig.read()).clone());
        }
    });
    let missed_surface_strategy_handler =
        EventHandler::new(move |missed_surface_strategy: MissedSurfaceStrategy| {
            ray_trace_config_sig
                .write()
                .set_missed_surface_strategy(missed_surface_strategy);
            ray_trace_config_handler.call((*ray_trace_config_sig.read()).clone());
        });

    let sources_list = available_sources.read().clone();

    rsx! {
        div { class: "ray-trace-fields",
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
                        max_refractions_handler.call(max_refractions);
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
                        max_bounces_handler.call(max_bounces);
                    }
                },
            }
            NodeConfigUnitInput {
                id: "rayTraceMinEnergy".to_string(),
                label: "Minimum ray energy".to_string(),
                value: ray_trace_config_sig.read().min_energy_per_ray().value,
                unit_config: UnitHandling::new("J", true),
                onchange: move |val: f64| {
                    if val >= 0.0 {
                        min_ray_energy_handler.call(joule!(val));
                    } else {
                        OPOSSUM_UI_LOGS.write().add_log("Minimum ray energy must be non-negative.");
                    }
                },
            }

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
                        missed_surface_strategy_handler.call(surface_strategy);
                    }
                },
            }

            // --- Sektion für das Zuordnen von SourcePort Eigenschaften ---
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
                                    ray_trace_config_sig,
                                    ray_trace_config_handler,
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
    ray_trace_config_sig: Signal<RayTraceConfig>,
    ray_trace_config_handler: EventHandler<RayTraceConfig>,
) -> Element {
    let mut is_collapsed = use_signal(|| true);
    let port_uuid = port.uuid;
    let port_name = port.name;

    let existing_source = ray_trace_config_sig.read()
        .get_source(&port_uuid)
        .map(|builder| builder.source().clone())
        .unwrap_or_else(|| RayDataSource::default());

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
                div { class: "card-body p-2 bg-dark text-light",
                    RaySourceEditor {
                        ray_data_builder: existing_source,
                        readonly: false,
                        on_save: move |light_builder| {
                            // Extract the concrete RayDataBuilder from the generic LightDataBuilder enum
                            if let LightDataBuilder::Geometric(updated_builder) = light_builder {
                                let mut updated_config = (*ray_trace_config_sig.read()).clone();
                                updated_config.map_source(port_uuid, updated_builder.into());

                                // Push the entire updated configuration up the standard pipeline
                                ray_trace_config_sig.set(updated_config.clone());
                                ray_trace_config_handler.call(updated_config);
                            }
                        },
                    }
                }
            }
        }
    }
}