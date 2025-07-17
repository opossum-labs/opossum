#![allow(clippy::derive_partial_eq_without_eq)]

mod energy_source_editor;
mod light_data_builder_selection;
mod ray_source_editor;

use crate::components::node_editor::{
    accordion::AccordionItem,
    property_editor::light_data_editor::energy_source_editor::EnergySourceEditor,
};
use light_data_builder_selection::SourceLightDataBuilderSelector;
use opossum_backend::{light_data_builder::LightDataBuilder, Proptype};
use ray_source_editor::RaySourceEditor;

use dioxus::prelude::*;

#[component]
pub fn LightDataEditor(
    light_data_builder: LightDataBuilder,
    prop_type_sig: Signal<Proptype>,
) -> Element {
    let light_data_builder_sig = use_signal(|| light_data_builder);

    use_effect(move || {
        prop_type_sig.set(Proptype::LightDataBuilder(Some(
            light_data_builder_sig.read().clone(),
        )));
    });

    let accordion_item_content = rsx! {
        SourceLightDataBuilderSelector { light_data_builder_sig }
        RaySourceEditor { light_data_builder_sig }
        EnergySourceEditor { light_data_builder_sig }
    };

    rsx! {
        div {
            class: "accordion accordion-borderless bg-dark border-start",
            id: "accordionLightDataConfig",
            AccordionItem {
                elements: vec![accordion_item_content],
                header: "Light definition",
                header_id: "sourceHeading",
                parent_id: "accordionLightDataConfig",
                content_id: "sourceCollapse",
            }
        }
    }
}
