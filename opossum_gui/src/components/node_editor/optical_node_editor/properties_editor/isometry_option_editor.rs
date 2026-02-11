use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        accordion::AccordionItem,
        inputs::{InputData, input_components::RowedInputs},
        node_config_editor::NodeChangeEvent,
        optical_node_editor::properties_editor::on_save_proptype_handler,
    },
};
use dioxus::prelude::*;
use opossum_core::{
    degree, meter,
    prelude::Isometry,
    utils::geom_transformation::{AlignmentAxis, RotationAxis, TranslationAxis},
};
use strum::IntoEnumIterator;
use uom::si::{
    angle::degree,
    f64::{Angle, Length},
};
use uuid::Uuid;

#[component]
pub fn IsometryOptionEditor(
    node_id: Memo<Uuid>,
    isometry: Isometry,
    property_key: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let isometry_sig = use_signal(|| isometry);
    let on_save = on_save_proptype_handler(
        isometry_sig,
        property_key.clone(),
        on_change,
        node_id.into(),
    );

    let on_new_rotation = EventHandler::new(move |(axis, rotation): (RotationAxis, Angle)| {
        let mut iso = *isometry_sig.read();
        if iso.set_rotation_of_axis(axis, rotation).is_ok() {
            on_save.call(iso);
        } else {
            OPOSSUM_UI_LOGS
                .write()
                .add_log(format!("Failed to set rotation for axis {axis}").as_str());
        }
    });
    let on_new_translation =
        EventHandler::new(move |(axis, translation): (TranslationAxis, Length)| {
            let mut iso = *isometry_sig.read();
            if iso.set_translation_of_axis(axis, translation).is_ok() {
                on_save.call(iso);
            } else {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log(format!("Failed to set translation for axis {axis}").as_str());
            }
        });

    let input_data =
        get_isometry_option_input_data(on_new_rotation, on_new_translation, isometry_sig.into());
    let accordion_content = vec![rsx! {
        RowedInputs { inputs: input_data }
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
fn on_isometry_option_change_str(
    on_new_rotation: EventHandler<(RotationAxis, Angle)>,
    on_new_translation: EventHandler<(TranslationAxis, Length)>,
    axis_type: AlignmentAxis,
) -> EventHandler<String> {
    EventHandler::new(move |val_str: String| {
        if let Ok(val) = val_str.parse::<f64>() {
            match axis_type {
                AlignmentAxis::Translation(translation_axis) => {
                    on_new_translation.call((translation_axis, meter!(val)));
                }
                AlignmentAxis::Rotation(rotation_axis) => {
                    on_new_rotation.call((rotation_axis, degree!(val)));
                }
            };
        }
    })
}

fn get_isometry_option_input_data(
    on_new_rotation: EventHandler<(RotationAxis, Angle)>,
    on_new_translation: EventHandler<(TranslationAxis, Length)>,
    isometry_sig: ReadSignal<Isometry>,
) -> Vec<InputData> {
    let id_add_on = "isometryOptionInput";
    let mut alignment_inputs = Vec::<InputData>::new();
    for (trans_axis, rot_axis) in TranslationAxis::iter().zip(RotationAxis::iter()) {
        alignment_inputs.push(InputData::new(
            trans_axis.into(),
            id_add_on,
            EventHandler::new(|_| {}),
            on_isometry_option_change_str(
                on_new_rotation,
                on_new_translation,
                AlignmentAxis::Translation(trans_axis),
            ),
            format!(
                "{}",
                isometry_sig.read().translation_of_axis(trans_axis).value
            ),
        ));
        alignment_inputs.push(InputData::new(
            rot_axis.into(),
            id_add_on,
            EventHandler::new(|_| {}),
            on_isometry_option_change_str(
                on_new_rotation,
                on_new_translation,
                AlignmentAxis::Rotation(rot_axis),
            ),
            format!(
                "{}",
                isometry_sig
                    .read()
                    .rotation_of_axis(rot_axis)
                    .get::<degree>()
            ),
        ));
    }
    alignment_inputs
}
