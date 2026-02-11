#![allow(clippy::derive_partial_eq_without_eq)]
mod distribution_editor;
mod image_source_editor;
mod point_source_editor;
mod ray_type_selection;
use dioxus::prelude::*;
use opossum_core::prelude::{LightDataBuilder, PointSrc, RayDataBuilder};
use ray_type_selection::RayDataBuilderSelector;

use crate::components::node_editor::{
    accordion::ElementList,
    hooks::use_update_signal_with_reactive_prop,
    inputs::input_components::RowedInputs,
    optical_node_editor::properties_editor::light_data_editor::ray_source_editor::{
        distribution_editor::DistributionEditor,
        image_source_editor::get_image_source_input_params,
        point_source_editor::ReferenceLengthEditor,
    },
};

#[component]
pub fn RaySourceEditor(
    ray_data_builder: RayDataBuilder,
    on_save: EventHandler<LightDataBuilder>,
) -> Element {
    let mut ray_data_builder_sig: Signal<RayDataBuilder> = use_signal(|| ray_data_builder.clone());

    let on_ray_data_builder_save = EventHandler::new(move |new_ray_data_builder: RayDataBuilder| {
        on_save.call(LightDataBuilder::Geometric(new_ray_data_builder.clone()));
        ray_data_builder_sig.set(new_ray_data_builder);
    });

    let mut element_list = vec![rsx! {RayDataBuilderSelector { ray_data_builder_sig, on_save: on_ray_data_builder_save }}];

    match &*ray_data_builder_sig.read() {
        RayDataBuilder::Raw(_) => {}
        RayDataBuilder::Collimated(_) => {
            element_list.push(rsx! {
                DistributionEditor {
                    ray_data_builder_sig,
                    on_save: on_ray_data_builder_save,
                }
            });
        }
        RayDataBuilder::PointSrc(point_src) => {
            element_list.push(rsx! {
                ReferenceLengthEditor {
                    point_src: point_src.clone(),
                    ray_data_handler: EventHandler::new(move |new_point_src: PointSrc| {
                        on_ray_data_builder_save.call(RayDataBuilder::PointSrc(new_point_src));
                    }),
                }
                DistributionEditor {
                    ray_data_builder_sig,
                    on_save: on_ray_data_builder_save,
                }
            });
        }
        RayDataBuilder::Image(img_src) => {
            let inputs = get_image_source_input_params(img_src, on_ray_data_builder_save);
            element_list.push(rsx! {
                RowedInputs { inputs }
            });
        }
    }

    rsx! {
        ElementList { element_list }
    }
}
