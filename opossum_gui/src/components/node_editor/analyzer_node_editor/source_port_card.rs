use crate::components::node_editor::accordion::AccordionItem;
use crate::components::node_editor::analyzer_node_editor::light_data_editor::ray_source_editor::RaySourceEditor;
use dioxus::prelude::*;
use opossum_core::{
    light::lightdata::ray_data_builder::RayDataBuilder,
    prelude::{LightDataBuilder, RayDataSource},
    types::api_types::SourcePortDto,
};
use uuid::Uuid;

/// Reusable accordion item for configuring a ray source assigned to an optical source port.
///
/// Uses the same MDB [`AccordionItem`] as the node-config sidebar (e.g. `SinglePortConfigEditor`)
/// so the analyzer's source definitions read as the rest of the app rather than as a bespoke card.
/// Each card is wrapped in its own `accordion` container, so several ports can be expanded
/// independently.
///
/// # Arguments
/// * `analyzer_id` - UUID of the parent analyzer node (used for undo/redo autofocus tracking).
/// * `port` - metadata of the source port being configured.
/// * `source` - the ray data source currently mapped to the port.
/// * `on_save` - emitted when the ray source definition is modified and saved.
/// * `readonly` - disables editing when set.
///
/// # Returns
/// The rendered accordion item element.
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
    let port_uuid = port.uuid;
    let port_name = port.name;

    // Trigger auto-focus and accordion expansion on undo/redo actions
    super::use_source_card_focus(analyzer_id, port_uuid);

    // The accordion body: the ray source editor for this port.
    let body = vec![rsx! {
        RaySourceEditor {
            ray_data_builder: source,
            readonly,
            on_save: move |light_builder| {
                if let LightDataBuilder::Geometric(updated_builder) = light_builder {
                    on_save.call(updated_builder.into());
                }
            },
        }
    }];

    rsx! {
        // Each card is its own accordion group (unique `parent_id`) so ports collapse
        // independently. The wrapper id doubles as the scroll target in `use_source_card_focus`.
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
