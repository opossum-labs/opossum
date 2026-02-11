#![allow(clippy::derive_partial_eq_without_eq)]

use crate::components::node_editor::inputs::{
    input_components::LabeledSelect, select_options_from_enum_iterator,
};
use dioxus::prelude::*;
use opossum_core::{
    prelude::RayDataBuilder, rays::Rays, utils::default_from_name::DefaultFromName,
};

#[component]
pub fn RayDataBuilderSelector(ray_data_builder_sig: ReadSignal<RayDataBuilder>, on_save: EventHandler<RayDataBuilder>) -> Element {
    rsx! {
        LabeledSelect {
            id: "selectRaySourceType",
            label: "Rays Type",
            options: select_options_from_enum_iterator(
                &*ray_data_builder_sig.read(),
                Some(&[&RayDataBuilder::Raw(Rays::default())]),
            ),
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(rdb) = RayDataBuilder::default_from_name(val.as_str()) {
                    on_save.call(rdb);
                }
            },
        }
    }
}
