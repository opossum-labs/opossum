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
    let mut aperture_sig = use_signal(|| aperture.clone());
    let alignment_sig = use_memo(move || *aperture.isometry());

    let on_alignment_change = EventHandler::new(move |alignment: Isometry| {
        let ap_iso = *aperture_sig.read().isometry();
        if alignment != ap_iso {
            aperture_sig.write().set_isometry(alignment);
            on_change.call(aperture_sig.read().clone());
        }
    });

    let on_shape_change = EventHandler::new(move |aperture: ApertureShape| {
        let ap_shape = aperture_sig.read().shape().clone();
        if aperture != ap_shape {
            aperture_sig.write().set_shape(aperture);
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

    let mut aperture_inputs = vec![];

    aperture_inputs.push(

    rsx! {
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
        div { class: "accordion-content-wrapper-div border-start",
            {
                if let ApertureShape::BinaryPolygon(polygon_config) = aperture_sig.read().shape()
                {
                    rsx! {
                        PolygonApertureInput {
                            polygon_config: polygon_config.clone(),
                            on_shape_change,
                            readonly,
                        }
                    }
                } else if let ApertureShape::Stack(stacked_aperture) = aperture_sig
                    .read()
                    .shape()
                {
                    rsx! {
                        StackedApertureInput {
                            stacked_aperture: stacked_aperture.clone(),
                            on_shape_change,
                            readonly,
                        }
                    }
                } else {
                    rsx! {
                        RowedInputs { inputs: get_aperture_input_data(aperture_sig.read().shape(), on_shape_change, readonly) }
                    }
                }
            }
        }
        RotationAlignmentInputs {
            alignment: alignment_sig,
            axes_skip: Some(vec![RotationAxis::Pitch, RotationAxis::Yaw]),
            on_new_rotation: on_new_rotation(on_alignment_change, alignment_sig.into()),
            node_id,
            readonly,
        }
        TranslationAlignmentInputs {
            alignment: alignment_sig,
            axes_skip: Some(vec![TranslationAxis::Z]),
            on_new_translation: on_new_translation(on_alignment_change, alignment_sig.into()),
            node_id,
            readonly,
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
