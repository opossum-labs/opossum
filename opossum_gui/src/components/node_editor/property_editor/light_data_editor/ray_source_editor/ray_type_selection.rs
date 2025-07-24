#![allow(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;
use opossum_backend::{ray_data_builder::RayDataBuilder, DefaultFromName, Rays};

use crate::components::node_editor::inputs::{
    input_components::LabeledSelect, select_options_from_enum_iterator,
};

#[component]
pub fn RayDataBuilderSelector(ray_data_builder_sig: Signal<RayDataBuilder>) -> Element {
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
                    ray_data_builder_sig.set(rdb);
                }
            },
        }
    }
}
