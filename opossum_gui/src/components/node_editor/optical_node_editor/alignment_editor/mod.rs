#![allow(clippy::derive_partial_eq_without_eq)]

mod grating_alignment;
use crate::{
    OPOSSUM_UI_LOGS,
    components::node_editor::{
        accordion::{AccordionItem, ElementList, content_id_for_panel},
        hooks::use_synced_signal,
        inputs::input_components::{
            LabeledSelect, NodeConfigUnitInput, RowedElements, UnitHandling,
        },
        node_config_editor::{NodeChangeAction, NodeChangeEvent},
        optical_node_editor::alignment_editor::grating_alignment::GratingAlignmentInputs,
    },
};
use approx::relative_ne;
use dioxus::prelude::*;
use opossum_core::{
    degree, meter,
    prelude::{Isometry, Properties},
    types::api_types::{NodeEditorPanel, NodeInfo},
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
    alignment: Memo<Isometry>,
    node_type: Memo<String>,
    node_properties_sig: ReadSignal<Properties>,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    info!("🔄 Render: AlignmentEditor");
    let current_node_id = *node_id.read();
    let accordion_content = if current_node_id != Uuid::nil() {
        vec![rsx! {
            AlignmentInputs {
                node_id,
                alignment: *alignment.read(),
                node_type: node_type.read().clone(),
                on_change,
                node_properties_sig,
                readonly,
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
            content_id: content_id_for_panel(NodeEditorPanel::Alignment),
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
    let mut alignment_sig = use_synced_signal(alignment);
    
    // Stable save callback for alignment changes
    let on_save = use_callback(move |new_iso: Isometry| {
        on_change.call(NodeChangeEvent {
            node_id: *node_id.peek(),
            action: NodeChangeAction::Alignment(new_iso),
        });
        alignment_sig.set(new_iso);
    });

    // Stable callback replacing the global on_new_rotation helper
    let on_rotation_change = use_callback(move |(new_rot, axis): (Angle, RotationAxis)| {
        let current_iso = *alignment_sig.peek();
        let old_angle = current_iso.rotation_of_axis(axis);
        if relative_ne!(old_angle.get::<degree>(), new_rot.get::<degree>()) {
            let mut new_iso = current_iso;
            if new_iso.set_rotation_of_axis(axis, new_rot).is_ok() {
                on_save(new_iso);
            } else {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log(&format!("Failed to set alignment for axis {axis}!"));
            }
        }
    });

    // Stable callback replacing the global on_new_translation helper
    let on_translation_change = use_callback(move |(new_trans, axis): (Length, TranslationAxis)| {
        let current_iso = *alignment_sig.peek();
        let old_trans = current_iso.translation_of_axis(axis);
        if relative_ne!(old_trans.value, new_trans.value, epsilon = 0.0) {
            let mut new_iso = current_iso;
            if new_iso.set_translation_of_axis(axis, new_trans).is_ok() {
                on_save(new_iso);
            } else {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log(&format!("Failed to set alignment for axis {axis}!"));
            }
        }
    });

    if node_type == "reflective grating" {
        // Create an EventHandler wrapper solely for GratingAlignmentInputs (which expects an EventHandler)
        let on_save_handler = EventHandler::new(move |iso| on_save(iso));
        rsx! {
            GratingAlignmentInputs {
                alignment_sig_outside: alignment_sig,
                node_properties_sig,
                on_save: on_save_handler,
                node_id,
                readonly,
            }
        }
    } else {
        rsx! {
            RotationAlignmentInputs {
                alignment: alignment_sig,
                axes_skip: None,
                on_new_rotation: on_rotation_change,
                node_id,
                readonly,
            }
            TranslationAlignmentInputs {
                alignment: alignment_sig,
                axes_skip: None,
                on_new_translation: on_translation_change,
                node_id,
                readonly,
            }
        }
    }
}

#[component]
pub fn PositioningEditor(
    node_id: Memo<Uuid>,
    position_opt: Memo<Option<Isometry>>,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    info!("🔄 Render: PositioningEditor");
    let current_node_id = *node_id.read();
    let accordion_content = if current_node_id != Uuid::nil() {
        let current_position = *position_opt.read();
        vec![rsx! {
            PositioningInputs {
                position_opt: current_position,
                on_change,
                node_id,
                readonly,
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
            content_id: content_id_for_panel(NodeEditorPanel::Positioning),
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
    info!("🔄 Render: PositioningInputs");

    let mut position_opt_sig = use_synced_signal(position_opt);
    let position_memo = use_memo(move || position_opt_sig.read().unwrap_or_default());
    let mut last_absolute_position = use_signal(|| position_opt.unwrap_or_default());

    use_effect(move || {
        if let Some(abs_pos) = *position_opt_sig.read() {
            if *last_absolute_position.peek() != abs_pos {
                last_absolute_position.set(abs_pos);
            }
        }
    });

    let is_absolute = position_opt_sig.read().is_some();

    // Stable save callback for positioning updates
    let on_save = use_callback(move |new_iso_opt: Option<Isometry>| {
        if let Some(abs) = new_iso_opt {
            last_absolute_position.set(abs);
        }
        position_opt_sig.set(new_iso_opt);
        on_change.call(NodeChangeEvent {
            node_id: *node_id.peek(),
            action: NodeChangeAction::Isometry(new_iso_opt),
        });
    });

    // Stable strategy change callback - FIX FOR PANIC: Uses peek() to avoid active borrow locks!
    let on_strategy_change = use_callback(move |e: Event<FormData>| {
        if e.data.value() == "Relative" {
            on_save(None);
        } else {
            let abs_pos = *last_absolute_position.peek();
            on_save(Some(abs_pos));
        }
    });

    // Stable rotation callback (replaces dynamic on_new_rotation helper)
    let on_rotation_change = use_callback(move |(new_rot, axis): (Angle, RotationAxis)| {
        let current_iso = position_opt_sig.peek().unwrap_or_default();
        let old_angle = current_iso.rotation_of_axis(axis);
        if relative_ne!(old_angle.get::<degree>(), new_rot.get::<degree>()) {
            let mut new_iso = current_iso;
            if new_iso.set_rotation_of_axis(axis, new_rot).is_ok() {
                on_save(Some(new_iso));
            } else {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log(&format!("Failed to set position rotation for axis {axis}!"));
            }
        }
    });

    // Stable translation callback (replaces dynamic on_new_translation helper)
    let on_translation_change = use_callback(move |(new_trans, axis): (Length, TranslationAxis)| {
        let current_iso = position_opt_sig.peek().unwrap_or_default();
        let old_trans = current_iso.translation_of_axis(axis);
        if relative_ne!(old_trans.value, new_trans.value, epsilon = 0.0) {
            let mut new_iso = current_iso;
            if new_iso.set_translation_of_axis(axis, new_trans).is_ok() {
                on_save(Some(new_iso));
            } else {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log(&format!("Failed to set position translation for axis {axis}!"));
            }
        }
    });

    // Memoize the options so LabeledSelect props remain strictly identical
    let strategy_options = use_memo(move || {
        let is_abs = position_opt_sig.read().is_some();
        vec![
            (!is_abs, "Relative".to_owned()),
            (is_abs, "Absolute".to_owned()),
        ]
    });

    let mut element_list = vec![rsx! {
        LabeledSelect {
            id: "nodePositioningSelector".to_string(),
            label: "Position Strategy".to_string(),
            options: strategy_options.read().clone(),
            readonly,
            onchange: on_strategy_change,
        }
    }];

    if is_absolute {
        element_list.push(rsx! {
            RotationAlignmentInputs {
                alignment: position_memo,
                axes_skip: None,
                on_new_rotation: on_rotation_change,
                node_id,
                readonly,
            }
            TranslationAlignmentInputs {
                alignment: position_memo,
                axes_skip: None,
                on_new_translation: on_translation_change,
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

// -------------------------------------------------------------------------------------------------
// LEGACY GLOBAL HELPERS: Preserved solely so we do not break any imports in parent modules like 
// `optical_node_editor/mod.rs` which may still be trying to `pub(super) use` them. They are no 
// longer actively called in the render paths inside this file to ensure proper memoization.
// -------------------------------------------------------------------------------------------------

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