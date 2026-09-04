use crate::components::node_editor::{
    accordion::{AccordionItem, StaticSection},
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
        StaticSection { header: "Sources Definitions",
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

#[component]
fn SourcePortCard(
    port: SourcePortDto,
    energy_config: ReadSignal<EnergyConfig>,
    on_change: EventHandler<NodeChangeEvent>,
    analyzer_id: Uuid,
) -> Element {
    let port_uuid = port.uuid;
    let port_name = port.name;

    // Trigger auto-focus and accordion expansion on undo/redo actions
    super::use_source_card_focus(analyzer_id, port_uuid);

    let existing_source = energy_config
        .read()
        .get_source(&port_uuid)
        .cloned()
        .unwrap_or_else(|| {
            let default_wvl = crate::APP_CONFIG.read().default_wavelength();
            default_energy_data_builder(default_wvl)
        });

    // The accordion body: the energy source editor for this port.
    let body = vec![rsx! {
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
                            action: NodeChangeAction::AnalyzerType(AnalyzerType::Energy(updated_config)),
                        });
                }
            },
        }
    }];

    rsx! {
        // Same MDB accordion as the ray-trace/ghost-focus source cards, so every analyzer's
        // source definitions read alike. Each card is its own accordion group (unique `parent_id`).
        div {
            class: "accordion accordion-borderless bg-dark border-start mb-2",
            id: "sourceCard{port_uuid}",
            AccordionItem {
                elements: body,
                header: port_name,
                header_id: "sourceHeading{port_uuid}",
                parent_id: "sourceCard{port_uuid}",
                content_id: "sourceCollapse{port_uuid}",
                level: 2,
            }
        }
    }
}
