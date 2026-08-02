use crate::{
    OPOSSUM_UI_LOGS, api,
    components::node_editor::{
        analyzer_node_editor::light_data_editor::energy_source_editor::EnergySourceEditor,
        node_config_editor::{NodeChangeAction, NodeChangeEvent},
    },
};

use dioxus::prelude::*;
use opossum_core::{
    analyzers::energy::EnergyConfig,
    prelude::{AnalyzerType, EnergyDataBuilder, LightDataBuilder},
    types::api_types::SourcePortDto,
};
use uuid::Uuid;

#[component]
pub fn EnergyEditor(
    node_id: Uuid,
    energy_config: EnergyConfig,
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

    let sources_list = available_sources.read().clone();

    rsx! {
      div { class: "energy-analyzer-fields",
        div { class: "mt-2 text-light",
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
                          energy_config: energy_config.clone(),
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
    energy_config: EnergyConfig,
    on_change: EventHandler<NodeChangeEvent>,
    analyzer_id: Uuid, // Cleaned up: Only analyzer_id needed
) -> Element {
    let mut is_collapsed = use_signal(|| true);
    let port_uuid = port.uuid;
    let port_name = port.name;
    // Undo/redo of this source's mapping expands and scrolls to this card - see `use_source_card_focus`.
    super::use_source_card_focus(analyzer_id, port_uuid, is_collapsed);

    let existing_source = energy_config
        .get_source(&port_uuid)
        .cloned()
        .unwrap_or_else(EnergyDataBuilder::default);

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

            EnergySourceEditor {
              energy_data_builder: existing_source,
              readonly: false,
              on_save: move |light_builder| {
                  if let LightDataBuilder::Energy(updated_builder) = light_builder {
                      let mut updated_config = energy_config.clone();
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
