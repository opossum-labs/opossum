mod distribution_editor;
mod image_source_editor;
mod point_source_editor;
mod ray_type_selection;

use crate::components::node_editor::{
    accordion::{AccordionItem, ElementList},
    analyzer_node_editor::light_data_editor::ray_source_editor::{
        distribution_editor::DistributionEditor,
        image_source_editor::get_image_source_input_params,
        point_source_editor::ReferenceLengthEditor,
    },
    hooks::use_synced_signal,
    inputs::input_components::RowedInputs,
    optical_node_editor::{
        RotationAlignmentInputs, TranslationAlignmentInputs, on_new_rotation, on_new_translation,
    },
};
use dioxus::prelude::*;
use opossum_core::{
    light::lightdata::ray_data_builder::RayDataBuilder,
    prelude::{Isometry, PointSrc, RayDataSource},
};
use ray_type_selection::RayDataBuilderSelector;
use uuid::Uuid;

#[component]
pub fn RaySourceEditor(
    port_uuid: Uuid,
    ray_data_builder: RayDataBuilder,
    on_save: EventHandler<RayDataBuilder>,
    readonly: bool,
) -> Element {
    let mut ray_data_builder_sig: Signal<RayDataBuilder> = use_synced_signal(ray_data_builder);

    // Central save helper propagating the complete `RayDataBuilder` upward
    let mut save_builder = move |new_builder: RayDataBuilder| {
        on_save.call(new_builder.clone());
        ray_data_builder_sig.set(new_builder);
    };

    // Derived signal for the inner `RayDataSource`
    let ray_source_sig = use_memo(move || ray_data_builder_sig.read().source().clone());

    // Handler updating only `source` while preserving `isometry`
    let on_ray_source_save = EventHandler::new(move |new_source: RayDataSource| {
        let mut updated_builder = ray_data_builder_sig.peek().clone();
        updated_builder.set_source(new_source);
        save_builder(updated_builder);
    });

    // Treat `None` isometry as Identity (`Isometry::default()`) for display
    let isometry_memo =
        use_memo(move || ray_data_builder_sig.read().isometry().unwrap_or_default());
    let port_id_sig = use_synced_signal(port_uuid);

    let on_isometry_change = EventHandler::new(move |new_iso: Isometry| {
        let mut updated_builder = ray_data_builder_sig.peek().clone();
        updated_builder.set_isometry(Some(new_iso));
        save_builder(updated_builder);
    });

    let mut element_list = vec![rsx! {
        RayDataBuilderSelector {
            ray_data_builder_sig: ray_source_sig,
            on_save: on_ray_source_save,
            readonly,
        }
    }];

    match ray_data_builder_sig.read().source() {
        RayDataSource::Raw(_) => {}
        RayDataSource::Collimated(_) => {
            element_list.push(rsx! {
                DistributionEditor {
                    ray_data_builder_sig: ray_source_sig,
                    on_save: on_ray_source_save,
                    readonly,
                }
            });
        }
        RayDataSource::PointSrc(point_src) => {
            element_list.push(rsx! {
                ReferenceLengthEditor {
                    point_src: point_src.clone(),
                    ray_data_handler: EventHandler::new(move |new_point_src: PointSrc| {
                        on_ray_source_save.call(RayDataSource::PointSrc(new_point_src));
                    }),
                    readonly,
                }
                DistributionEditor {
                    ray_data_builder_sig: ray_source_sig,
                    on_save: on_ray_source_save,
                    readonly,
                }
            });
        }
        RayDataSource::Image(img_src) => {
            let inputs = get_image_source_input_params(img_src, on_ray_source_save, readonly);
            element_list.push(rsx! {
                RowedInputs { inputs }
            });
        }
    }

    // Global source isometry (position and orientation in 3D space, e.g., for off-axis sources)
    let isometry_accordion_content = vec![rsx! {
        RotationAlignmentInputs {
            alignment: isometry_memo,
            axes_skip: None,
            on_new_rotation: on_new_rotation(on_isometry_change, isometry_memo.into()),
            node_id: port_id_sig,
            readonly,
        }
        TranslationAlignmentInputs {
            alignment: isometry_memo,
            axes_skip: None,
            on_new_translation: on_new_translation(on_isometry_change, isometry_memo.into()),
            node_id: port_id_sig,
            readonly,
        }
    }];

    element_list.push(rsx! {
        div {
            class: "accordion accordion-borderless bg-dark border-start",
            id: "accordionSourceIsometry{port_uuid}",
            AccordionItem {
                elements: isometry_accordion_content,
                header: "Source isometry".to_string(),
                header_id: format!("srcIsometryHeading{port_uuid}"),
                parent_id: format!("accordionSourceIsometry{port_uuid}"),
                content_id: format!("srcIsometryCollapse{port_uuid}"),
                level: 2,
            }
        }
    });

    rsx! {
        ElementList { element_list }
    }
}
