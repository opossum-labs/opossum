#![allow(clippy::derive_partial_eq_without_eq)]

mod grating_alignment;

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        CallbackWrapper,
        accordion::AccordionItem,
        inputs::{InputData, input_components::RowedInputs},
        node_config_editor::NodeChangeAction,
    },
};
use dioxus::prelude::*;
use grating_alignment::GratingAlignmentInputs;
use opossum_backend::{
    AlignmentAxis, Isometry, Properties, RotationAxis, TranslationAxis, degree, millimeter,
};
use strum::IntoEnumIterator;
use uom::si::{angle::degree, length::millimeter};

#[component]
pub fn AlignmentEditor(
    alignment_sig: Signal<Isometry>,
    node_properties_sig: Signal<Properties>,
    node_type: String,
) -> Element {
    let mut node_change_sig = use_context::<Signal<Option<NodeChangeAction>>>();

    use_effect(move || node_change_sig.set(Some(NodeChangeAction::Alignment(*alignment_sig.read()))));

    let accordion_content = if node_type == "reflective grating" {
        rsx! {
            GratingAlignmentInputs { alignment_sig, node_properties_sig }
        }
    } else {
        rsx! {
            RotationAlignmentInputs { alignment_sig, axes_skip: None }
            TranslationAlignmentInputs { alignment_sig }
        }
    };
    rsx! {
        AccordionItem {
            elements: vec![accordion_content],
            header: "Alignment",
            header_id: "alignmentHeading",
            parent_id: "accordionNodeConfig",
            content_id: "alignmentCollapse",
        }
    }
}

#[component]
fn TranslationAlignmentInputs(alignment_sig: Signal<Isometry>) -> Element {
    let input_data = get_translation_alignment_input_data(*alignment_sig.read(), alignment_sig);

    rsx! {
        RowedInputs { inputs: input_data }
    }
}

#[component]
fn RotationAlignmentInputs(
    alignment_sig: Signal<Isometry>,
    axes_skip: Option<Vec<RotationAxis>>,
) -> Element {
    let input_data =
        get_rotation_alignment_input_data(*alignment_sig.read(), alignment_sig, axes_skip.as_ref());
    rsx! {
        RowedInputs { inputs: input_data }
    }
}

fn on_isometry_option_change(
    mut iso_sig: Signal<Isometry>,
    axis_type: AlignmentAxis,
) -> CallbackWrapper {
    CallbackWrapper::new(move |e: Event<FormData>| {
        if let Ok(val) = e.data.value().parse::<f64>() {
            let mut iso = *iso_sig.read();
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

fn get_translation_alignment_input_data(
    iso: Isometry,
    iso_sig: Signal<Isometry>,
) -> Vec<InputData> {
    let id_add_on = "inputNodeAlignmentTrans";
    let mut alignment_inputs = Vec::<InputData>::new();
    for trans_axis in TranslationAxis::iter() {
        alignment_inputs.push(InputData::new(
            trans_axis.into(),
            id_add_on,
            on_isometry_option_change(iso_sig, AlignmentAxis::Translation(trans_axis)),
            format!(
                "{:.3}",
                iso.translation_of_axis(trans_axis).get::<millimeter>()
            ),
        ));
    }
    alignment_inputs
}

fn get_rotation_alignment_input_data(
    iso: Isometry,
    iso_sig: Signal<Isometry>,
    axes_skip: Option<&Vec<RotationAxis>>,
) -> Vec<InputData> {
    let id_add_on = "inputNodeAlignmentRot";
    let mut alignment_inputs = Vec::<InputData>::new();
    for rot_axis in RotationAxis::iter() {
        if let Some(axes_skip) = axes_skip {
            if axes_skip.contains(&rot_axis) {
                continue;
            }
        }
        alignment_inputs.push(InputData::new(
            rot_axis.into(),
            id_add_on,
            on_isometry_option_change(iso_sig, AlignmentAxis::Rotation(rot_axis)),
            format!("{:.3}", iso.rotation_of_axis(rot_axis).get::<degree>()),
        ));
    }
    alignment_inputs
}
