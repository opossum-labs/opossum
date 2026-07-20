use dioxus::prelude::*;
use opossum_core::{
    prelude::{Aperture, ApertureShape, ApertureType, Isometry},
    utils::{
        default_from_name::DefaultFromName,
        geom_transformation::{RotationAxis, TranslationAxis},
    },
};

use crate::components::node_editor::{
    accordion::AccordionItem,
    hooks::use_synced_signal,
    inputs::{
        InputData, IntoInputData,
        input_components::{LabeledSelect, RowedInputs},
        select_options_from_enum_iterator,
    },
    optical_node_editor::{
        RotationAlignmentInputs, TranslationAlignmentInputs, on_new_rotation, on_new_translation,
        port_config_editor::aperture_editor::{
            CircularApertureParam, GaussianApertureParam, PolygonApertureInput, RectApertureParam,
            StackedApertureInput,
        },
    },
};
use uuid::Uuid;

#[component]
pub fn ApertureEditor(
    node_id: Memo<Uuid>,
    aperture: Aperture,
    on_change: EventHandler<Aperture>,
    readonly: bool,
) -> Element {
    let mut aperture_sig = use_synced_signal(aperture.clone());

    // Fallback to Isometry::identity() if the option is None
    let alignment_sig = use_memo(move || aperture.isometry().copied().unwrap_or_default());

    let on_alignment_change = EventHandler::new(move |alignment: Isometry| {
        let current_iso = aperture_sig.read().isometry().copied().unwrap_or_default();
        if alignment != current_iso {
            aperture_sig.write().set_isometry(alignment);
            on_change.call(aperture_sig.read().clone());
        }
    });

    let on_shape_change = EventHandler::new(move |shape: ApertureShape| {
        let ap_shape = aperture_sig.read().shape().clone();
        if shape != ap_shape {
            aperture_sig.write().set_shape(shape.clone());

            // Reset the isometry if the shape is set to "Open"
            if matches!(shape, ApertureShape::Open) {
                aperture_sig.write().set_isometry(Isometry::default());
            }

            on_change.call(aperture_sig.read().clone());
        }
    });

    let on_type_change = EventHandler::new(move |aperture_type: ApertureType| {
        let ap_type = *aperture_sig.read().aperture_type();
        if aperture_type != ap_type {
            aperture_sig.write().set_aperture_type(aperture_type);
            on_change.call(aperture_sig.read().clone());
        }
    });

    // 1. Extract values outside of RSX to completely bypass macro parsing limitations
    let current_shape = aperture_sig.read().shape().clone();
    let is_open_shape = matches!(current_shape, ApertureShape::Open);

    // 2. Pre-render the specific sub-input view before entering the main rsx! block
    let shape_specific_input = match &current_shape {
        ApertureShape::BinaryPolygon(polygon_config) => rsx! {
            PolygonApertureInput {
                polygon_config: polygon_config.clone(),
                on_shape_change,
                readonly,
            }
        },
        ApertureShape::Stack(stacked_aperture) => rsx! {
            StackedApertureInput {
                stacked_aperture: stacked_aperture.clone(),
                on_shape_change,
                readonly,
            }
        },
        ApertureShape::Open => rsx! {}, // Empty element for open shape
        _ => rsx! {
            RowedInputs { inputs: get_aperture_input_data(&current_shape, on_shape_change, readonly) }
        },
    };

    let mut aperture_inputs = vec![];

    aperture_inputs.push(
    rsx! {
        // 1. Aperture Shape Selector is now first
        LabeledSelect {
            id: "apertureShapeSelector",
            label: "Aperture shape",
            options: select_options_from_enum_iterator(aperture_sig.read().shape(), None),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(aperture_shape) = ApertureShape::default_from_name(val.as_str()) {
                    on_shape_change.call(aperture_shape);
                }
            },
        }

        // Render remaining controls only if shape is not "Open"
        if !is_open_shape {
            LabeledSelect {
                id: "apertureTypeSelector",
                label: "Aperture type",
                options: select_options_from_enum_iterator(aperture_sig.read().aperture_type(), None),
                readonly,
                onchange: move |e: Event<FormData>| {
                    let val = e.value();
                    if let Some(aperture_type) = ApertureType::default_from_name(val.as_str()) {
                        on_type_change.call(aperture_type);
                    }
                },
            }
        }

        if !is_open_shape {
            div { class: "accordion-content-wrapper-div border-start", {shape_specific_input} }
        }

        if !is_open_shape {
            RotationAlignmentInputs {
                alignment: alignment_sig,
                axes_skip: Some(vec![RotationAxis::Pitch, RotationAxis::Yaw]),
                on_new_rotation: on_new_rotation(on_alignment_change, alignment_sig.into()),
                node_id,
                readonly,
            }
        }

        if !is_open_shape {
            TranslationAlignmentInputs {
                alignment: alignment_sig,
                axes_skip: Some(vec![TranslationAxis::Z]),
                on_new_translation: on_new_translation(on_alignment_change, alignment_sig.into()),
                node_id,
                readonly,
            }
        }
    });

    rsx! {
        div {
            class: "accordion accordion-borderless bg-dark noselect",
            id: "accordionApertureConfig",
            AccordionItem {
                elements: aperture_inputs,
                header: "Aperture Configuration",
                header_id: "apertureConfigHeading",
                parent_id: "accordionApertureConfig",
                content_id: "apertureConfigCollapse",
                level: 2,
            }
        }
    }
}

pub(super) fn get_aperture_input_data(
    aperture_shape: &ApertureShape,
    on_save: EventHandler<ApertureShape>,
    readonly: bool,
) -> Vec<InputData> {
    match aperture_shape {
        ApertureShape::BinaryCircle(circle_shape) => {
            CircularApertureParam::to_input_data_vec(circle_shape, on_save, readonly)
        }
        ApertureShape::BinaryRectangle(rectangle_shape) => {
            RectApertureParam::to_input_data_vec(rectangle_shape, on_save, readonly)
        }
        ApertureShape::Gaussian(gaussian_shape) => {
            GaussianApertureParam::to_input_data_vec(gaussian_shape, on_save, readonly)
        }
        _ => vec![],
    }
}
