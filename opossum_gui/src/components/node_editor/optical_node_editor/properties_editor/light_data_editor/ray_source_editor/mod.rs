#![allow(clippy::derive_partial_eq_without_eq)]
mod collimated_source_editor;
mod distribution_editor;
mod image_source_editor;
mod point_source_editor;
mod ray_type_selection;
use dioxus::prelude::*;
use opossum_core::prelude::{LightDataBuilder, RayDataBuilder};
use ray_type_selection::RayDataBuilderSelector;

use crate::components::node_editor::{
    accordion::ElementList,
    hooks::use_update_signal_with_reactive_prop,
    inputs::input_components::RowedInputs,
    optical_node_editor::properties_editor::light_data_editor::ray_source_editor::{
        collimated_source_editor::ReferenceLengthEditor, distribution_editor::DistributionEditor,
        image_source_editor::get_image_source_input_params,
    },
};

#[component]
pub fn RaySourceEditor(
    ray_data_builder: RayDataBuilder,
    light_data_builder_sig: Signal<LightDataBuilder>,
) -> Element {
    let ray_data_builder_sig: Signal<RayDataBuilder> = use_signal(|| ray_data_builder.clone());
    use_update_signal_with_reactive_prop(ray_data_builder.clone(), ray_data_builder_sig);
    use_context_provider(|| ray_data_builder_sig);

    use_effect(move || {
        if ray_data_builder != *ray_data_builder_sig.read() {
            light_data_builder_sig.set(LightDataBuilder::Geometric(
                ray_data_builder_sig.read().clone(),
            ));
        }
    });

    let mut element_list = vec![rsx! {RayDataBuilderSelector { ray_data_builder_sig }}];

    match &*ray_data_builder_sig.read() {
        RayDataBuilder::Raw(_) => {}
        RayDataBuilder::Collimated(_) => {
            element_list.push(rsx! {
                DistributionEditor {}
            });
        }
        RayDataBuilder::PointSrc(point_src) => {
            element_list.push(rsx! {
                ReferenceLengthEditor { point_src: point_src.clone() }
                DistributionEditor {}
            });
        }
        RayDataBuilder::Image(img_src) => {
            let inputs = get_image_source_input_params(img_src, ray_data_builder_sig);
            element_list.push(rsx! {
                RowedInputs { inputs }
            });
        }
    }

    rsx! {
        ElementList { element_list }
    }
}
