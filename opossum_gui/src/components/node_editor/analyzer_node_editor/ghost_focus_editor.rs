use crate::components::{
    inputs::material_selector::MaterialSelector,
    node_editor::{
        analyzer_node_editor::source_port_card::SourcePortCard,
        inputs::{
            input_components::{FlushableTextInput, LabeledSelect},
            select_options_from_enum_iterator,
        },
        node_config_editor::{NodeChangeAction, NodeChangeEvent},
    },
};
use dioxus::prelude::*;
use opossum_core::{
    core_optics::hit_map::fluence_estimator::FluenceEstimator,
    material::Material,
    prelude::{AnalyzerType, GhostFocusConfig, RayDataSource},
    types::api_types::SourcePortDto,
    utils::default_from_name::DefaultFromName,
};
use uuid::Uuid;

#[component]
pub fn GhostFocusEditor(
    node_id: Uuid,
    ghost_focus_config: ReadSignal<GhostFocusConfig>,
    on_change: EventHandler<NodeChangeEvent>,
    available_sources: Vec<SourcePortDto>,
) -> Element {
    info!("🔄 Render: GhostFocusEditor");

    // Stable callback reading directly from the reactive ReadSignal handle
    let on_save_max_bounces = use_callback(move |val: String| {
        if let Ok(max_bounces) = val.parse::<usize>() {
            let mut local_config = ghost_focus_config.peek().clone();
            local_config.set_max_bounces(max_bounces);
            on_change.call(NodeChangeEvent {
                node_id,
                action: NodeChangeAction::AnalyzerType(AnalyzerType::GhostFocus(local_config)),
            });
        }
    });

    // Stable callback reading directly from the reactive ReadSignal handle
    let on_change_fluence = use_callback(move |e: Event<FormData>| {
        let val = e.value();
        if let Some(fluence_estimator) = FluenceEstimator::default_from_name(val.as_str()) {
            let mut local_config = ghost_focus_config.peek().clone();
            local_config.set_fluence_estimator(fluence_estimator);
            on_change.call(NodeChangeEvent {
                node_id,
                action: NodeChangeAction::AnalyzerType(AnalyzerType::GhostFocus(local_config)),
            });
        }
    });

    // Callback for updating the ambient medium material
    let on_change_ambient_material = use_callback(move |updated_material: Material| {
        let mut local_config = ghost_focus_config.peek().clone();
        local_config.set_ambient_material(updated_material);
        on_change.call(NodeChangeEvent {
            node_id,
            action: NodeChangeAction::AnalyzerType(AnalyzerType::GhostFocus(local_config)),
        });
    });

    let current_config = ghost_focus_config.read();

    rsx! {
        div { class: "ghost-focus-fields",
            FlushableTextInput {
                id: "ghostFocusMaxBounces".to_string(),
                label: "Max Bounces".to_string(),
                value: format!("{}", current_config.max_bounces()),
                r#type: "number",
                step: "1",
                min: "0",
                container_class: "form-floating border-start".to_string(),
                input_class: "form-control bg-dark text-light form-control-sm noselect".to_string(),
                label_class: "form-label text-secondary".to_string(),
                on_save: on_save_max_bounces,
            }
            LabeledSelect {
                id: "ghostFocusFluence".to_string(),
                label: "Fluence Estimator".to_string(),
                options: select_options_from_enum_iterator(current_config.fluence_estimator(), None),
                onchange: on_change_fluence,
            }

            // Material selector for the ambient medium
            MaterialSelector {
                label: "Ambient Material".to_string(),
                material: current_config.ambient_material().clone(),
                readonly: false,
                on_change: on_change_ambient_material,
            }

            div { class: "mt-4 border-top pt-3 text-light",
                h6 { class: "text-secondary mb-3", "Sources Definitions" }

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
                                    RayDataSource::default,
                                    |builder| builder.source().clone(),
                                );
                            let on_save_source = move |updated_builder| {
                                let mut updated_config = ghost_focus_config.peek().clone();
                                updated_config.map_source(port_uuid, updated_builder);
                                on_change
                                    .call(NodeChangeEvent {
                                        node_id,
                                        action: NodeChangeAction::AnalyzerType(
                                            AnalyzerType::GhostFocus(updated_config),
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
}
