#![allow(clippy::derive_partial_eq_without_eq)]
use crate::{
    components::node_editor::{
        accordion::AccordionItem, inputs::{input_components::{LabeledInput, RowedInputs}, InputData, InputParam},
        node_editor_component::NodeChange, CallbackWrapper,
    },
    OPOSSUM_UI_LOGS,
};
use dioxus::prelude::*;
use opossum_backend::{degree, millimeter, Isometry, RotationAxis, TranslationAxis};
use strum::IntoEnumIterator;
use uom::si::{angle::degree, length::millimeter};

#[component]
pub fn AlignmentEditor(alignment: Option<Isometry>) -> Element {
    let node_change_signal: Signal<Option<NodeChange>> = use_context::<Signal<Option<NodeChange>>>();
    let iso = Signal::new(alignment.unwrap_or_else(Isometry::identity));
    let input_data = get_alignment_input_data(node_change_signal, iso);

    let accordion_content = vec![rsx! {
        RowedInputs {inputs: input_data }
    }];

    rsx! {
        AccordionItem {
            elements: accordion_content,
            header: "Alignment",
            header_id: "alignmentHeading",
            parent_id: "accordionNodeConfig",
            content_id: "alignmentCollapse",
        }
    }
}


fn translation_onchange(
    mut node_change: Signal<Option<NodeChange>>,
    mut iso_sig: Signal<Isometry>,
    axis: TranslationAxis,
) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        let Ok(value) = e.data.value().parse::<f64>() else {
            return;
        };
        let mut iso = iso_sig();
        match iso.set_translation_of_axis(axis, millimeter!(value)) {
            Ok(()) => {
                iso_sig.set(iso);
                node_change.set(Some(NodeChange::Alignment(iso)));
            }
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(
                    format!("Failed to set translation for axis {axis}: {err_str}").as_str(),
                );
            }
        }
    })
}


fn rotation_onchange(
    mut node_change: Signal<Option<NodeChange>>,
    mut iso_sig: Signal<Isometry>,
    axis: RotationAxis,
) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        let Ok(value) = e.data.value().parse::<f64>() else {
            return;
        };
        let mut iso = iso_sig();
        match iso.set_rotation_of_axis(axis, degree!(value)) {
            Ok(()) => {
                iso_sig.set(iso);
                node_change.set(Some(NodeChange::Alignment(iso)));
            }
            Err(err_str) => {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log(format!("Failed to set rotation for axis {axis}: {err_str}").as_str());
            }
        }
    })
}




fn get_alignment_input_data(
    node_change_signal: Signal<Option<NodeChange>>,
    iso: Signal<Isometry>, 
) -> Vec<InputData> {
    let id_add_on = "inputNodeAlignment".to_string();
    let mut alignment_trans_inputs = Vec::<InputData>::new();
    let mut alignment_rot_inputs = Vec::<InputData>::new();
    for trans_axis in TranslationAxis::iter(){
        alignment_trans_inputs.push(
            InputData::new(
            trans_axis.into(),
            &id_add_on,
            translation_onchange(node_change_signal, iso, trans_axis),
            format!("{:.3}", iso.read().translation_of_axis(trans_axis).get::<millimeter>()),
        )
        )
    }
    for rot_axis in RotationAxis::iter(){
        alignment_rot_inputs.push(
            InputData::new(
            rot_axis.into(),
            &id_add_on,
            rotation_onchange(node_change_signal, iso, rot_axis),
            format!("{:.3}", iso.read().rotation_of_axis(rot_axis).get::<degree>()),
        )
        )
    }

    let mut alignment_inputs = Vec::<InputData>::new();

    for (trans, rot) in alignment_trans_inputs.iter().zip(alignment_rot_inputs.iter()) {
        alignment_inputs.push(trans.clone());
        alignment_inputs.push(rot.clone());
    }
    alignment_inputs
}
