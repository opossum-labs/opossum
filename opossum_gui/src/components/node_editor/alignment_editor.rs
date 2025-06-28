#![allow(clippy::derive_partial_eq_without_eq)]
use crate::{
    components::node_editor::{
        accordion::{AccordionItem, LabeledInput},
        node_editor_component::NodeChange,
    },
    OPOSSUM_UI_LOGS,
};
use dioxus::prelude::*;
use opossum_backend::{degree, millimeter, Isometry, RotationAxis, TranslationAxis};
use uom::si::{angle::degree, length::millimeter};

#[component]
pub fn AlignmentEditor(alignment: Option<Isometry>) -> Element {
    let iso = Signal::new(alignment.unwrap_or(Isometry::identity()));
    let accordion_content = vec![rsx! {
        NodeAlignmentTranslationInput {
            iso,
            axis: TranslationAxis::X,
        }
        NodeAlignmentTranslationInput {
            iso,
            axis: TranslationAxis::Y,
        }
        NodeAlignmentTranslationInput {
            iso,
            axis: TranslationAxis::Z,
        }
        NodeAlignmentRotationInput {
            iso,
            axis: RotationAxis::Roll,
        }
        NodeAlignmentRotationInput {
            iso,
            axis: RotationAxis::Pitch,
        }
        NodeAlignmentRotationInput {
            iso,
            axis: RotationAxis::Yaw,
        }
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

#[component]
pub fn NodeAlignmentTranslationInput(iso: Signal<Isometry>, axis: TranslationAxis) -> Element {
    let node_change_signal = use_context::<Signal<Option<NodeChange>>>();

    rsx! {
        LabeledInput {
            id: format!("inputNodeTranslation{axis}"),
            label: format!("{} translation in mm", axis),
            value: format!("{:.3}", iso.read().translation_of_axis(axis).get::<millimeter>()),
            r#type: "number",
            onchange: Some(translation_onchange(node_change_signal, iso, axis)),
        }
    }
}

fn translation_onchange(
    mut node_change: Signal<Option<NodeChange>>,
    mut iso_sig: Signal<Isometry>,
    axis: TranslationAxis,
) -> Callback<Event<FormData>> {
    use_callback(move |e: Event<FormData>| {
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

#[component]
pub fn NodeAlignmentRotationInput(iso: Signal<Isometry>, axis: RotationAxis) -> Element {
    let node_change_signal = use_context::<Signal<Option<NodeChange>>>();

    rsx! {
        LabeledInput {
            id: format!("inputNodeRotation{axis}"),
            label: format!("{} rotation in degrees", axis),
            value: format!("{:.3}", iso.read().rotation_of_axis(axis).get::<degree>()),
            r#type: "number",
            onchange: Some(rotation_onchange(node_change_signal, iso, axis)),
        }
    }
}

fn rotation_onchange(
    mut node_change: Signal<Option<NodeChange>>,
    mut iso_sig: Signal<Isometry>,
    axis: RotationAxis,
) -> Callback<Event<FormData>> {
    use_callback(move |e: Event<FormData>| {
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
