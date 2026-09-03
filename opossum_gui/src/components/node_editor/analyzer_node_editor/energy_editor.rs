use crate::components::node_editor::{
    analyzer_node_editor::light_data_editor::{
        default_energy_data_builder, energy_source_editor::EnergySourceEditor,
    },
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
};

use dioxus::prelude::*;
use opossum_core::{
    analyzers::energy::EnergyConfig,
    prelude::{AnalyzerType, LightDataBuilder},
    types::api_types::SourcePortDto,
};
use uuid::Uuid;

#[component]
pub fn EnergyEditor(
    node_id: Uuid,
    energy_config: ReadSignal<EnergyConfig>,
    on_change: EventHandler<NodeChangeEvent>,
    available_sources: Vec<SourcePortDto>,
) -> Element {
    info!("🔄 Render: EnergyEditor");

    rsx! {
        div { class: "energy-analyzer-fields",
            div { class: "mt-2 text-light",
                h6 { class: "text-secondary mb-3", "Sources Definitions" }

                if available_sources.is_empty() {
                    div { class: "text-muted small italic", "No Source Ports found." }
                }

                {
                    available_sources
                        .into_iter()
                        .map(|port| {
                            rsx! {
                                SourcePortCard {
                                    key: "{port.uuid}",
                                    port,
                                    energy_config,
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
    energy_config: ReadSignal<EnergyConfig>,
    on_change: EventHandler<NodeChangeEvent>,
    analyzer_id: Uuid,
) -> Element {
    let mut is_collapsed = use_signal(|| true);
    let port_uuid = port.uuid;
    let port_name = port.name;

    // Trigger auto-focus and accordion expansion on undo/redo actions
    super::use_source_card_focus(analyzer_id, port_uuid, is_collapsed);

    let existing_source = energy_config
    .read()
    .get_source(&port_uuid)
    .cloned()
    .unwrap_or_else(|| {
        let default_wvl = crate::APP_CONFIG.read().default_wavelength();
        default_energy_data_builder(default_wvl)
    });

    rsx! {
        div {
            class: "card bg-dark border-secondary mb-2",
            id: "sourceCard{port_uuid}",
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

                    EnergySourceEditor {
                        energy_data_builder: existing_source,
                        readonly: false,
                        on_save: move |light_builder| {
                            if let LightDataBuilder::Energy(updated_builder) = light_builder {
                                let mut updated_config = energy_config.peek().clone();
                                updated_config.map_source(port_uuid, updated_builder);

                                on_change
                                    .call(NodeChangeEvent {
                                        node_id: analyzer_id,
                                        action: NodeChangeAction::AnalyzerType(
                                            AnalyzerType::Energy(updated_config),
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
