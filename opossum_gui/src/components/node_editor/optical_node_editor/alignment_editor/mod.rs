#![allow(clippy::derive_partial_eq_without_eq)]

mod grating_alignment;
use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        accordion::{AccordionItem, ElementList},
        inputs::input_components::{
            LabeledSelect, NodeConfigUnitInput, RowedElements, UnitHandling,
        },
        node_config_editor::{NodeChangeAction, NodeChangeEvent},
        optical_node_editor::alignment_editor::grating_alignment::GratingAlignmentInputs,
    },
};
use approx::relative_ne;
use dioxus::prelude::*;
// use grating_alignment::GratingAlignmentInputs;
use opossum_core::{
    degree, meter,
    prelude::{Isometry, Properties},
    types::api_types::NodeInfo,
    utils::geom_transformation::{RotationAxis, TranslationAxis},
};
use strum::IntoEnumIterator;
use uom::si::{
    angle::degree,
    f64::{Angle, Length},
};
use uuid::Uuid;

#[component]
pub fn AlignmentEditor(
    node_id: Memo<Uuid>,
    node_info: ReadSignal<NodeInfo>,
    node_properties_sig: ReadSignal<Properties>,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let accordion_content = if node_info.read().uuid == *node_id.read() {
        vec![rsx! {
            AlignmentInputs {
                node_id,
                alignment: node_info.read().alignment.unwrap_or_default(),
                node_type: node_info.read().node_type.clone(),
                on_change,
                node_properties_sig,
                readonly
            }
        }]
    } else {
        vec![]
    };
    rsx! {
        AccordionItem {
            elements: accordion_content,
            header: "Alignment",
            header_id: "alignmentHeading",
            parent_id: "accordionNodeConfig",
            content_id: "alignmentCollapse",
            level: 1,
        }
    }
}

#[component]
pub fn AlignmentInputs(
    node_id: Memo<Uuid>,
    alignment: Isometry,
    node_type: String,
    node_properties_sig: ReadSignal<Properties>,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let mut alignment_sig = use_signal(|| alignment);
    let on_save = EventHandler::new(move |new_iso: Isometry| {
        on_change.call(NodeChangeEvent {
            node_id: *node_id.read(),
            action: NodeChangeAction::Alignment(new_iso),
        });
        alignment_sig.set(new_iso);
    });

    if node_type == "reflective grating" {
        rsx! {
            GratingAlignmentInputs {
                alignment_sig_outside: alignment_sig,
                node_properties_sig,
                on_save,
                node_id,
                readonly,
            }
        }
    } else {
        rsx! {
            RotationAlignmentInputs {
                alignment: alignment_sig,
                axes_skip: None,
                on_new_rotation: on_new_rotation(on_save, alignment_sig.into()),
                node_id,
                readonly,
            }
            TranslationAlignmentInputs {
                alignment: alignment_sig,
                axes_skip: None,
                on_new_translation: on_new_translation(on_save, alignment_sig.into()),
                node_id,
                readonly,
            }
        }
    }
}

pub fn on_new_translation(
    on_save: EventHandler<Isometry>,
    alignment: ReadSignal<Isometry>,
) -> EventHandler<(Length, TranslationAxis)> {
    EventHandler::new(move |(new_trans, axis): (Length, TranslationAxis)| {
        let old_alignment_ax_val = alignment.read().translation_of_axis(axis);
        if relative_ne!(old_alignment_ax_val.value, new_trans.value, epsilon = 0.0) {
            let mut new_alignment = *alignment.read();
            if new_alignment
                .set_translation_of_axis(axis, new_trans)
                .is_ok()
            {
                on_save.call(new_alignment);
            } else {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log(format!("Failed to set alignment for axis {axis}!").as_str());
            }
        }
    })
}

pub fn on_new_rotation(
    on_save: EventHandler<Isometry>,
    alignment: ReadSignal<Isometry>,
) -> EventHandler<(Angle, RotationAxis)> {
    EventHandler::new(move |(new_rot, axis): (Angle, RotationAxis)| {
        let old_alignment_ax_val = alignment.read().rotation_of_axis(axis);
        if relative_ne!(
            old_alignment_ax_val.get::<degree>(),
            new_rot.get::<degree>()
        ) {
            let mut new_alignment = *alignment.read();
            if new_alignment.set_rotation_of_axis(axis, new_rot).is_ok() {
                on_save.call(new_alignment);
            } else {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log(format!("Failed to set alignment for axis {axis}!").as_str());
            }
        }
    })
}

#[component]
pub fn PositioningEditor(
    node_id: Memo<Uuid>,
    node_info: ReadSignal<NodeInfo>,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    let accordion_content = if node_info.read().uuid == *node_id.read() {
        let position_opt = node_info.read().isometry;
        vec![rsx! {
            PositioningInputs{
                position_opt,
                on_change,
                node_id,
                readonly
            }
        }]
    } else {
        vec![]
    };
    rsx! {
        AccordionItem {
            elements: accordion_content,
            header: "Position",
            header_id: "positionHeading",
            parent_id: "accordionNodeConfig",
            content_id: "positionCollapse",
            level: 1,
        }
    }
}

