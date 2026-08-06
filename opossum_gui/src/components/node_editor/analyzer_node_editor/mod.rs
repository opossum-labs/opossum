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

/// Wires a source-port card to `PENDING_SOURCE_CARD_OPEN`: when an undo/redo (via the backend's
/// `JumpTarget::source_port`) focuses this `(analyzer_id, port_uuid)`, expand the card (`is_collapsed`),
/// scroll it into view, open every collapsible *inside* the card (the ray source's Position / Energy /
/// Spectral Distribution accordions - otherwise the reverted value stays hidden), then clear the request so
/// it fires once. Call once per card - it's a custom hook - and give the card's outer element the id
/// `sourceCard{port_uuid}`. Shared by the energy, ray-trace and ghost-focus source editors so the
/// open/scroll behaviour isn't triplicated.
pub fn use_source_card_focus(
    analyzer_id: uuid::Uuid,
    port_uuid: uuid::Uuid,
    mut is_collapsed: Signal<bool>,
) {
    use_effect(move || {
        if *crate::PENDING_SOURCE_CARD_OPEN.read() == Some((analyzer_id, port_uuid)) {
            is_collapsed.set(false);
            // Scoping the inner-accordion open to this card's own subtree (not the shared static content
            // ids) targets exactly the focused card and avoids duplicate-id collisions across cards.
            // Deferred one animation frame so the card body (which only mounts once `is_collapsed` is
            // false) has rendered before we open its accordions. Energy sources have no inner accordions,
            // so the `forEach` is simply empty for them.
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
    let node_id = use_memo(move || active_node.read().node_id);

    // CRITICAL FIX: Pair the node_id directly inside the async future with the fetched info.
    // This ensures that the ID and the Config data update at the EXACT same millisecond.
    let mut resource_future = use_resource(move || async move {
        let current_id = *node_id.read();
        match api::get_analyzer(current_id).await {
            Ok(analyzer_info) => Some((current_id, analyzer_info)), // Paired Tuple
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                None
            }
        }
    });

    // Scenery-wide list of "source port" nodes, shared by the Energy/RayTrace/GhostFocus editors to
    // decide which source cards to render. Fetched here (not per-editor) so it refreshes on the same
    // `NODE_DETAILS_REFRESH` trigger as the analyzer config below - a source port's creation/deletion
    // (including via undo/redo) bumps that signal, which previously only refreshed card *values*, not
    // *membership*, leaving a deleted source's card stuck on screen.
    let mut available_sources_future = use_resource(move || async move {
        match api::get_available_sources().await {
            Ok(sources) => sources,
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                Vec::new()
            }
        }
    });

    use_effect(move || {
        crate::NODE_DETAILS_REFRESH();
        resource_future.restart();
        available_sources_future.restart();
    });

    match &*resource_future.read_unchecked() {
        Some(Some((loaded_id, analyzer_info))) => {
            let loaded_id_val = *loaded_id;
            let available_sources = available_sources_future.read().clone().unwrap_or_default();
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
                                        EnergyEditor {
                                            node_id: loaded_id_val,
                                            energy_config,
                                            on_change,
                                            available_sources,
                                        }
                                    }
                                }
                                AnalyzerType::RayTrace(ray_trace_config) => {
                                    rsx! {
                                        RayTraceEditor {
                                            node_id: loaded_id_val,
                                            ray_trace_config,
                                            on_change,
                                            available_sources,
                                        }
                                    }
                                }
                                AnalyzerType::GhostFocus(ghost_focus_config) => {
                                    rsx! {
                                        GhostFocusEditor {
                                            node_id: loaded_id_val,
                                            ghost_focus_config,
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
