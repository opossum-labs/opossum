#![allow(clippy::derive_partial_eq_without_eq)]

mod energy_source_editor;
mod light_data_builder_selection;
mod ray_source_editor;

use crate::components::node_editor::{
    accordion::AccordionItem,
    node_editor_component::NodeChange,
    property_editor::{
        light_data_editor::energy_source_editor::EnergySourceEditor, use_set_node_change_property,
    },
};
use light_data_builder_selection::SourceLightDataBuilderSelector;
use opossum_backend::light_data_builder::LightDataBuilder;
use ray_source_editor::RaySourceEditor;

use dioxus::prelude::*;

#[component]
pub fn LightDataEditor(
    light_data_builder: LightDataBuilder,
    property_key: String,
    node_change: Signal<Option<NodeChange>>,
) -> Element {
    let light_data_builder_sig = use_signal(|| light_data_builder.clone());

    use_set_node_change_property(
        &property_key,
        light_data_builder,
        light_data_builder_sig,
        node_change,
    );

    let mut accordion_item_content = vec![rsx! {
    SourceLightDataBuilderSelector {light_data_builder_sig }}];

    match light_data_builder_sig() {
        LightDataBuilder::Energy(energy_data_builder) => accordion_item_content.push(rsx! {
            EnergySourceEditor { energy_data_builder, light_data_builder_sig }
        }),
        LightDataBuilder::Geometric(ray_data_builder) => accordion_item_content.push(rsx! {
            RaySourceEditor { ray_data_builder, light_data_builder_sig }
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
