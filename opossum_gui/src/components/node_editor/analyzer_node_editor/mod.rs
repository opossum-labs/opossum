#![allow(clippy::derive_partial_eq_without_eq)]
pub mod energy_editor;
pub mod ghost_focus_editor;
mod light_data_editor;
pub mod ray_trace_editor;

use crate::components::{
    node_editor::{
        analyzer_node_editor::{
            energy_editor::EnergyEditor, ghost_focus_editor::GhostFocusEditor,
            ray_trace_editor::RayTraceEditor,
        },
        node_config_editor::NodeChangeEvent,
        optical_node_editor::general_editor::NodeTypeInput,
    },
    scenery_editor::SelectedNode,
};
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use opossum_core::prelude::*;

#[component]
pub fn AnalyzerNodeEditor(
    active_node: Memo<SelectedNode>,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let node_id = use_memo(move || active_node.read().node_id);

    // CRITICAL FIX: Pair the node_id directly inside the async future with the fetched info.
    // This ensures that the ID and the Config data update at the EXACT same millisecond.
    let resource_future = use_resource(move || async move {
        let current_id = *node_id.read();
        match api::get_analyzer(current_id).await {
            Ok(analyzer_info) => Some((current_id, analyzer_info)), // Paired Tuple
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                None
            }
        }
    });

    match &*resource_future.read_unchecked() {
        Some(Some((loaded_id, analyzer_info))) => {
            let loaded_id_val = *loaded_id;
            rsx! {
                div {
                    class: "analyzer-node-editor-container p-1",
                    style: "max-height: 75vh; overflow-y: auto; overflow-x: hidden; padding-right: 4px;",

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
                                AnalyzerType::Energy(energy_config) => {
                                    rsx! {
                                        EnergyEditor { node_id: loaded_id_val, energy_config, on_change }
                                    }
                                }
                                AnalyzerType::RayTrace(ray_trace_config) => {
                                    rsx! {
                                        RayTraceEditor { node_id: loaded_id_val, ray_trace_config, on_change }
                                    }
                                }
                                AnalyzerType::GhostFocus(ghost_focus_config) => {
                                    rsx! {
                                        GhostFocusEditor { node_id: loaded_id_val, ghost_focus_config, on_change }
                                    }
                                }
                            }
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
