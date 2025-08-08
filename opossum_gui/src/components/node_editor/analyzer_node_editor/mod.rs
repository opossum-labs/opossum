#![allow(clippy::derive_partial_eq_without_eq)]

use crate::components::scenery_editor::NodeElement;
use crate::{HTTP_API_CLIENT, OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;

#[component]
pub fn AnalyzerNodeEditor(
    node_element_sig: Signal<Option<NodeElement>>,
    // node_properties_sig: Signal<Properties>,
) -> Element {
    let resource_future = use_resource(move || async move {
        let node = node_element_sig.read();
        if let Some(node) = &*(node) {
            match api::get_analyzer_info(&HTTP_API_CLIENT(), node.id()).await {
                Ok(analyzer_info) => {
                    // node_properties_sig.set(node_attr.properties().clone());
                    Some(analyzer_info)
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
        Some(Some(_)) => {
            rsx! {
                div {
                    h6 { "Analyzer Configuration" }
                    div {
                        class: "accordion accordion-borderless bg-dark ",
                        id: "accordionNodeConfig",
                                        // GeneralEditor {
                    //     node_id: analyzer_info.id(),
                    //     node_type: format!("{}", analyzer_info.analyzer_type()),
                    //     node_name: analyzer_info.name(),
                    //     node_lidt: *analyzer_info.lidt(),
                    // }
                    // PropertiesEditor { node_properties_sig }
                    // AlignmentEditor {
                    //     alignment: *node_attr.alignment(),
                    //     node_properties_sig,
                    //     node_type: node_attr.node_type(),
                    // }
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
