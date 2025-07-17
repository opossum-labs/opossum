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
pub fn RaySourceEditor(light_data_builder_sig: Signal<LightDataBuilder>) -> Element {
    let ray_data_builder_sig = use_signal(|| {
        if let LightDataBuilder::Geometric(rdb) = &*light_data_builder_sig.read() {
            rdb.clone()
        } else {
            RayDataBuilder::default()
        }
    });

    use_effect(move || {
        light_data_builder_sig.set(LightDataBuilder::Geometric(
            ray_data_builder_sig.read().clone(),
        ));
    });

    if let LightDataBuilder::Geometric(_) = &*light_data_builder_sig.read() {
        rsx! {
            RayDataBuilderSelector { ray_data_builder_sig }
            PointSourceEditor { ray_data_builder_sig }
            CollimatedSourceEditor { ray_data_builder_sig }
            ImageSourceEditor { ray_data_builder_sig }
        }
    } else {
        rsx! {}
    }
}
