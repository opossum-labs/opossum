#![allow(clippy::derive_partial_eq_without_eq)]
pub mod alignment_editor;
pub mod general_editor;
pub mod properties_editor;

use crate::components::node_editor::optical_node_editor::alignment_editor::{
    AlignmentEditor, PositioningEditor,
};
use crate::components::node_editor::optical_node_editor::general_editor::GeneralEditor;
use crate::components::node_editor::optical_node_editor::properties_editor::PropertiesEditor;
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use opossum_core::prelude::{Isometry, Properties};
use uuid::Uuid;

#[component]
pub fn OpticalNodeEditor(node_id: Uuid, node_properties_sig: Signal<Properties>) -> Element {
    let node_id_memo = use_memo(use_reactive!(|node_id| node_id));

    let resource_future = use_resource(move || async move {
        match api::get_node_properties(node_id_memo()).await {
            Ok(node_attr) => {
                node_properties_sig.set(node_attr.properties().clone());
                Some(node_attr)
            }
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                None
            }
        }
    });

    match &*resource_future.read_unchecked() {
        Some(Some(node_attr)) => {
            rsx! {
                div {
                    h6 { "Node Configuration" }
                    div {
                        class: "accordion accordion-borderless bg-dark ",
                        id: "accordionNodeConfig",
                        // WICHTIG: Das `key`-Attribut zwingt Dioxus dazu, die Komponente
                        // neu zu mounten, wenn sich die ID ändert. Das verhindert,
                        // dass alte Event-Handler auf den neuen Node angewendet werden.
                        GeneralEditor {
                            key: "{node_id}",
                            node_id,
                            node_type: node_attr.node_type(),
                            name: node_attr.name(),
                            lidt: *node_attr.lidt(),
                            inverted: node_attr.inverted(),
                        }
                        PropertiesEditor {
                            key: "{node_id}",
                            node_id,
                            node_properties_sig,
                        }
                        PositioningEditor {
                            key: "{node_id}",
                            node_id,
                            position_opt: node_attr.isometry(),
                            node_properties_sig,
                            node_type: node_attr.node_type(),
                        }
                        AlignmentEditor {
                            key: "{node_id}",
                            node_id,
                            alignment: node_attr.alignment().unwrap_or(Isometry::identity()),
                            node_properties_sig,
                            node_type: node_attr.node_type(),
                        }
                    }
                }
            }
        }
        _ => {
            rsx! {
                div { "No node selected" }
            }
        }
    }
}