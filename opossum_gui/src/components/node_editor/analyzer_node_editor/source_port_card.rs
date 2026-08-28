use crate::components::node_editor::analyzer_node_editor::light_data_editor::ray_source_editor::RaySourceEditor;
use dioxus::prelude::*;
use opossum_core::{
    light::lightdata::ray_data_builder::RayDataBuilder,
    prelude::{LightDataBuilder, RayDataSource},
    types::api_types::SourcePortDto,
};
use uuid::Uuid;

/// Reusable card component for configuring a ray source assigned to an optical source port.
#[component]
pub fn SourcePortCard(
    /// UUID of the parent analyzer node (used for autofocus scroll tracking).
    analyzer_id: Uuid,
    /// Metadata of the source port being configured.
    port: SourcePortDto,
    /// Currently mapped ray data source.
    source: RayDataSource,
    /// Event emitted when the ray source definition is modified and saved.
    on_save: EventHandler<RayDataBuilder>,
    /// Flag to disable editing.
    #[props(default = false)]
    readonly: bool,
) -> Element {
    let mut is_collapsed = use_signal(|| true);
    let port_uuid = port.uuid;
    let port_name = port.name;

    // Trigger auto-focus and accordion expansion on undo/redo actions
    super::use_source_card_focus(analyzer_id, port_uuid, is_collapsed);

    rsx! {
      div {
        class: "card bg-dark border-secondary mb-2",
        id: "sourceCard{port_uuid}",

        // Accordion Header
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

        // Accordion Body containing the source editor
        if !is_collapsed() {
          div {
            key: "{analyzer_id}-{port_uuid}",
            class: "card-body p-2 bg-dark text-light",

            RaySourceEditor {
              ray_data_builder: source,
              readonly,
              on_save: move |light_builder| {
                  if let LightDataBuilder::Geometric(updated_builder) = light_builder {
                      on_save.call(updated_builder.into());
                  }
              },
            }
          }
        }
      }
    }
}
