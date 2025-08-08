#![allow(clippy::derive_partial_eq_without_eq)]
mod collimated_source_editor;
mod distribution_editor;
mod image_source_editor;
mod point_source_editor;
mod ray_type_selection;

use collimated_source_editor::CollimatedSourceEditor;
use dioxus::prelude::*;
use image_source_editor::ImageSourceEditor;
use opossum_backend::{light_data_builder::LightDataBuilder, ray_data_builder::RayDataBuilder};
use point_source_editor::PointSourceEditor;
use ray_type_selection::RayDataBuilderSelector;

#[component]
pub fn RaySourceEditor(
    ray_data_builder: RayDataBuilder,
    light_data_builder_sig: Signal<LightDataBuilder>,
) -> Element {
    let ray_data_builder_sig = use_signal(|| ray_data_builder.clone());

    use_effect(move || {
        if ray_data_builder != *ray_data_builder_sig.read() {
            light_data_builder_sig.set(LightDataBuilder::Geometric(
                ray_data_builder_sig.read().clone(),
            ));
        }
    });

    rsx! {
        RayDataBuilderSelector { ray_data_builder_sig }
        PointSourceEditor { ray_data_builder_sig }
        CollimatedSourceEditor { ray_data_builder_sig }
        ImageSourceEditor { ray_data_builder_sig }
    }
}
