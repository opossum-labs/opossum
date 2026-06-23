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
    node_id: Memo<Uuid>,
    energy_config: EnergyConfig,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let energy_config_sig = use_signal(|| energy_config);

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

    let energy_config_handler = EventHandler::new(move |energy_config: EnergyConfig| {
        on_change.call(NodeChangeEvent {
            node_id: *node_id.read(),
            action: NodeChangeAction::AnalyzerType(AnalyzerType::Energy(energy_config)),
        });
    });

    // CRITICAL LIFETIME FIX: Clone the data outside the rsx tree to release the read guard immediately
    let sources_list = available_sources.read().clone();

    rsx! {
      div { class: "energy-analyzer-fields",
        // --- Section for configuring SourcePort properties ---
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
                          energy_config_sig,
                          energy_config_handler,
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
    energy_config_sig: Signal<EnergyConfig>,
    energy_config_handler: EventHandler<EnergyConfig>,
) -> Element {
    let mut is_collapsed = use_signal(|| true);
    let port_uuid = port.uuid;
    let port_name = port.name;

    // Safely look up the existing configuration via the mapped energy builder inside the core state
    let existing_source = energy_config_sig
        .read()
        .get_source(&port_uuid)
        .cloned()
        .unwrap_or_else(EnergyDataBuilder::default);

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
            EnergySourceEditor {
              energy_data_builder: existing_source,
              readonly: false,
              on_save: move |light_builder| {
                  // Extract the concrete EnergyDataBuilder from the generic LightDataBuilder enum
                  if let LightDataBuilder::Energy(updated_builder) = light_builder {
                      let mut updated_config = (*energy_config_sig.read()).clone();
                      updated_config.map_source(port_uuid, updated_builder);

                      // Push the entire updated configuration up the standard pipeline
                      energy_config_sig.set(updated_config.clone());
                      energy_config_handler.call(updated_config);
                  }
              },
            }
          }
        }
      }
    }
}
