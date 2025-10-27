use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        CallbackWrapper,
        accordion::AccordionItem,
        inputs::{InputData, input_components::RowedInputs},
        optical_node_editor::properties_editor::use_set_node_change_property,
    },
};
use dioxus::prelude::*;
use opossum_core::{
    degree, millimeter,
    prelude::Isometry,
    utils::geom_transformation::{AlignmentAxis, RotationAxis, TranslationAxis},
};
use strum::IntoEnumIterator;
use uom::si::{angle::degree, length::millimeter};

#[component]
pub fn IsometryOptionEditor(isometry: Isometry, property_key: String) -> Element {
    let isometry_sig = use_signal(|| isometry);

    use_set_node_change_property(&property_key, isometry, isometry_sig);

    let input_data = get_isometry_option_input_data(isometry_sig);

    let accordion_content = vec![rsx! {
        RowedInputs {inputs: input_data }
    }];

    rsx! {
        div {
            class: "accordion accordion-borderless bg-dark border-start",
            id: "accordionIsometryOptionConfig",
            AccordionItem {
                elements: accordion_content,
                header: "Source isometry",
                header_id: "srcIsometryHeading",
                parent_id: "accordionIsometryOptionConfig",
                content_id: "srcIsometryCollapse",
            }
        }
    }
}

fn on_isometry_option_change(
    mut isometry_sig: Signal<Isometry>,
    axis_type: AlignmentAxis,
) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(val) = e.data.value().parse::<f64>() {
            let res = match axis_type {
                AlignmentAxis::Translation(translation_axis) => isometry_sig
                    .write()
                    .set_translation_of_axis(translation_axis, millimeter!(val)),
                AlignmentAxis::Rotation(rotation_axis) => isometry_sig
                    .write()
                    .set_rotation_of_axis(rotation_axis, degree!(val)),
            };
            if let Err(err_str) = res {
                OPOSSUM_UI_LOGS.write().add_log(
                    format!("Failed to set alignment for axis {axis_type}: {err_str}",).as_str(),
                );
            }
        }
    })
}

fn get_isometry_option_input_data(isometry_sig: Signal<Isometry>) -> Vec<InputData> {
    let id_add_on = "isometryOptionInput";
    let mut alignment_inputs = Vec::<InputData>::new();
    for (trans_axis, rot_axis) in TranslationAxis::iter().zip(RotationAxis::iter()) {
        alignment_inputs.push(InputData::new(
            trans_axis.into(),
            id_add_on,
            on_isometry_option_change(isometry_sig, AlignmentAxis::Translation(trans_axis)),
            format!(
                "{:.3}",
                isometry_sig
                    .read()
                    .translation_of_axis(trans_axis)
                    .get::<millimeter>()
            ),
        ));
        alignment_inputs.push(InputData::new(
            rot_axis.into(),
            id_add_on,
            on_isometry_option_change(isometry_sig, AlignmentAxis::Rotation(rot_axis)),
            format!(
                "{:.3}",
                isometry_sig
                    .read()
                    .rotation_of_axis(rot_axis)
                    .get::<degree>()
            ),
        ));
    }
    alignment_inputs
}
