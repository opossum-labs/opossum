use crate::OPOSSUM_UI_LOGS;
use crate::components::node_editor::analyzer_node_editor::light_data_editor::default_ray_data_source;
use crate::components::{
    inputs::material_selector::MaterialSelector,
    node_editor::{
        accordion::StaticSection,
        analyzer_node_editor::source_port_card::SourcePortCard,
        inputs::{
            input_components::{
                FlushableTextInput, LabeledSelect, NodeConfigUnitInput, UnitHandling,
            },
            select_options_from_enum_iterator,
        },
        node_config_editor::{NodeChangeAction, NodeChangeEvent},
    },
};
use dioxus::prelude::*;
use opossum_core::{
    analyzers::propagation_strategy::MissedSurfaceStrategy, joule, material::Material, prelude::*,
    types::api_types::SourcePortDto, utils::default_from_name::DefaultFromName,
};
use uuid::Uuid;

#[component]
pub fn RayTraceEditor(
    node_id: Uuid,
    ray_trace_config: ReadSignal<RayTraceConfig>,
    on_change: EventHandler<NodeChangeEvent>,
    available_sources: Vec<SourcePortDto>,
) -> Element {
    info!("🔄 Render: RayTraceEditor");

    // Stable callback for updating max refractions via reactive signal handle
    let on_save_max_refractions = use_callback(move |val: String| {
        if let Ok(max_refractions) = val.parse::<usize>() {
            let mut local_config = ray_trace_config.peek().clone();
            local_config.set_max_number_of_refractions(max_refractions);
            on_change.call(NodeChangeEvent {
                node_id,
                action: NodeChangeAction::AnalyzerType(AnalyzerType::RayTrace(local_config)),
            });
        }
    });

    // Stable callback for updating max bounces via reactive signal handle
    let on_save_max_bounces = use_callback(move |val: String| {
        if let Ok(max_bounces) = val.parse::<usize>() {
            let mut local_config = ray_trace_config.peek().clone();
            local_config.set_max_number_of_bounces(max_bounces);
            on_change.call(NodeChangeEvent {
                node_id,
                action: NodeChangeAction::AnalyzerType(AnalyzerType::RayTrace(local_config)),
            });
        }
    });

    // Stable callback for updating minimum ray energy via reactive signal handle
    let on_change_min_energy = use_callback(move |val: f64| {
        if val >= 0.0 {
            let mut local_config = ray_trace_config.peek().clone();
            if local_config.set_min_energy_per_ray(joule!(val)).is_ok() {
                on_change.call(NodeChangeEvent {
                    node_id,
                    action: NodeChangeAction::AnalyzerType(AnalyzerType::RayTrace(local_config)),
                });
            }
        } else {
            OPOSSUM_UI_LOGS
                .write()
                .add_log("Minimum ray energy must be non-negative.");
        }
    });

    // Stable callback for updating missed-surface strategy via reactive signal handle
    let on_change_missed_strategy = use_callback(move |e: Event<FormData>| {
        let val = e.value();
        if let Some(surface_strategy) = MissedSurfaceStrategy::default_from_name(val.as_str()) {
            let mut local_config = ray_trace_config.peek().clone();
            local_config.set_missed_surface_strategy(surface_strategy);
            on_change.call(NodeChangeEvent {
                node_id,
                action: NodeChangeAction::AnalyzerType(AnalyzerType::RayTrace(local_config)),
            });
        }
    });

    // Callback for updating the ambient medium material
    let on_change_ambient_material = use_callback(move |updated_material: Material| {
        let mut local_config = ray_trace_config.peek().clone();
        local_config.set_ambient_material(updated_material);
        on_change.call(NodeChangeEvent {
            node_id,
            action: NodeChangeAction::AnalyzerType(AnalyzerType::RayTrace(local_config)),
        });
    });

    let current_config = ray_trace_config.read();

    rsx! {
        StaticSection { header: "Ray Tracing",
            FlushableTextInput {
                id: "rayTraceMaxRefr".to_string(),
                label: "Max refractions".to_string(),
                value: format!("{}", current_config.max_number_of_refractions()),
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
                value: format!("{}", current_config.max_number_of_bounces()),
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
                value: current_config.min_energy_per_ray().value,
                unit_config: UnitHandling::new("J", true),
                onchange: on_change_min_energy,
            }

            LabeledSelect {
                id: "rayTraceMissedSurf".to_string(),
                label: "Missed-Surface Strategy".to_string(),
                options: select_options_from_enum_iterator(current_config.missed_surface_strategy(), None),
                onchange: on_change_missed_strategy,
            }

            // Material selector for the ambient medium
            MaterialSelector {
                label: "Ambient Material".to_string(),
                material: current_config.ambient_material().clone(),
                readonly: false,
                on_change: on_change_ambient_material,
            }
        }

        StaticSection { header: "Sources Definitions",
            if available_sources.is_empty() {
                div { class: "text-muted small italic", "No Source Ports found." }
            }

            {
                available_sources
                    .into_iter()
                    .map(|port| {
                        let port_uuid = port.uuid;
                        let source = current_config
                            .get_source(&port_uuid)
                            .map_or_else(
                                || {
                                    let default_wvl = crate::APP_CONFIG.read().default_wavelength();
                                    default_ray_data_source(default_wvl)
                                },
                                |b| b.source().clone(),
                            );
                        let on_save_source = move |updated_builder| {
                            let mut updated_config = ray_trace_config.peek().clone();
                            updated_config.map_source(port_uuid, updated_builder);
                            on_change
                                .call(NodeChangeEvent {
                                    node_id,
                                    action: NodeChangeAction::AnalyzerType(
                                        AnalyzerType::RayTrace(updated_config),
                                    ),
                                });
                        };
                        rsx! {
                            SourcePortCard {
                                key: "{port_uuid}",
                                analyzer_id: node_id,
                                port,
                                source,
                                on_save: on_save_source,
                            }
                        }
                    })
            }
        }
    }
}
