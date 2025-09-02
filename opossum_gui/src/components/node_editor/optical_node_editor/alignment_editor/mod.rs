#![allow(clippy::derive_partial_eq_without_eq)]

mod grating_alignment;

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        CallbackWrapper,
        accordion::AccordionItem,
        inputs::{
            InputData,
            input_components::{LabeledSelect, RowedInputs},
        },
        node_config_editor::NodeChangeAction,
        optical_node_editor::properties_editor::use_update_signal_with_reactive_prop,
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
    alignment: Isometry,
    node_properties_sig: Signal<Properties>,
    node_type: String,
) -> Element {
    let alignment_sig = use_signal(|| alignment);
    use_context_provider(|| alignment_sig);
    use_update_signal_with_reactive_prop(alignment, alignment_sig);
    let node_config_processor = use_coroutine_handle::<NodeChangeAction>();
    use_effect(move || {
        if *alignment_sig.read() != alignment {
            node_config_processor.send(NodeChangeAction::Alignment(*alignment_sig.read()));
        }
    });

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
pub fn PositioningEditor(
    position_opt: Option<Isometry>,
    node_properties_sig: Signal<Properties>,
    node_type: String,
) -> Element {
    let mut position_opt_sig = use_signal(|| position_opt);
    use_context_provider(|| position_opt_sig);

    use_update_signal_with_reactive_prop(position_opt, position_opt_sig);
    let node_config_processor = use_coroutine_handle::<NodeChangeAction>();

    use_effect(move || {
        if *position_opt_sig.read() != position_opt {
            node_config_processor.send(NodeChangeAction::Isometry(*position_opt_sig.read()));
        }
    });

    let mut accordion_content = Vec::<Result<VNode, RenderError>>::new();
    if node_type != "source" {
        accordion_content.push(rsx! {
        LabeledSelect {
            id: "nodePositioningSelector",
            label: "Position Strategy",
            options: vec![
                (position_opt_sig.read().is_none(), "Relative".to_owned()),
                (position_opt_sig.read().is_some(), "Absolute".to_owned()),
            ],
            onchange: move |_: Event<FormData>| {
                if position_opt_sig.read().is_some() {
                    position_opt_sig.set(None);
                } else {
                    position_opt_sig.set(Some(Isometry::default()));
                }
            },
        }});

        if position_opt_sig.read().is_some() {
            accordion_content.push(rsx! {
                PositioningInputs { position_opt_sig }
            });
        }
    } else {
        accordion_content.push(rsx! {PositioningInputs { position_opt_sig }})
    }

    rsx! {
        AccordionItem {
            elements: accordion_content,
            header: "Position",
            header_id: "positionHeading",
            parent_id: "accordionNodeConfig",
            content_id: "positionCollapse",
        }
    }
}
#[component]
fn PositioningInputs(position_opt_sig: Signal<Option<Isometry>>) -> Element {
    let position_sig = use_signal(|| position_opt_sig.read().unwrap_or_default());
    let node_config_processor = use_coroutine_handle::<NodeChangeAction>();

    use_effect(move || {
        node_config_processor.send(NodeChangeAction::Isometry(Some(*position_sig.read())));
    });

    rsx! {
        RotationAlignmentInputs { alignment_sig: position_sig, axes_skip: None }
        TranslationAlignmentInputs { alignment_sig: position_sig }
    }
}

#[component]
fn TranslationAlignmentInputs(alignment_sig: Signal<Isometry>) -> Element {
    let input_data = get_translation_alignment_input_data(alignment_sig);

    rsx! {
        RowedInputs { inputs: input_data }
    }
}

#[component]
fn RotationAlignmentInputs(
    alignment_sig: Signal<Isometry>,
    axes_skip: Option<Vec<RotationAxis>>,
) -> Element {
    let input_data = get_rotation_alignment_input_data(alignment_sig, axes_skip.as_ref());
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

fn get_translation_alignment_input_data(iso_sig: Signal<Isometry>) -> Vec<InputData> {
    let id_add_on = "inputNodeAlignmentTrans";
    let mut alignment_inputs = Vec::<InputData>::new();
    for trans_axis in TranslationAxis::iter() {
        alignment_inputs.push(InputData::new(
            trans_axis.into(),
            id_add_on,
            on_isometry_option_change(iso_sig, AlignmentAxis::Translation(trans_axis)),
            format!(
                "{:.3}",
                iso_sig
                    .read()
                    .translation_of_axis(trans_axis)
                    .get::<millimeter>()
            ),
        ));
    }
    alignment_inputs
}

fn get_rotation_alignment_input_data(
    iso_sig: Signal<Isometry>,
    axes_skip: Option<&Vec<RotationAxis>>,
) -> Vec<InputData> {
    let id_add_on = "inputNodeAlignmentRot";
    let mut alignment_inputs = Vec::<InputData>::new();
    for rot_axis in RotationAxis::iter() {
        if let Some(axes_skip) = axes_skip
            && axes_skip.contains(&rot_axis)
        {
            continue;
        }
        alignment_inputs.push(InputData::new(
            rot_axis.into(),
            id_add_on,
            on_isometry_option_change(iso_sig, AlignmentAxis::Rotation(rot_axis)),
            format!(
                "{:.3}",
                iso_sig.read().rotation_of_axis(rot_axis).get::<degree>()
            ),
        ));
    }
    alignment_inputs
}
