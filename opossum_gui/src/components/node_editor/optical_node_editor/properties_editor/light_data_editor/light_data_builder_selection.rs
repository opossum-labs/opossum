#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::node_editor::inputs::{
    input_components::LabeledSelect, select_options_from_enum_iterator,
};
use dioxus::prelude::*;
use opossum_backend::{DefaultFromName, light_data_builder::LightDataBuilder};

#[component]
pub fn SourceLightDataBuilderSelector(light_data_builder_sig: Signal<LightDataBuilder>) -> Element {
    rsx! {
        LabeledSelect {
            id: "selectSourceType",
            label: "Source Type",
            options: select_options_from_enum_iterator(&*light_data_builder_sig.read(), None),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(ldb) = LightDataBuilder::default_from_name(val.as_str()) {
                    light_data_builder_sig.set(ldb);
                }
            },
        }
    }
}
