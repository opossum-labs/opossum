#![allow(clippy::derive_partial_eq_without_eq)]
pub mod alignment_editor;
pub mod general_editor;
pub mod properties_editor;

use crate::components::node_editor::optical_node_editor::alignment_editor::AlignmentEditor;
use crate::components::node_editor::optical_node_editor::general_editor::GeneralEditor;
use crate::components::node_editor::optical_node_editor::properties_editor::PropertiesEditor;
use crate::components::scenery_editor::NodeElement;
use crate::{HTTP_API_CLIENT, OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use opossum_backend::Properties;

#[component]
pub fn OpticalNodeEditor(
    node_element_sig: Signal<Option<NodeElement>>,
    node_properties_sig: Signal<Properties>,
) -> Element {
    let resource_future = use_resource(move || async move {
        let node = node_element_sig.read();
        if let Some(node) = &*(node) {
            match api::get_node_properties(&HTTP_API_CLIENT(), node.id()).await {
                Ok(node_attr) => {
                    node_properties_sig.set(node_attr.properties().clone());
                    Some(node_attr)
                }
                Err(err_str) => {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                    None
                }
            }
        } else {
            None
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
                        GeneralEditor {
                            node_id: node_attr.uuid(),
                            node_type: node_attr.node_type(),
                            node_name: node_attr.name(),
                            node_lidt: *node_attr.lidt(),
                        }
                        PropertiesEditor { node_properties_sig }
                        AlignmentEditor {
                            alignment: *node_attr.alignment(),
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
