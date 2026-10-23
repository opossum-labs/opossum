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
    core_optics::node_attr::NodePositioning,
    degree, meter,
    prelude::{Isometry, Properties},
    types::api_types::NodeEditorPanel,
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
    node_id: ReadSignal<Uuid>,
    alignment: ReadSignal<Isometry>,
    node_type: ReadSignal<String>,
    node_properties: ReadSignal<Properties>,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    debug!("🔄 Render: AlignmentEditor");
    let current_node_id = *node_id.read();

    let accordion_content = if current_node_id == Uuid::nil() {
        vec![]
    } else {
        vec![rsx! {
            AlignmentInputs {
                node_id,
                alignment,
                node_type,
                on_change,
                node_properties,
                readonly,
            }
        }]
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
    node_id: ReadSignal<Uuid>,
    alignment: ReadSignal<Isometry>, // Now receives a ReadSignal instead of raw Isometry
    node_type: ReadSignal<String>,   // Now receives a ReadSignal instead of raw String
    node_properties: ReadSignal<Properties>,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    // Sync the signal only when the external alignment prop value actually changes
    let mut alignment_sig = use_synced_signal(*alignment.read());

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
    let on_translation_change =
        use_callback(move |(new_trans, axis): (Length, TranslationAxis)| {
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

    let current_node_type = node_type.read().clone();

    if current_node_type == "reflective grating" {
        // Create an EventHandler wrapper solely for GratingAlignmentInputs
        #[allow(clippy::redundant_closure)] // false positive by linter
        let on_save_handler = EventHandler::new(move |iso| on_save(iso));
        rsx! {
            GratingAlignmentInputs {
                alignment_sig_outside: alignment_sig,
                node_properties,
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
    node_id: ReadSignal<Uuid>,
    position: ReadSignal<NodePositioning>,
    on_change: EventHandler<NodeChangeEvent>,
    readonly: bool,
) -> Element {
    debug!("🔄 Render: PositioningEditor");
    let current_node_id = *node_id.read();
    let accordion_content = if current_node_id == Uuid::nil() {
        vec![]
    } else {
        vec![rsx! {
            PositioningInputs {
                position,
                on_change,
                node_id,
                readonly,
            }
        }]
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
    position: ReadSignal<NodePositioning>,
    on_change: EventHandler<NodeChangeEvent>,
    node_id: ReadSignal<Uuid>,
    readonly: bool,
) -> Element {
    debug!("🔄 Render: PositioningInputs");

    // Synchronize local state whenever the external ReadSignal updates
    let mut positioning_sig = use_synced_signal(*position.read());

    // Extract the effective isometry for display purposes
    let initial_isometry = match *position.peek() {
        NodePositioning::Absolute(iso) => iso,
        NodePositioning::Automatic(cached) => cached.unwrap_or_default(),
    };
    let mut last_absolute_position = use_signal(|| initial_isometry);

    // Update last known isometry whenever an absolute or cached automatic position is available
    use_effect(move || {
        let current_pos = *positioning_sig.read();
        if let Some(iso) = current_pos.effective_position()
            && *last_absolute_position.peek() != *iso
        {
            last_absolute_position.set(*iso);
        }
    });

    // Determine current strategy states
    let current_pos_val = positioning_sig.read();
    let is_absolute = matches!(*current_pos_val, NodePositioning::Absolute(_));
    let has_cached_auto = matches!(*current_pos_val, NodePositioning::Automatic(Some(_)));

    // Memoize the isometry passed to rotation and translation inputs
    let position_memo = use_memo(move || {
        positioning_sig
            .read()
            .effective_position()
            .copied()
            .unwrap_or_else(|| *last_absolute_position.read())
    });

    // Stable save callback dispatching changes to the parent handler
    let on_save = use_callback(move |new_positioning: NodePositioning| {
        if let NodePositioning::Absolute(iso) = new_positioning {
            last_absolute_position.set(iso);
        }
        positioning_sig.set(new_positioning);
        on_change.call(NodeChangeEvent {
            node_id: *node_id.peek(),
            action: NodeChangeAction::Isometry(new_positioning),
        });
    });

    // Strategy selector callback switching between Automatic and Absolute modes
    let on_strategy_change = use_callback(move |e: Event<FormData>| {
        let selected_val = e.data.value();
        if selected_val == "Automatic" {
            // Reset back to uncalculated/default automatic positioning
            on_save(NodePositioning::Automatic(None));
        } else {
            let abs_pos = *last_absolute_position.peek();
            on_save(NodePositioning::Absolute(abs_pos));
        }
    });

    // Rotation change handler updating the absolute isometry
    let on_rotation_change = use_callback(move |(new_rot, axis): (Angle, RotationAxis)| {
        let current_iso = positioning_sig
            .peek()
            .effective_position()
            .copied()
            .unwrap_or_default();
        let old_angle = current_iso.rotation_of_axis(axis);
        if relative_ne!(old_angle.get::<degree>(), new_rot.get::<degree>()) {
            let mut new_iso = current_iso;
            if new_iso.set_rotation_of_axis(axis, new_rot).is_ok() {
                on_save(NodePositioning::Absolute(new_iso));
            } else {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log(&format!("Failed to set position rotation for axis {axis}!"));
            }
        }
    });

    // Translation change handler updating the absolute isometry
    let on_translation_change =
        use_callback(move |(new_trans, axis): (Length, TranslationAxis)| {
            let current_iso = positioning_sig
                .peek()
                .effective_position()
                .copied()
                .unwrap_or_default();
            let old_trans = current_iso.translation_of_axis(axis);
            if relative_ne!(old_trans.value, new_trans.value, epsilon = 0.0) {
                let mut new_iso = current_iso;
                if new_iso.set_translation_of_axis(axis, new_trans).is_ok() {
                    on_save(NodePositioning::Absolute(new_iso));
                } else {
                    OPOSSUM_UI_LOGS.write().add_log(&format!(
                        "Failed to set position translation for axis {axis}!"
                    ));
                }
            }
        });

    // Memoize options to keep LabeledSelect properties stable across renders
    let strategy_options = use_memo(move || {
        let is_abs = matches!(*positioning_sig.read(), NodePositioning::Absolute(_));
        vec![
            (!is_abs, "Automatic".to_owned()),
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

    // Render inputs if absolute, or show them as grayed out (readonly) if automatic has a cached position
    if is_absolute || has_cached_auto {
        // If it's automatic with a cached position, force readonly to true so it appears grayed out
        let inputs_readonly = readonly || has_cached_auto;

        element_list.push(rsx! {
            RotationAlignmentInputs {
                alignment: position_memo,
                axes_skip: None,
                on_new_rotation: on_rotation_change,
                node_id,
                readonly: inputs_readonly,
            }
            TranslationAlignmentInputs {
                alignment: position_memo,
                axes_skip: None,
                on_new_translation: on_translation_change,
                node_id,
                readonly: inputs_readonly,
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
    node_id: ReadSignal<Uuid>,
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
                key: "{trans_axis}",
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
    let on_change_callback = use_callback(move |new_trans: f64| {
        on_new_translation.call((meter!(new_trans), axis));
    });
    rsx! {
        NodeConfigUnitInput {
            id,
            label: format!("{} translation", axis),
            value: value_memo,
            unit_config: UnitHandling::new("m", true),
            readonly,
            onchange: on_change_callback,
        }
    }
}

#[component]
pub fn RotationAlignmentInputs(
    alignment: ReadSignal<Isometry>,
    axes_skip: Option<Vec<RotationAxis>>,
    on_new_rotation: EventHandler<(Angle, RotationAxis)>,
    node_id: ReadSignal<Uuid>,
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
                key: "{rot_axis}",
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
    let on_change_callback = use_callback(move |new_rot: f64| {
        on_new_rotation.call((degree!(new_rot), axis));
    });

    rsx! {
        NodeConfigUnitInput {
            id,
            label: format!("{} rotation", axis),
            value: value_memo,
            unit_config: UnitHandling::new("°", true),
            readonly,
            onchange: on_change_callback,
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
