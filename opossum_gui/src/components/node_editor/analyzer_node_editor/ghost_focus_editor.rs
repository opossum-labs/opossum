use crate::components::node_editor::{
    analyzer_node_editor::light_data_editor::ray_source_editor::RaySourceEditor,
    inputs::{
        input_components::{FlushableTextInput, LabeledSelect},
        select_options_from_enum_iterator,
    },
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};

use dioxus::prelude::*;
use opossum_core::{
    core_optics::hit_map::fluence_estimator::FluenceEstimator,
    prelude::{AnalyzerType, GhostFocusConfig, LightDataBuilder, RayDataSource},
    types::api_types::SourcePortDto,
    utils::default_from_name::DefaultFromName,
};
use uuid::Uuid;

#[component]
pub fn GhostFocusEditor(
    node_id: Uuid,
    ghost_focus_config: GhostFocusConfig,
    on_change: EventHandler<NodeChangeEvent>,
    available_sources: Vec<SourcePortDto>,
) -> Element {
    let on_save_max_bounces = {
        let config = ghost_focus_config.clone();
        move |val: String| {
            if let Ok(max_bounces) = val.parse::<usize>() {
                let mut local_config = config.clone();
                local_config.set_max_bounces(max_bounces);
                on_change.call(NodeChangeEvent {
                    node_id,
                    action: NodeChangeAction::AnalyzerType(AnalyzerType::GhostFocus(local_config)),
                });
            }
        }
    };

    let on_change_fluence = {
        let config = ghost_focus_config.clone();
        move |e: Event<FormData>| {
            let val = e.value();
            if let Some(fluence_estimator) = FluenceEstimator::default_from_name(val.as_str()) {
                let mut local_config = config.clone();
                local_config.set_fluence_estimator(fluence_estimator);
                on_change.call(NodeChangeEvent {
                    node_id,
                    action: NodeChangeAction::AnalyzerType(AnalyzerType::GhostFocus(local_config)),
                });
            }
        }
    };

    let sources_list = available_sources;

    rsx! {
        div { class: "ghost-focus-fields",
            FlushableTextInput {
                id: "ghostFocusMaxBounces".to_string(),
                label: "Max Bounces".to_string(),
                value: format!("{}", ghost_focus_config.max_bounces()),
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
                options: select_options_from_enum_iterator(ghost_focus_config.fluence_estimator(), None),
                onchange: on_change_fluence,
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
                                    ghost_focus_config: ghost_focus_config.clone(),
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
    ghost_focus_config: GhostFocusConfig,
    on_change: EventHandler<NodeChangeEvent>,
    analyzer_id: Uuid, // Cleaned up: Only analyzer_id needed
) -> Element {
    let mut is_collapsed = use_signal(|| true);
    let port_uuid = port.uuid;
    let port_name = port.name;
    // Undo/redo of this source's mapping expands and scrolls to this card - see `use_source_card_focus`.
    super::use_source_card_focus(analyzer_id, port_uuid, is_collapsed);

    let existing_source = ghost_focus_config
        .get_source(&port_uuid)
        .map_or_else(RayDataSource::default, |builder| builder.source().clone());

    rsx! {
        div { class: "card bg-dark border-secondary mb-2", id: "sourceCard{port_uuid}",
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
                                let mut updated_config = ghost_focus_config.clone();
                                updated_config.map_source(port_uuid, updated_builder.into());

                                on_change
                                    .call(NodeChangeEvent {
                                        node_id: analyzer_id,
                                        action: NodeChangeAction::AnalyzerType(
                                            AnalyzerType::GhostFocus(updated_config),
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
