use crate::{
    ACTIVE_PUMP_SCENARIO, OPOSSUM_UI_LOGS, PUMP_SCENARIO_LIST_REFRESH, api,
    components::scenery_editor::GraphsWorkspaceAction,
};
use dioxus::prelude::*;
use opossum_core::{gain::PumpScenario, types::api_types::AmplifierDto};
use uuid::Uuid;

/// Document-wide editor for pump scenarios - the operating points a model can be analyzed in.
///
/// Replaces the former property-driven amplifier overview: a node amplifies because a scenario
/// names it, not because of a value sitting on the node itself. Unlike the node-properties view
/// this is not bound to the current selection - it answers "what operating points does this model
/// have, and what does each of them amplify?", including nodes in groups whose tab isn't open.
///
/// Exactly one scenario can be **active** at a time (a GUI-only choice, see [`ACTIVE_PUMP_SCENARIO`]):
/// that is the one the canvas status line and the context menu's amplifier toggle reflect, since a
/// node can only ever show one status on the canvas even though it may belong to several scenarios.
#[component]
pub fn PumpScenarioEditor() -> Element {
    let mut scenarios_resource = use_resource(move || async move {
        // Every change that can alter the list or a scenario's contents bumps this - document
        // structure (delete, paste, undo, load) as well as scenario/gain-model edits made from
        // this very panel. `NODE_DETAILS_REFRESH` is deliberately not read here, for the same
        // reason the old amplifier overview didn't read it: it fires on every property edit of
        // any node, which would refetch this whole list for e.g. an unrelated lens radius.
        PUMP_SCENARIO_LIST_REFRESH();
        match api::get_pump_scenarios().await {
            Ok(scenarios) => scenarios,
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                Vec::new()
            }
        }
    });
    let scenarios = scenarios_resource
        .read_unchecked()
        .clone()
        .unwrap_or_default();

    // The active selection can go stale when the document changes underneath it (the active
    // scenario was deleted from under the user) - fall back to "none" rather than pointing at
    // nothing.
    if let Some(active_id) = ACTIVE_PUMP_SCENARIO()
        && !scenarios.iter().any(|s| s.id == active_id)
    {
        *ACTIVE_PUMP_SCENARIO.write() = None;
    }

    let mut new_scenario_name = use_signal(String::new);
    let mut expanded = use_signal(|| None::<Uuid>);

    let create_scenario = move |()| {
        let name = new_scenario_name.peek().trim().to_string();
        if name.is_empty() {
            return;
        }
        spawn(async move {
            api::eval_action_run(
                api::post_pump_scenario(&name).await,
                Some(move |_id: Uuid| {
                    new_scenario_name.set(String::new());
                    scenarios_resource.restart();
                }),
            );
        });
    };

    rsx! {
        div {
            h6 { "Pump scenarios" }
            div { class: "scenario-new-row",
                input {
                    class: "scenario-new-input",
                    r#type: "text",
                    placeholder: "New scenario name",
                    value: "{new_scenario_name}",
                    oninput: move |e| new_scenario_name.set(e.value()),
                    onkeydown: move |e| {
                        if e.key() == Key::Enter {
                            create_scenario(());
                        }
                    },
                }
                button {
                    r#type: "button",
                    class: "scenario-new-btn",
                    disabled: new_scenario_name().trim().is_empty(),
                    onclick: move |_| create_scenario(()),
                    "Add"
                }
            }
            if scenarios.is_empty() {
                div { class: "amp-empty",
                    "No pump scenarios yet. Add one above, then right-click a lens, wedge or cylindric lens on the canvas and choose \"As amplifier\" to add it."
                }
            } else {
                for scenario in scenarios {
                    ScenarioCard {
                        key: "{scenario.id}",
                        scenario_id: scenario.id,
                        scenario: scenario.scenario,
                        is_expanded: expanded() == Some(scenario.id),
                        on_toggle_expanded: move |id| {
                            expanded.set(if expanded() == Some(id) { None } else { Some(id) });
                        },
                        on_changed: move |()| scenarios_resource.restart(),
                    }
                }
            }
        }
    }
}

