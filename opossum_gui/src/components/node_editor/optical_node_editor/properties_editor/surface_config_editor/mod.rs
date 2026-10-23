pub(super) mod coating_editor;

use dioxus::prelude::*;
use opossum_core::{
    coatings::CoatingType, core_optics::SurfaceConfig, nodes::fluence_detector::Fluence,
};
use uom::si::radiant_exposure::joule_per_square_centimeter;
use uuid::Uuid;

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        accordion::AccordionItem,
        hooks::use_synced_signal,
        inputs::input_components::{NodeConfigUnitInput, UnitHandling},
        node_config_editor::NodeChangeEvent,
        optical_node_editor::properties_editor::{
            on_save_proptype_handler, surface_config_editor::coating_editor::CoatingEditor,
        },
    },
    utils::ToSentenceCase,
};

/// Edits the coating and the LIDT of one surface of a component, the property named after its
/// face.
#[component]
pub fn SurfaceConfigEditor(
    node_id: ReadSignal<Uuid>,
    surface_config: SurfaceConfig,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let surface_config_sig = use_synced_signal(surface_config);
    let on_save =
        on_save_proptype_handler(surface_config_sig, property_key.clone(), on_change, node_id);
    let config = surface_config_sig.read().clone();
    let lidt = config.lidt();
    let coating = config.coating().clone();
    let accordion_content = vec![rsx! {
        CoatingEditor {
            coating_type: config.coating().clone(),
            on_change: move |coating: CoatingType| {
                match SurfaceConfig::new(coating, lidt) {
                    Ok(config) => on_save.call(config),
                    Err(err) => OPOSSUM_UI_LOGS.write().add_log(&format!("Invalid coating: {err}")),
                }
            },
            readonly,
        }
        NodeConfigUnitInput {
            id: format!("{}{property_key}Lidt", node_id.read()),
            label: "LIDT",
            value: lidt.get::<joule_per_square_centimeter>(),
            unit_config: UnitHandling::new("J/cm²", true),
            onchange: move |value: f64| {
                match SurfaceConfig::new(coating.clone(), Fluence::new::<joule_per_square_centimeter>(value)) {
                    Ok(config) => on_save.call(config),
                    Err(err) => OPOSSUM_UI_LOGS.write().add_log(&format!("Invalid LIDT value: {err}")),
                }
            },
            readonly,
        }
    }];
    let id = property_key.replace(' ', "");
    rsx! {
        div {
            class: "accordion accordion-borderless bg-dark border-start",
            id: "accordionSurface{id}",
            AccordionItem {
                elements: accordion_content,
                header: property_key.to_sentence_case(),
                header_id: "surfaceHeading{id}",
                parent_id: "accordionSurface{id}",
                content_id: "surfaceCollapse{id}",
                level: 2,
            }
        }
    }
}
