#![allow(clippy::derive_partial_eq_without_eq)]
mod distribution_editor;
mod image_source_editor;
mod point_source_editor;
mod ray_type_selection;
use dioxus::prelude::*;
use opossum_core::prelude::{LightDataBuilder, PointSrc, RayDataSource};
use ray_type_selection::RayDataBuilderSelector;

use crate::components::node_editor::{
    accordion::ElementList,
    analyzer_node_editor::light_data_editor::ray_source_editor::{
        distribution_editor::DistributionEditor,
        image_source_editor::get_image_source_input_params,
        point_source_editor::ReferenceLengthEditor,
    },
    inputs::input_components::RowedInputs,
};

#[component]
pub fn RaySourceEditor(
    ray_data_builder: RayDataSource,
    on_save: EventHandler<LightDataBuilder>,
    readonly: bool,
) -> Element {
    let mut ray_data_builder_sig: Signal<RayDataSource> = use_signal(|| ray_data_builder.clone());

    let on_ray_data_builder_save = EventHandler::new(move |new_ray_data_builder: RayDataSource| {
        on_save.call(LightDataBuilder::Geometric(new_ray_data_builder.clone()));
        ray_data_builder_sig.set(new_ray_data_builder);
    });

    let mut element_list = vec![
        rsx! {RayDataBuilderSelector { ray_data_builder_sig, on_save: on_ray_data_builder_save, readonly }},
    ];

    match &*ray_data_builder_sig.read() {
        RayDataSource::Raw(_) => {}
        RayDataSource::Collimated(_) => {
            element_list.push(rsx! {
                DistributionEditor {
                    ray_data_builder_sig,
                    on_save: on_ray_data_builder_save,
                    readonly,
                }
            });
        }
        RayDataSource::PointSrc(point_src) => {
            element_list.push(rsx! {
                ReferenceLengthEditor {
                    point_src: point_src.clone(),
                    ray_data_handler: EventHandler::new(move |new_point_src: PointSrc| {
                        on_ray_data_builder_save.call(RayDataSource::PointSrc(new_point_src));
                    }),
                    readonly,
                }
                DistributionEditor {
                    ray_data_builder_sig,
                    on_save: on_ray_data_builder_save,
                    readonly,
                }
            });
        }
        RayDataSource::Image(img_src) => {
            let inputs = get_image_source_input_params(img_src, on_ray_data_builder_save, readonly);
            element_list.push(rsx! {
                RowedInputs { inputs }
            });
        }
    }

    rsx! {
        ElementList { element_list }
    }
}
