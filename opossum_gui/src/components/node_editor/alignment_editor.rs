#![allow(clippy::derive_partial_eq_without_eq)]
use crate::{
    components::node_editor::{
        accordion::AccordionItem, inputs::{input_components::{LabeledInput, RowedInputs}, InputData, InputParam},
        node_editor_component::NodeChange, CallbackWrapper,
    },
    OPOSSUM_UI_LOGS,
};
use dioxus::prelude::*;
use opossum_backend::{degree, millimeter, AlignmentAxis, Isometry, RotationAxis, TranslationAxis};
use strum::IntoEnumIterator;
use uom::si::{angle::degree, length::millimeter};

#[component]
pub fn AlignmentEditor(alignment: Option<Isometry>) -> Element {
    let iso_sig = Signal::new(alignment.unwrap_or_else(Isometry::identity));
    rsx!{
        AlignmentInputs{iso_sig}
    }    
}

#[component]
fn AlignmentInputs(iso_sig: Signal<Isometry>) -> Element{
    let node_change_signal: Signal<Option<NodeChange>> = use_context::<Signal<Option<NodeChange>>>();
    let input_data = get_alignment_input_data(node_change_signal, iso_sig);

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

fn on_isometry_option_change(
    mut node_change: Signal<Option<NodeChange>>,
    mut iso_sig: Signal<Isometry>,
    axis_type: AlignmentAxis,
) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(val) = e.data.value().parse::<f64>() {
            let mut iso = iso_sig();
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
                    iso_sig.set(iso);
                    node_change.set(Some(NodeChange::Alignment(iso)));
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

fn get_alignment_input_data(
    node_change_signal: Signal<Option<NodeChange>>,
    iso: Signal<Isometry>,
) -> Vec<InputData> {
    let id_add_on = "inputNodeAlignment".to_string();
    let mut alignment_inputs = Vec::<InputData>::new();
    for (trans_axis, rot_axis) in TranslationAxis::iter().zip(RotationAxis::iter()) {
        alignment_inputs.push(InputData::new(
            trans_axis.into(),
            &id_add_on,
            on_isometry_option_change(node_change_signal, iso, AlignmentAxis::Translation(trans_axis)),
            format!(
                "{:.3}",
                iso.read()
                    .translation_of_axis(trans_axis)
                    .get::<millimeter>()
            ),
        ));
        alignment_inputs.push(InputData::new(
            rot_axis.into(),
            &id_add_on,
            on_isometry_option_change(node_change_signal, iso, AlignmentAxis::Rotation(rot_axis)),
            format!(
                "{:.3}",
                iso.read().rotation_of_axis(rot_axis).get::<degree>()
            ),
        ));
    }
    alignment_inputs
}
