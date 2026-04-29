use dioxus::prelude::*;
use opossum_core::{
    prelude::{Aperture, ApertureShape, ApertureType},
    utils::default_from_name::DefaultFromName,
};

use crate::components::node_editor::{
    inputs::{
        InputData, IntoInputData,
        input_components::{LabeledSelect, RowedInputs},
        select_options_from_enum_iterator,
    },
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
    optical_node_editor::aperture_editor::{
        CircularApertureParam, GaussianApertureParam, PolygonApertureInput, RectApertureParam,
        StackedApertureInput,
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
                        PolygonApertureInput { polygon_config: polygon_config.clone(), on_shape_change, readonly }
                    }
                } else if let ApertureShape::Stack(stacked_aperture) = aperture_sig
                    .read()
                    .shape()
                {
                    rsx! {
                        StackedApertureInput { stacked_aperture: stacked_aperture.clone(), on_shape_change, readonly }
                    }
                } else {
                    rsx! {
                        RowedInputs { inputs: get_aperture_input_data(aperture_sig.read().shape(), on_shape_change, readonly) }
                    }
                }
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