/// One scenario: name (renamable), active/delete controls, and - when expanded - the nodes it
/// amplifies.
#[component]
fn ScenarioCard(
    scenario_id: Uuid,
    scenario: PumpScenario,
    is_expanded: bool,
    on_toggle_expanded: EventHandler<Uuid>,
    on_changed: EventHandler<()>,
) -> Element {
    let id = scenario_id;
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let saved_name = scenario.name().to_string();
    let mut name_input = use_signal(|| saved_name.clone());
    // The last name confirmed by the backend - `Signal` is `Copy`, which is what lets `save_name`
    // below be used from both `onblur` and `onkeydown` (a plain closure capturing an owned `String`
    // could only ever be moved into one of them).
    let mut original_name = use_signal(move || saved_name);
    let is_active = ACTIVE_PUMP_SCENARIO() == Some(id);

    let mut save_name = move || {
        let new_name = name_input.peek().trim().to_string();
        let original = original_name.peek().clone();
        if new_name.is_empty() || new_name == original {
            name_input.set(original);
            return;
        }
        spawn(async move {
            api::eval_action_run(
                api::put_pump_scenario_name(id, new_name.clone()).await,
                Some(move |()| {
                    original_name.set(new_name);
                    *PUMP_SCENARIO_LIST_REFRESH.write() += 1;
                    on_changed.call(());
                }),
            );
        });
    };

    rsx! {
        div { class: if is_active { "card bg-dark border-warning mb-2 scenario-card active" } else { "card bg-dark border-secondary mb-2 scenario-card" },
            div { class: "card-body p-2 text-light",
                div { class: "d-flex justify-content-between align-items-center scenario-header",
                    input {
                        class: "scenario-name-input",
                        r#type: "text",
                        value: "{name_input}",
                        oninput: move |e| name_input.set(e.value()),
                        onblur: move |_| save_name(),
                        onkeydown: move |e| {
                            if e.key() == Key::Enter {
                                save_name();
                            }
                        },
                    }
                    div { class: "scenario-actions",
                        button {
                            r#type: "button",
                            title: if is_active { "This is the active scenario - shown on the canvas" } else { "Make this the active scenario" },
                            class: if is_active { "scenario-active-btn active" } else { "scenario-active-btn" },
                            onclick: move |_| {
                                let new_active = if is_active { None } else { Some(id) };
                                workspace_processor
                                    .send(GraphsWorkspaceAction::SetActivePumpScenario(new_active));
                            },
                            if is_active { "\u{2605}" } else { "\u{2606}" }
                        }
                        button {
                            r#type: "button",
                            title: "Delete this scenario",
                            class: "scenario-delete-btn",
                            onclick: move |_| {
                                spawn(async move {
                                    api::eval_action_run(
                                        api::delete_pump_scenario(id).await,
                                        Some(move |()| {
                                            // Routed through the action (rather than clearing
                                            // `ACTIVE_PUMP_SCENARIO` directly) so the canvas markers
                                            // of every open tab get bulk-cleared too, not just the
                                            // global - see `SetActivePumpScenario`'s handling.
                                            if is_active {
                                                workspace_processor
                                                    .send(GraphsWorkspaceAction::SetActivePumpScenario(None));
                                            }
                                            *PUMP_SCENARIO_LIST_REFRESH.write() += 1;
                                            on_changed.call(());
                                        }),
                                    );
                                });
                            },
                            "\u{2715}"
                        }
                    }
                }
                div {
                    class: "amp-card-sub scenario-expand-toggle",
                    onclick: move |_| on_toggle_expanded.call(id),
                    if is_expanded { "\u{25be} amplifiers" } else { "\u{25b8} amplifiers" }
                }
                if is_expanded {
                    ScenarioAmplifiers { scenario_id: id }
                }
            }
        }
    }
}

/// The amplifying nodes of one (expanded) scenario, fetched only while the card is open.
#[component]
fn ScenarioAmplifiers(scenario_id: Uuid) -> Element {
    let amplifiers = use_resource(move || async move {
        PUMP_SCENARIO_LIST_REFRESH();
        match api::get_pump_scenario_amplifiers(scenario_id).await {
            Ok(amplifiers) => amplifiers,
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                Vec::new()
            }
        }
    });
    let amplifiers = amplifiers.read_unchecked().clone().unwrap_or_default();
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();

    rsx! {
        div { class: "scenario-amplifier-list",
            if amplifiers.is_empty() {
                div { class: "amp-empty", "Nothing amplifies in this scenario yet." }
            } else {
                for amplifier in amplifiers {
                    ScenarioAmplifierRow {
                        key: "{amplifier.uuid}",
                        amplifier,
                        on_reveal: move |(node_id, graph_id)| {
                            workspace_processor
                                .send(GraphsWorkspaceAction::RevealNode {
                                    node_id,
                                    graph_id,
                                });
                        },
                    }
                }
            }
        }
    }
}

/// One amplifying node of an expanded scenario. Clicking it reveals the node on the canvas, same as
/// the previous overview's cards - editing the gain model itself still happens in the node's own
/// properties panel (`amp config`), which is where the eventual gain-model editor for this scenario
/// belongs too (see the architecture doc's note on the properties editor's `_ => None` arm).
#[component]
fn ScenarioAmplifierRow(amplifier: AmplifierDto, on_reveal: EventHandler<(Uuid, Uuid)>) -> Element {
    let AmplifierDto {
        uuid,
        name,
        node_type,
        group_id,
        group_name,
        amp_model,
    } = amplifier;

    rsx! {
        div {
            class: "card bg-dark border-secondary mb-1 amp-overview-card",
            onclick: move |_| on_reveal.call((uuid, group_id)),
            div { class: "card-body p-2 text-light",
                div { class: "d-flex justify-content-between align-items-center",
                    span { class: "fw-bold small", "{name}" }
                    span { class: "badge bg-warning text-dark", "{amp_model}" }
                }
                div { class: "amp-card-sub", "{node_type} · {group_name}" }
            }
        }
    }
}