#[component]
pub fn PositioningInputs(
    position_opt: Option<Isometry>,
    on_change: EventHandler<NodeChangeEvent>,
    node_id: Memo<Uuid>,
    readonly: bool,
) -> Element {
    let mut position_opt_sig = use_signal(|| position_opt);
    let position_memo = use_memo(move || position_opt_sig.read().unwrap_or_default());
    let mut last_absolute_position = use_signal(|| position_opt.unwrap_or_default());

    let on_save = EventHandler::new(move |new_iso_opt: Option<Isometry>| {
        on_change.call(NodeChangeEvent {
            node_id: *node_id.read(),
            action: NodeChangeAction::Isometry(new_iso_opt),
        });
        position_opt_sig.set(new_iso_opt);
    });
    let on_position_change = EventHandler::new(move |new_iso: Isometry| {
        on_save.call(Some(new_iso));
    });

    use_effect(move || {
        let current_val = *position_opt_sig.read();
        if let Some(abs_pos) = current_val {
            last_absolute_position.set(abs_pos);
        }
    });

    let mut element_list = vec![rsx! {
    LabeledSelect {
            id: "nodePositioningSelector",
            label: "Position Strategy",
            options: vec![
                (position_opt_sig.read().is_none(), "Relative".to_owned()),
                (position_opt_sig.read().is_some(), "Absolute".to_owned()),
            ],
            readonly,
            onchange: move |e: Event<FormData>| {
                if e.data.value() == "Relative" {
                    on_save.call(None);
                }
                else{
                    on_save.call(Some(*last_absolute_position.read()));
                }
            },
        }
    }];

    if position_opt_sig.read().is_some() {
        element_list.push(rsx! {
            RotationAlignmentInputs {
                alignment: position_memo,
                axes_skip: None,
                on_new_rotation: on_new_rotation(on_position_change, position_memo.into()),
                node_id,
                readonly,
            }
            TranslationAlignmentInputs {
                alignment: position_memo,
                axes_skip: None,
                on_new_translation: on_new_translation(on_position_change, position_memo.into()),
                node_id,
                readonly,
            }
        });
    }
    rsx! {
        ElementList { element_list }
    }
}

#[component]
pub fn TranslationAlignmentInputs(
    alignment: ReadSignal<Isometry>,
    axes_skip: Option<Vec<TranslationAxis>>,
    on_new_translation: EventHandler<(Length, TranslationAxis)>,
    node_id: Memo<Uuid>,
    readonly: bool,
) -> Element {
    let id_add_on = "inputNodeAlignmentTrans";

    let mut trans_input_vec = Vec::<Element>::new();

    for trans_axis in TranslationAxis::iter() {
        if let Some(ref axes_skip) = axes_skip
            && axes_skip.contains(&trans_axis)
        {
            continue;
        }
        trans_input_vec.push(rsx! {
            TranslationInput {
                alignment,
                axis: trans_axis,
                id: format!("{id_add_on}{}{}", trans_axis, node_id.read().as_simple().to_string()),
                on_new_translation,
                readonly,
            }
        });
    }
    rsx! {
        RowedElements { elements: trans_input_vec, num_per_row: 2 }
    }
}

#[component]
pub fn TranslationInput(
    alignment: ReadSignal<Isometry>,
    axis: TranslationAxis,
    id: String,
    on_new_translation: EventHandler<(Length, TranslationAxis)>,
    readonly: bool,
) -> Element {
    let value_memo = use_memo(move || {
        let translation = alignment.read().translation_of_axis(axis);
        if translation.value.abs() < f64::EPSILON {
            0.
        } else {
            translation.value
        }
    });

    rsx! {
        NodeConfigUnitInput {
            id,
            label: format!("{} translation", axis),
            value: value_memo,
            unit_config: UnitHandling::new("m", true),
            readonly,
            onchange: move |new_trans: f64| {
                on_new_translation.call((meter!(new_trans), axis));
            },
        }
    }
}

#[component]
pub fn RotationAlignmentInputs(
    alignment: ReadSignal<Isometry>,
    axes_skip: Option<Vec<RotationAxis>>,
    on_new_rotation: EventHandler<(Angle, RotationAxis)>,
    node_id: Memo<Uuid>,
    readonly: bool,
) -> Element {
    let id_add_on = "inputNodeAlignmentRot";

    let mut rot_input_vec = Vec::<Element>::new();

    for rot_axis in RotationAxis::iter() {
        if let Some(ref axes_skip) = axes_skip
            && axes_skip.contains(&rot_axis)
        {
            continue;
        }
        rot_input_vec.push(rsx! {
            RotationInput {
                alignment,
                axis: rot_axis,
                id: format!("{id_add_on}{}{}", rot_axis, node_id.read().as_simple().to_string()),
                on_new_rotation,
                readonly,
            }
        });
    }
    rsx! {
        RowedElements { elements: rot_input_vec, num_per_row: 2 }
    }
}

#[component]
pub fn RotationInput(
    alignment: ReadSignal<Isometry>,
    axis: RotationAxis,
    id: String,
    on_new_rotation: EventHandler<(Angle, RotationAxis)>,
    readonly: bool,
) -> Element {
    let value_memo = use_memo(move || {
        let angle = alignment.read().rotation_of_axis(axis);
        if angle.value.abs() < f64::EPSILON {
            0.
        } else {
            angle.get::<degree>()
        }
    });

    rsx! {
        NodeConfigUnitInput {
            id,
            label: format!("{} rotation", axis),
            value: value_memo,
            unit_config: UnitHandling::new("°", true),
            readonly,
            onchange: move |new_rot: f64| {
                on_new_rotation.call((degree!(new_rot), axis));
            },
        }
    }
}
