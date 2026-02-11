#![allow(clippy::derive_partial_eq_without_eq)]
pub mod ghost_focus_editor;
pub mod ray_trace_editor;

use crate::components::node_editor::{
    analyzer_node_editor::{
        ghost_focus_editor::GhostFocusEditor, ray_trace_editor::RayTraceEditor,
    },
    node_config_editor::NodeChangeEvent,
    optical_node_editor::general_editor::NodeTypeInput,
};
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use opossum_core::prelude::*;
use uuid::Uuid;

#[component]
pub fn AnalyzerNodeEditor(
    node_id: Memo<Uuid>,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let resource_future = use_resource(move || async move {
        match api::get_analyzer_info(*node_id.read()).await {
            Ok(analyzer_info) => Some(analyzer_info),
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                None
            }
        }
    });
    match &*resource_future.read_unchecked() {
        Some(Some(analyzer_info)) => {
            if analyzer_info.id() == *node_id.read() {
                rsx! {
                    div {
                        h6 { "Analyzer Configuration" }
                        div {
                            class: "accordion accordion-borderless bg-dark ",
                            id: "accordionAnalyzerConfig",
                            NodeTypeInput {
                                node_type: format!("{}", analyzer_info.analyzer_type()),
                                label: "Analyzer Type",
                            }
                            {
                                match analyzer_info.analyzer_type().clone() {
                                    AnalyzerType::Energy => rsx! {},
                                    AnalyzerType::RayTrace(ray_trace_config) => {
                                        rsx! {
                                            RayTraceEditor { node_id, ray_trace_config, on_change }
                                        }
                                    }
                                    AnalyzerType::GhostFocus(ghost_focus_config) => {
                                        rsx! {
                                            GhostFocusEditor { node_id, ghost_focus_config, on_change }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                rsx! {}
            }
        }
        _ => {
            rsx! {
                div { "No node selected" }
            }
        }
    }
}
