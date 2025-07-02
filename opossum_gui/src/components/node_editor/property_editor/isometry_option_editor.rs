use crate::{
    components::node_editor::{
        accordion::AccordionItem,
        inputs::{input_components::RowedInputs, InputData},
        CallbackWrapper,
    },
    OPOSSUM_UI_LOGS,
};
use dioxus::prelude::*;
use opossum_backend::{
    degree, millimeter, AlignmentAxis, Isometry, Proptype, RotationAxis, TranslationAxis,
};
use strum::IntoEnumIterator;
use uom::si::{angle::degree, length::millimeter};

#[component]
pub fn IsometryOptionEditor(property_key: String, prop_type_sig: Signal<Proptype>) -> Element {
    if let Proptype::Isometry(iso_opt) = &*prop_type_sig.read() {
        let iso = iso_opt.unwrap_or_default();
        let input_data = get_isometry_option_input_data(iso, prop_type_sig);

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
    } else {
        rsx! {}
    }
}

fn on_isometry_option_change(
    mut iso: Isometry,
    mut prop_type_sig: Signal<Proptype>,
    axis_type: AlignmentAxis,
) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(val) = e.data.value().parse::<f64>() {
            let res = match axis_type {
                AlignmentAxis::Translation(translation_axis) => {
                    iso.set_translation_of_axis(translation_axis, millimeter!(val))
                }
                AlignmentAxis::Rotation(rotation_axis) => {
                    iso.set_rotation_of_axis(rotation_axis, degree!(val))
                }
            };
            match res {
                Ok(()) => {
                    prop_type_sig.set(Proptype::Isometry(Some(iso)));
                }
                Err(err_str) => {
                    OPOSSUM_UI_LOGS.write().add_log(
                        format!("Failed to set alignment for axis {axis_type}: {err_str}",)
                            .as_str(),
                    );
                }
            }
        }
    })
}

fn get_isometry_option_input_data(
    iso: Isometry,
    prop_type_sig: Signal<Proptype>,
) -> Vec<InputData> {
    let id_add_on = "isometryOptionInput".to_string();
    let mut alignment_inputs = Vec::<InputData>::new();
    for (trans_axis, rot_axis) in TranslationAxis::iter().zip(RotationAxis::iter()) {
        alignment_inputs.push(InputData::new(
            trans_axis.into(),
            &id_add_on,
            on_isometry_option_change(iso, prop_type_sig, AlignmentAxis::Translation(trans_axis)),
            format!(
                "{:.3}",
                iso.translation_of_axis(trans_axis).get::<millimeter>()
            ),
        ));
        alignment_inputs.push(InputData::new(
            rot_axis.into(),
            &id_add_on,
            on_isometry_option_change(iso, prop_type_sig, AlignmentAxis::Rotation(rot_axis)),
            format!("{:.3}", iso.rotation_of_axis(rot_axis).get::<degree>()),
        ));
    }
    alignment_inputs
}
