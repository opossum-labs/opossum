use crate::OPOSSUM_UI_LOGS;
use crate::components::node_editor::{
    inputs::{
        input_components::{FlushableTextInput, LabeledSelect},
        select_options_from_enum_iterator,
    },
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};
// Import the RaySourceEditor and the unified API client module
use crate::components::node_editor::optical_node_editor::properties_editor::light_data_editor::ray_source_editor::RaySourceEditor;
use crate::api;

use dioxus::prelude::*;
use opossum_core::{
    core_optics::hit_map::fluence_estimator::FluenceEstimator,
    prelude::{AnalyzerType, GhostFocusConfig, RayDataSource, LightDataBuilder},
    types::api_types::SourcePortDto,
    utils::default_from_name::DefaultFromName,
};
use uuid::Uuid;

#[component]
pub fn GhostFocusEditor(
    node_id: Memo<Uuid>,
    ghost_focus_config: GhostFocusConfig,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut ghost_focus_config_sig = use_signal(|| ghost_focus_config);
    
    // Signal to store the slim DTOs of all available SourcePorts in the model
    let mut available_sources = use_signal(Vec::<SourcePortDto>::new);

    // Fetch globally available SourcePorts from the backend recursively on mount
    use_future(move || async move {
        if let Ok(sources) = api::get_available_sources().await {
            available_sources.set(sources);
        } else {
            OPOSSUM_UI_LOGS
                .write()
                .add_log("Failed to fetch available source ports from backend.");
        }
    });

    let ghost_focus_config_handler =
        EventHandler::new(move |ghost_focus_config: GhostFocusConfig| {
            on_change.call(NodeChangeEvent {
                node_id: *node_id.read(),
                action: NodeChangeAction::AnalyzerType(AnalyzerType::GhostFocus(
                    ghost_focus_config,
                )),
            });
        });

    let max_bounces_handler = EventHandler::new(move |max_bounces: usize| {
        ghost_focus_config_sig.write().set_max_bounces(max_bounces);
        ghost_focus_config_handler.call((*ghost_focus_config_sig.read()).clone());
    });
    let fluence_estimator_handler =
        EventHandler::new(move |fluence_estimator: FluenceEstimator| {
            ghost_focus_config_sig
                .write()
                .set_fluence_estimator(fluence_estimator);
            ghost_focus_config_handler.call((*ghost_focus_config_sig.read()).clone());
        });

    // CRITICAL LIFETIME FIX: Clone the data outside the rsx tree to release the read guard immediately
    let sources_list = available_sources.read().clone();

    rsx! {
        div { class: "ghost-focus-fields",
            FlushableTextInput {
                id: "ghostFocusMaxBounces".to_string(),
                label: "Max Bounces".to_string(),
                value: format!("{}", ghost_focus_config_sig.read().max_bounces()),
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
                        fluence_estimator_handler.call(fluence_estimator);
                    }
                },
            }

            // --- Section for configuring SourcePort properties ---
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
                                    ghost_focus_config_sig,
                                    ghost_focus_config_handler,
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
    ghost_focus_config_sig: Signal<GhostFocusConfig>,
    ghost_focus_config_handler: EventHandler<GhostFocusConfig>,
) -> Element {
    let mut is_collapsed = use_signal(|| true);
    let port_uuid = port.uuid;
    let port_name = port.name;

    // Safely look up the existing configuration via the mapped source builder inside the core state
    let existing_source = ghost_focus_config_sig.read()
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
                                let mut updated_config = (*ghost_focus_config_sig.read()).clone();
                                updated_config.map_source(port_uuid, updated_builder.into());

                                // Push the entire updated configuration up the standard pipeline
                                ghost_focus_config_sig.set(updated_config.clone());
                                ghost_focus_config_handler.call(updated_config);
                            }
                        },
                    }
                }
            }
        }
    }
}