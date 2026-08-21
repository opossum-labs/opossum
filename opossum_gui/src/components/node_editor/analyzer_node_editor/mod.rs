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
use opossum_core::{
    analyzers::energy::EnergyConfig,
    prelude::{AnalyzerType, GhostFocusConfig, RayTraceConfig},
};

/// Wires a source-port card to `PENDING_SOURCE_CARD_OPEN` for auto-expansion on undo/redo actions.
pub fn use_source_card_focus(
    analyzer_id: uuid::Uuid,
    port_uuid: uuid::Uuid,
    mut is_collapsed: Signal<bool>,
) {
    use_effect(move || {
        if *crate::PENDING_SOURCE_CARD_OPEN.read() == Some((analyzer_id, port_uuid)) {
            is_collapsed.set(false);
            let script = format!(
                "const card = document.getElementById('sourceCard{port_uuid}'); \
                 if (card) {{ \
                   card.scrollIntoView({{ behavior: 'smooth', block: 'nearest' }}); \
                   requestAnimationFrame(() => {{ \
                     card.querySelectorAll('.accordion-collapse').forEach(el => {{ \
                       if (window.mdb && mdb.Collapse) {{ mdb.Collapse.getOrCreateInstance(el, {{ toggle: false }}).show(); }} \
                     }}); \
                   }}); \
                 }}"
            );
            spawn(async move {
                let _ = dioxus::document::eval(&script).await;
            });
            *crate::PENDING_SOURCE_CARD_OPEN.write() = None;
        }
    });
}

#[component]
pub fn AnalyzerNodeEditor(
    active_node: Memo<SelectedNode>,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    info!("🔄 Render: AnalyzerNodeEditor");
    let node_id = use_memo(move || active_node.read().node_id);

    // Dedicated signals to supply reactive ReadSignal props to specific sub-editors
    let mut energy_config_sig = use_signal(EnergyConfig::default);
    let mut ray_trace_config_sig = use_signal(RayTraceConfig::default);
    let mut ghost_focus_config_sig = use_signal(GhostFocusConfig::default);

    // Single unified resource to load analyzer details and source ports together
    let resource_future = use_resource(move || async move {
        crate::NODE_DETAILS_REFRESH();

        let current_id = *node_id.read();

        let analyzer_info = match api::get_analyzer(current_id).await {
            Ok(info) => {
                match info.analyzer_type() {
                    AnalyzerType::Energy(cfg) => energy_config_sig.set(cfg.clone()),
                    AnalyzerType::RayTrace(cfg) => ray_trace_config_sig.set(cfg.clone()),
                    AnalyzerType::GhostFocus(cfg) => ghost_focus_config_sig.set(cfg.clone()),
                }
                Some(info)
            }
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                None
            }
        };

        let available_sources = match api::get_available_sources().await {
            Ok(sources) => sources,
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                Vec::new()
            }
        };

        (current_id, analyzer_info, available_sources)
    });

    match &*resource_future.read_unchecked() {
        Some((loaded_id, Some(analyzer_info), available_sources))
            if *loaded_id == *node_id.read() =>
        {
            let loaded_id_val = *loaded_id;
            let available_sources = available_sources.clone();

            rsx! {
                div {
                    class: "analyzer-node-editor-container p-1",
                    style: "max-height: 75vh; overflow-y: auto; overflow-x: hidden; padding-right: 4px;",

                    h6 { "Analyzer Configuration" }
                    div {
                        class: "accordion accordion-borderless bg-dark",
                        id: "accordionAnalyzerConfig",
                        NodeTypeInput {
                            node_type: format!("{}", analyzer_info.analyzer_type()),
                            label: "Analyzer Type",
                        }
                        {
                            match analyzer_info.analyzer_type() {
                                AnalyzerType::Energy(_) => {
                                    rsx! {
                                        EnergyEditor {
                                            node_id: loaded_id_val,
                                            energy_config: energy_config_sig,
                                            on_change,
                                            available_sources,
                                        }
                                    }
                                }
                                AnalyzerType::RayTrace(_) => {
                                    rsx! {
                                        RayTraceEditor {
                                            node_id: loaded_id_val,
                                            ray_trace_config: ray_trace_config_sig,
                                            on_change,
                                            available_sources,
                                        }
                                    }
                                }
                                AnalyzerType::GhostFocus(_) => {
                                    rsx! {
                                        GhostFocusEditor {
                                            node_id: loaded_id_val,
                                            ghost_focus_config: ghost_focus_config_sig,
                                            on_change,
                                            available_sources,
                                        }
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
