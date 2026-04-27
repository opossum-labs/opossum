
use opossum_core::{prelude::{Aperture, ApertureShape}, utils::default_from_name::DefaultFromName};
use dioxus::prelude::*;
use inflector::Inflector;

use crate::components::node_editor::{
    inputs::{
        InputData, IntoInputData,
        input_components::{LabeledSelect, RowedInputs},
        select_options_from_enum_iterator,
    },
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
    optical_node_editor::{aperture_editor::rectangular_aperture::RectApertureParam, properties_editor::
        on_save_proptype_handler}
    ,
};
use uuid::Uuid;

#[component]
pub fn ApertureEditor(
    node_id: Memo<Uuid>,
    aperture: Aperture,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let mut aperture_sig = use_signal(|| aperture.clone());

    let on_save = EventHandler::new(move |aperture: ApertureShape|{
        let ap_type = aperture_sig.read().aperture_type().clone();
        if aperture != *aperture_sig.read().shape() {
            let new_aperture = Aperture::new(aperture, ap_type);
            on_change.call(NodeChangeEvent {
                node_id: *node_id.read(),
                action: NodeChangeAction::Aperture(new_aperture.clone()),
            });
            aperture_sig.set(new_aperture);
        }
    });

    rsx! {
        LabeledSelect {
            id: "apertureSelector",
            label: "Aperture definition",
            options: select_options_from_enum_iterator(&*aperture_sig.read().shape(), None),
            readonly,
            onchange: move |e: Event<FormData>| {
                let val = e.value();
                if let Some(aperture_shape) = ApertureShape::default_from_name(
                    val.as_str(),
                ) {
                    on_save.call(aperture_shape);
                }
            },
        }
        div { class: "accordion-content-wrapper-div border-start",
            RowedInputs { inputs: get_aperture_input_data(aperture_sig.read().shape(), on_save, readonly) }
        }
    }
}

fn get_aperture_input_data(
    aperture_shape: &ApertureShape,
    on_save: EventHandler<ApertureShape>,
    readonly: bool,
) -> Vec<InputData> {
    match aperture_shape {
        // Aperture::Const(ref_ind) => {
        //     ConstRefParam::to_input_data_vec(ref_ind, on_save, readonly)
        // }
        // Aperture::Sellmeier1(ref_ind) => {
        //     Sellmeier1Param::to_input_data_vec(ref_ind, on_save, readonly)
        // }
        // Aperture::Schott(ref_ind) => {
        //     SchottParam::to_input_data_vec(ref_ind, on_save, readonly)
        // }
        // Aperture::Conrady(ref_ind) => {
        //     ConradyParam::to_input_data_vec(ref_ind, on_save, readonly)
        // }
        // Aperture::Air(ref_ind) => {
        //     AirParam::to_input_data_vec(ref_ind, on_save, readonly)
        // }
        ApertureShape::Open => vec![],
        ApertureShape::BinaryCircle(circle_shape) => vec![],
        ApertureShape::BinaryRectangle(rectangle_shape) => RectApertureParam::to_input_data_vec(rectangle_shape, on_save, readonly),
        ApertureShape::BinaryPolygon(polygon_config) => vec![],
        ApertureShape::Gaussian(gaussian_shape) => vec![],
        ApertureShape::Stack(stack_shape) => vec![],
    }
}
