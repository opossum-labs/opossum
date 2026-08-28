#![allow(clippy::derive_partial_eq_without_eq)]
pub mod energy_editor;
pub mod ghost_focus_editor;
mod light_data_editor;
pub mod ray_trace_editor;
mod source_port_card;

use crate::components::{
    node_editor::{
        analyzer_node_editor::{
            energy_editor::EnergyEditor, ghost_focus_editor::GhostFocusEditor,
            ray_trace_editor::RayTraceEditor,
        },
        node_config_editor::{NodeChangeAction, NodeChangeEvent},
        optical_node_editor::general_editor::NodeTypeInput,
    },
    scenery_editor::SelectedNode,
};
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use opossum_core::{
    analyzers::energy::EnergyConfig,
    prelude::{AnalyzerType, GhostFocusConfig, RayTraceConfig},
    types::api_types::PumpScenarioItemDto,
};
use uuid::Uuid;

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
        // The scenario list is not part of the analyzer, so its own refresh signal has to be read
        // as well - a scenario created or renamed elsewhere has to appear in the selection below
        // without reselecting the analyzer.
        crate::PUMP_SCENARIO_LIST_REFRESH();

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

        let pump_scenarios = match api::get_pump_scenarios().await {
            Ok(scenarios) => scenarios,
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                Vec::new()
            }
        };

        (current_id, analyzer_info, available_sources, pump_scenarios)
    });

    match &*resource_future.read_unchecked() {
        Some((loaded_id, Some(analyzer_info), available_sources, pump_scenarios))
            if *loaded_id == *node_id.read() =>
        {
            let loaded_id_val = *loaded_id;
            let available_sources = available_sources.clone();
            let pump_scenarios = pump_scenarios.clone();
            let selected_scenarios = analyzer_info.pump_scenarios().to_vec();

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
                        // Outside the per-type match on purpose: which operating points a run uses
                        // is stated next to the analyzer's config, not inside it, so it applies to
                        // every kind of analysis alike.
                        PumpScenarioSelection {
                            analyzer_id: loaded_id_val,
                            selected: selected_scenarios,
                            scenarios: pump_scenarios,
                            on_change,
                        }
                    }
                }
            }
        }
        _ => {
            rsx! {
                div { class: "noselect", "No node selected" }
            }
        }
    }
}

/// The operating points this analyzer is run in: one report per selected pump scenario, none
/// selected meaning a single passive run.
///
/// This is what turns a configured scenario into a result. Without it an amplifier can be set up
/// completely - candidate, scenario, gain model - and still never amplify anything, because nothing
/// ever tells an analysis to use that operating point.
///
/// Selecting a scenario appends it to the end of the list rather than reordering the selection: the
/// order is the order the reports come out in, so a scenario that was already selected keeps its
/// place when another one is added or removed.
///
/// # Props
///
/// * `analyzer_id` - the analyzer whose selection is edited.
/// * `selected` - the scenarios currently selected, in report order.
/// * `scenarios` - every scenario the document has, in document order.
/// * `on_change` - the analyzer editor's usual change channel, which saves and marks the document
///   modified.
#[component]
fn PumpScenarioSelection(
    analyzer_id: Uuid,
    selected: Vec<Uuid>,
    scenarios: Vec<PumpScenarioItemDto>,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    // Each row carries the selection its own checkbox would produce, worked out here rather than in
    // the event handler: the handler then has nothing to decide, and every row states the whole list
    // it sends - which is what the endpoint takes.
    let rows: Vec<(Uuid, String, bool, Vec<Uuid>)> = scenarios
        .iter()
        .map(|item| {
            let is_selected = selected.contains(&item.id);
            let mut toggled = selected.clone();
            if is_selected {
                toggled.retain(|id| *id != item.id);
            } else {
                toggled.push(item.id);
            }
            (
                item.id,
                item.scenario.name().to_string(),
                is_selected,
                toggled,
            )
        })
        .collect();
    let nothing_selected = selected.is_empty();

    rsx! {
        div { class: "pump-scenario-selection mt-2",
            label { class: "text-secondary small", "Pump scenarios" }
            if rows.is_empty() {
                div { class: "text-secondary small fst-italic",
                    "No pump scenario defined - this analysis runs on the passive model."
                }
            } else {
                for (scenario_id, name, is_selected, toggled) in rows {
                    div { class: "form-check", key: "{scenario_id}",
                        input {
                            class: "form-check-input",
                            r#type: "checkbox",
                            id: "analyzer-{analyzer_id}-scenario-{scenario_id}",
                            checked: is_selected,
                            onchange: move |_| {
                                on_change
                                    .call(NodeChangeEvent {
                                        node_id: analyzer_id,
                                        action: NodeChangeAction::AnalyzerPumpScenarios(toggled.clone()),
                                    });
                            },
                        }
                        label {
                            class: "form-check-label text-light small",
                            r#for: "analyzer-{analyzer_id}-scenario-{scenario_id}",
                            "{name}"
                        }
                    }
                }
                // Stated rather than left to be inferred from an empty list of ticks: running
                // passively is a legitimate setting, not a missing one.
                if nothing_selected {
                    div { class: "text-secondary small fst-italic",
                        "None selected - this analysis runs once, on the passive model."
                    }
                }
            }
        }
    }
}
