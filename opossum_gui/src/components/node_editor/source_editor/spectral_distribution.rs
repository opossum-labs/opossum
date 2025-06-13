use crate::components::node_editor::{
    accordion::AccordionItem, source_editor::LightDataBuilderHistory,
};
use dioxus::prelude::*;

#[component]
pub fn SpectralDistributionEditor(
    light_data_builder_sig: Signal<LightDataBuilderHistory>,
) -> Element {
    let accordion_item_content = rsx! {};

    rsx! {
        AccordionItem {
            elements: vec![accordion_item_content],
            header: "Spectral Distribution",
            header_id: "sourceSpectralDistHeading",
            parent_id: "accordionSourceDists",
            content_id: "sourceSpectralDistCollapse",
        }
    }
}
