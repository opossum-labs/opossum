#![allow(clippy::derive_partial_eq_without_eq)]

mod grating_alignment;

use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        accordion::AccordionItem,
        hooks::use_update_signal_with_reactive_prop,
        inputs::{
            InputData,
            input_components::{LabeledSelect, NodeConfigUnitInput, RowedElements, RowedInputs},
        },
        node_config_editor::{NodeChangeAction, NodeChangeEvent},
    },
};
use approx::relative_ne;
use dioxus::prelude::*;
use grating_alignment::GratingAlignmentInputs;
use opossum_core::{
    degree, meter, millimeter, prelude::{Isometry, Properties}, utils::geom_transformation::{AlignmentAxis, RotationAxis, TranslationAxis}
};
use strum::IntoEnumIterator;
use uom::si::{angle::degree, f64::{Angle, Length}, length::millimeter};
use uuid::Uuid;

#[component]
pub fn AlignmentEditor(
    node_id: Uuid,
    alignment: Isometry,
    node_properties_sig: Signal<Properties>,
    node_type: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let alignment_memo = use_memo(use_reactive!(|alignment| alignment));
// 
    let on_save = EventHandler::new(move |new_iso: Isometry| {
        on_change.call(NodeChangeEvent {
            node_id,
            action: NodeChangeAction::Alignment(new_iso),
        });
    });

    let accordion_content = if node_type == "reflective grating" {
        rsx! {
            GratingAlignmentInputs {
                alignment_sig_outside: alignment_memo,
                node_properties_sig,
                on_save,
                node_id,
            }
        }
    } else {
        rsx! {
            RotationAlignmentInputs {
                alignment: alignment_memo,
                axes_skip: None,
                on_new_rotation: on_new_rotation(on_save, alignment_memo.into()),
                node_id,
            }
            TranslationAlignmentInputs {
                alignment: alignment_memo,
                on_new_translation: on_new_translation(on_save, alignment_memo.into()),
                node_id,
            }
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

fn on_new_translation(on_save: EventHandler<Isometry>, alignment: ReadSignal<Isometry>) -> EventHandler<(Length, TranslationAxis)> {
    EventHandler::new(move |(new_trans, axis): (Length, TranslationAxis)| {
        let old_alignment_ax_val = alignment.read().translation_of_axis(axis);
        if relative_ne!(
            old_alignment_ax_val.value, new_trans.value
        ) {
            let mut new_alignment = *alignment.read();
            if new_alignment
                                        .set_translation_of_axis(axis, new_trans)
                .is_ok()
            {
                on_save.call(new_alignment);
            }
            else{
                OPOSSUM_UI_LOGS.write().add_log(
                    format!("Failed to set alignment for axis {axis}!",)
                        .as_str(),
                );
            }
        }
    })
}


fn on_new_rotation(on_save: EventHandler<Isometry>, alignment: ReadSignal<Isometry>) -> EventHandler<(Angle, RotationAxis)> {
    EventHandler::new(move |(new_rot, axis): (Angle, RotationAxis)| {
        let old_alignment_ax_val = alignment.read().rotation_of_axis(axis);
        if relative_ne!(
            old_alignment_ax_val.get::<degree>(), new_rot.get::<degree>()
        ) {
            let mut new_alignment = *alignment.read();
            if new_alignment
                                        .set_rotation_of_axis(axis, new_rot)
                .is_ok()
            {
                on_save.call(new_alignment);
            }
            else{
                OPOSSUM_UI_LOGS.write().add_log(
                    format!("Failed to set alignment for axis {axis}!",)
                        .as_str(),
                );
            }
        }
    })
}

#[component]
pub fn PositioningEditor(
    node_id: Uuid,
    position_opt: Option<Isometry>,
    node_properties_sig: Signal<Properties>,
    node_type: String,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let mut position_opt_sig = use_signal(|| position_opt);
    use_context_provider(|| position_opt_sig);
    use_update_signal_with_reactive_prop(position_opt, position_opt_sig);

    let on_save = EventHandler::new(move |new_iso: Isometry| {
        position_opt_sig.set(Some(new_iso));
        on_change.call(NodeChangeEvent {
            node_id,
            action: NodeChangeAction::Isometry(Some(new_iso)),
        });
    });

    let mut accordion_content = Vec::<Result<VNode, RenderError>>::new();
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
                    on_change
                        .call(NodeChangeEvent {
                            node_id,
                            action: NodeChangeAction::Isometry(None),
                        });
                } else {
                    let new_iso = Isometry::default();
                    position_opt_sig.set(Some(new_iso));
                    on_change
                        .call(NodeChangeEvent {
                            node_id,
                            action: NodeChangeAction::Isometry(Some(new_iso)),
                        });
                }
            },
        }
    });

    if position_opt_sig.read().is_some() {
        accordion_content.push(rsx! {
            PositioningInputs { position_opt_sig, on_save, node_id }
        });
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
fn PositioningInputs(
    position_opt_sig: Signal<Option<Isometry>>,
    on_save: EventHandler<Isometry>,
    node_id: Uuid
) -> Element {
    let mut position_sig = use_signal(|| position_opt_sig.read().unwrap_or_default());

    use_effect(move || {
        #[allow(clippy::collapsible_if)]
        if let Some(iso) = position_opt_sig.read().as_ref() {
            if *position_sig.peek() != *iso {
                position_sig.set(*iso);
            }
        }
    });

    rsx! {
        RotationAlignmentInputs {
            alignment: position_sig,
            axes_skip: None,
            on_new_rotation: on_new_rotation(on_save, position_sig.into()),
            node_id,
        }
        TranslationAlignmentInputs {
            alignment: position_sig,
            on_new_translation: on_new_translation(on_save, position_sig.into()),
            node_id,
        }
    }
}

#[component]
fn TranslationAlignmentInputs(
    alignment: ReadSignal<Isometry>,
    on_new_translation: EventHandler<(Length, TranslationAxis)>,
    node_id: Uuid
) -> Element {
    let id_add_on = "inputNodeAlignmentTrans";

    let mut x_sig = use_signal(move || alignment.read().translation_of_axis(TranslationAxis::X).value);
    let mut y_sig = use_signal(move || alignment.read().translation_of_axis(TranslationAxis::Y).value);
    let mut z_sig = use_signal(move || alignment.read().translation_of_axis(TranslationAxis::Z).value);

    use_update_signal_with_reactive_prop(alignment.read().translation_of_axis(TranslationAxis::X).value, x_sig);
    use_update_signal_with_reactive_prop(alignment.read().translation_of_axis(TranslationAxis::Y).value, y_sig);
    use_update_signal_with_reactive_prop(alignment.read().translation_of_axis(TranslationAxis::Z).value, z_sig);

    rsx!{
        div { class: "row gy-1 gx-2",
            div { class: "col-sm",
                NodeConfigUnitInput {
                    id: format!("{id_add_on}{}{}", TranslationAxis::X, node_id.as_simple().to_string()),
                    label: format!("{} translation", TranslationAxis::X),
                    value: x_sig,
                    base_unit: "m",
                    onchange: move |new_trans: f64| {
                        x_sig.set(new_trans);
                        on_new_translation.call((meter!(new_trans), TranslationAxis::X));
                    },
                }
            }
            div { class: "col-sm",
                NodeConfigUnitInput {
                    id: format!("{id_add_on}{}", TranslationAxis::Y),
                    label: format!("{} translation", TranslationAxis::Y),
                    value: y_sig,
                    base_unit: "m",
                    onchange: move |new_trans: f64| {
                        y_sig.set(new_trans);
                        on_new_translation.call((meter!(new_trans), TranslationAxis::Y));
                    },
                }
            }
        }
        NodeConfigUnitInput {
            id: format!("{id_add_on}{}", TranslationAxis::Z),
            label: format!("{} translation", TranslationAxis::Z),
            value: z_sig,
            base_unit: "m",
            onchange: move |new_trans: f64| {
                z_sig.set(new_trans);
                on_new_translation.call((meter!(new_trans), TranslationAxis::Z));
            },
        }
    }
}

#[component]
fn RotationAlignmentInputs(
    alignment: ReadSignal<Isometry>,
    axes_skip: Option<Vec<RotationAxis>>,
    on_new_rotation: EventHandler<(Angle, RotationAxis)>,
    node_id: Uuid
) -> Element {

    let id_add_on = "inputNodeAlignmentRot";

    let mut rot_input_vec = Vec::<Element>::new();

    for rot_axis in RotationAxis::iter() {
        if let Some(ref axes_skip) = axes_skip
            && axes_skip.contains(&rot_axis)
        {
            continue;
        }
        rot_input_vec.push(rsx!{
            RotationInput {
                alignment,
                axis: rot_axis,
                id: format!("{id_add_on}{}{}", rot_axis, node_id.as_simple().to_string()),
                on_new_rotation: on_new_rotation.clone(),
            }
        });
    }
    rsx!{
        RowedElements { elements: rot_input_vec, num_per_row: 2 }
    }
}

#[component]
pub fn RotationInput(alignment: ReadSignal<Isometry>, axis: RotationAxis, id: String, on_new_rotation: EventHandler<(Angle, RotationAxis)>) -> Element{
    let mut value_sig = use_signal(move || alignment.read().rotation_of_axis(axis).get::<degree>());
    use_update_signal_with_reactive_prop(alignment.read().rotation_of_axis(axis).get::<degree>(), value_sig);

    rsx!{
        NodeConfigUnitInput {
            id,
            label: format!("{} rotation", axis),
            value: value_sig,
            base_unit: "°",
            onchange: move |new_rot: f64| {
                on_new_rotation.call((degree!(new_rot), axis));
            },
        }
    }
}

fn on_isometry_option_change_str(
    mut iso_sig: Signal<Isometry>,
    axis_type: AlignmentAxis,
    on_save: EventHandler<Isometry>,
) -> EventHandler<String> {
    EventHandler::new(move |val_str: String| {
        if let Ok(val) = val_str.parse::<f64>() {
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
                    on_save.call(iso);
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
    iso_sig: Signal<Isometry>,
    on_save: EventHandler<Isometry>,
) -> Vec<InputData> {
    let id_add_on = "inputNodeAlignmentTrans";
    let mut alignment_inputs = Vec::<InputData>::new();

    for trans_axis in TranslationAxis::iter() {
        alignment_inputs.push(InputData::new(
            trans_axis.into(),
            id_add_on,
            EventHandler::new(|_| {}),
            on_isometry_option_change_str(iso_sig, AlignmentAxis::Translation(trans_axis), on_save),
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
    on_save: EventHandler<Isometry>,
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
            EventHandler::new(|_| {}),
            on_isometry_option_change_str(iso_sig, AlignmentAxis::Rotation(rot_axis), on_save),
            format!(
                "{:.3}",
                iso_sig.read().rotation_of_axis(rot_axis).get::<degree>()
            ),
        ));
    }
    alignment_inputs
}
