#![allow(clippy::derive_partial_eq_without_eq)]

mod energy_source_editor;
mod light_data_builder_selection;
mod ray_source_editor;

use crate::components::node_editor::{
    accordion::AccordionItem,
    node_config_editor::NodeChangeEvent,
    optical_node_editor::properties_editor::{
        light_data_editor::energy_source_editor::EnergySourceEditor, on_save_proptype_handler
    },
};
use light_data_builder_selection::SourceLightDataBuilderSelector;
use opossum_core::prelude::LightDataBuilder;
use ray_source_editor::RaySourceEditor;
use uuid::Uuid;

use dioxus::prelude::*;

#[component]
pub fn LightDataEditor(
    node_id: Memo<Uuid>,
    light_data_builder: LightDataBuilder,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let light_data_builder_sig = use_signal(|| light_data_builder.clone());

    let on_save = on_save_proptype_handler(
        light_data_builder_sig,
        property_key.clone(),
        on_change,
        node_id.into(),
    );

    let mut accordion_item_content = vec![rsx! {
        SourceLightDataBuilderSelector { light_data_builder_sig, on_save }
    }];

    match &*light_data_builder_sig.read() {
        LightDataBuilder::Energy(energy_data_builder) => accordion_item_content.push(rsx! {
            EnergySourceEditor { energy_data_builder: energy_data_builder.clone(), on_save }
        }),
        LightDataBuilder::Geometric(ray_data_builder) => accordion_item_content.push(rsx! {
            RaySourceEditor { ray_data_builder: ray_data_builder.clone(), on_save }
        }),
    }
    rsx! {
        div {
            class: "accordion accordion-borderless bg-dark border-start",
            id: "accordionLightDataConfig",
            AccordionItem {
                elements: accordion_item_content,
                header: "Light definition",
                header_id: "sourceHeading",
                parent_id: "accordionLightDataConfig",
                content_id: "sourceCollapse",
            }
        }
    }
}
