#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::node_editor::inputs::{
    input_components::LabeledSelect, select_options_from_enum_iterator,
};
use dioxus::prelude::*;
use opossum_core::{prelude::LightDataBuilder, utils::default_from_name::DefaultFromName};

#[component]
pub fn SourceLightDataBuilderSelector(
    light_data_builder_sig: ReadSignal<LightDataBuilder>,
    on_save: EventHandler<LightDataBuilder>,
        readonly: bool
) -> Element {
    rsx! {
        LabeledSelect {
            id: "selectSourceType",
            label: "Source Type",
            options: select_options_from_enum_iterator(&*light_data_builder_sig.read(), None),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(ldb) = LightDataBuilder::default_from_name(val.as_str()) {
                    on_save.call(ldb);
                }
            },
        }
    }
}
