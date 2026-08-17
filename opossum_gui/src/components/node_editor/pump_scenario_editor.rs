use crate::{
    ACTIVE_PUMP_SCENARIO, OPOSSUM_UI_LOGS, PUMP_SCENARIO_LIST_REFRESH, api,
    components::scenery_editor::GraphsWorkspaceAction,
};
use dioxus::prelude::*;
use opossum_core::{
    gain::{ConstGain, GainModel, PumpScenario},
    types::api_types::ScenarioAmplifierDto,
};
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

/// Every amplifier candidate of the document, each showing (and letting the user edit) its gain
/// model within this one (expanded) scenario - fetched only while the card is open.
///
/// The list is never empty because "nothing amplifies here": every candidate appears regardless of
/// whether it is turned on in this particular scenario (`GainModel::None` if not) - it is empty only
/// when the document has no candidates at all, in any scenario.
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

    // A large laser system is built from subsystems whose candidates repeat, so the interesting
    // question is often "what's in *this* subsystem". Only groups that actually hold a candidate are
    // offered - anything else would be a dead entry. Own filter state per expanded scenario, same as
    // the document-wide amplifier overview this restores the behavior of.
    let mut by_group = use_signal(|| false);
    let mut selected_group = use_signal(|| None::<Uuid>);

    let groups = groups_of(&amplifiers);
    // The selection can go stale when the document changes underneath it (the group was deleted, or
    // its last candidate was unmarked), so fall back to the first group that is still there.
    let active_group = selected_group()
        .filter(|id| groups.iter().any(|(group_id, _)| group_id == id))
        .or_else(|| groups.first().map(|(group_id, _)| *group_id));

    rsx! {
        div { class: "scenario-amplifier-list",
            if amplifiers.is_empty() {
                div { class: "amp-empty",
                    "No candidates in the whole document yet. Right-click a lens, wedge or cylindric lens on the canvas and choose \"As amplifier\"."
                }
            } else {
                div { class: "amp-filter",
                    for (label , shows_one_group) in [("All", false), ("By group", true)] {
                        button {
                            key: "{label}",
                            r#type: "button",
                            class: if by_group() == shows_one_group { "amp-filter-btn active" } else { "amp-filter-btn" },
                            onclick: move |_| by_group.set(shows_one_group),
                            "{label}"
                        }
                    }
                }
                if by_group() {
                    select {
                        class: "amp-group-select",
                        value: active_group.map(|id| id.to_string()).unwrap_or_default(),
                        onchange: move |e| {
                            selected_group.set(Uuid::parse_str(&e.value()).ok());
                        },
                        for (group_id , group_name) in groups {
                            option { key: "{group_id}", value: "{group_id}", "{group_name}" }
                        }
                    }
                }
                for amplifier in amplifiers
                    .iter()
                    .filter(|amplifier| !by_group() || Some(amplifier.group_id) == active_group)
                    .cloned()
                {
                    ScenarioAmplifierRow {
                        key: "{amplifier.uuid}",
                        scenario_id,
                        amplifier,
                        show_group: !by_group(),
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

/// The distinct groups the given candidates live in, as `(uuid, name)` sorted by name.
///
/// Derived from the candidate list rather than fetched separately, which is what makes "only groups
/// that contain a candidate" true by construction. Restores `amp_overview.rs`'s own `groups_of`
/// (recovered from git history at its last revision before that file was replaced) unchanged - every
/// candidate still carries `group_id`/`group_name` the same way.
fn groups_of(amplifiers: &[ScenarioAmplifierDto]) -> Vec<(Uuid, String)> {
    let mut groups: Vec<(Uuid, String)> = Vec::new();
    for amplifier in amplifiers {
        if !groups.iter().any(|(id, _)| *id == amplifier.group_id) {
            groups.push((amplifier.group_id, amplifier.group_name.clone()));
        }
    }
    groups.sort_by(|(_, a), (_, b)| a.cmp(b));
    groups
}

/// One amplifier candidate of an expanded scenario: a None/Const switch plus, when Const, the gain
/// factor - both editing this node's [`GainModel`] within `scenario_id` specifically, unaffected by
/// (and not affecting) any other scenario. Clicking the name or the subtext reveals the node on the
/// canvas, same as the previous overview's cards; the switch and the factor input have their own
/// normal cursor, so they don't fight that click.
///
/// `show_group` names the containing group on the card - see `AmplifierCard`'s identical reasoning
/// in the amp overview this restores the behavior of (git history).
#[component]
fn ScenarioAmplifierRow(
    scenario_id: Uuid,
    amplifier: ScenarioAmplifierDto,
    show_group: bool,
    on_reveal: EventHandler<(Uuid, Uuid)>,
) -> Element {
    let ScenarioAmplifierDto {
        uuid,
        name,
        node_type,
        group_id,
        group_name,
        gain_model,
    } = amplifier;
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let is_active = gain_model.is_active();

    // Local text form of the gain factor, re-synced whenever the fetched `gain_model` actually
    // changes (a switch flip made elsewhere, an undo/redo, this very row's own save completing) -
    // same "compare and pull in" shape `FlushableTextInput` uses, without needing that component's
    // dirty-tracking: nothing here re-renders mid-keystroke, only after a completed save.
    let mut gain_str = use_signal(|| format_gain(gain_model));
    let mut last_gain_model = use_signal(|| gain_model);
    if *last_gain_model.peek() != gain_model {
        last_gain_model.set(gain_model);
        gain_str.set(format_gain(gain_model));
    }

    let set_model = move |model: GainModel| {
        workspace_processor.send(GraphsWorkspaceAction::SetScenarioGainModel {
            scenario_id,
            node_id: uuid,
            graph_id: group_id,
            model,
        });
    };

    // `ConstGain::new` validates finite/non-negative; a rejected value is surfaced the same way
    // `eval_action_run` already logs errors elsewhere, and the field reverts to the last known-good
    // value rather than keeping the rejected text.
    let mut save_gain = move || {
        let raw = gain_str.peek().trim().to_string();
        match raw
            .parse::<f64>()
            .map_err(|e| e.to_string())
            .and_then(|v| ConstGain::new(v).map_err(|e| e.to_string()))
        {
            Ok(gain) => set_model(GainModel::Const(gain)),
            Err(err_str) => {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log(&format!("'{raw}' is not a valid gain factor: {err_str}"));
                gain_str.set(format_gain(gain_model));
            }
        }
    };

    rsx! {
        div { class: "card bg-dark border-secondary mb-1 amp-overview-card",
            div { class: "card-body p-2 text-light",
                div { class: "d-flex justify-content-between align-items-center",
                    span {
                        class: "fw-bold small amp-row-reveal",
                        onclick: move |_| on_reveal.call((uuid, group_id)),
                        "{name}"
                    }
                    button {
                        r#type: "button",
                        title: if is_active { "Turn off amplification in this scenario" } else { "Turn on amplification in this scenario" },
                        class: if is_active { "amp-gain-switch active" } else { "amp-gain-switch" },
                        onclick: move |_| {
                            set_model(if is_active { GainModel::None } else { GainModel::Const(ConstGain::default()) });
                        },
                        if is_active { "Const" } else { "None" }
                    }
                }
                if is_active {
                    input {
                        class: "amp-gain-input",
                        r#type: "number",
                        step: "0.1",
                        min: "0",
                        value: "{gain_str}",
                        oninput: move |e| gain_str.set(e.value()),
                        onblur: move |_| save_gain(),
                        onkeydown: move |e| {
                            if e.key() == Key::Enter {
                                save_gain();
                            }
                        },
                    }
                }
                div {
                    class: "amp-card-sub amp-row-reveal",
                    onclick: move |_| on_reveal.call((uuid, group_id)),
                    "{node_type}"
                    if show_group {
                        span { " · {group_name}" }
                    }
                }
            }
        }
    }
}

/// Text form of a [`GainModel`]'s gain factor - `Const`'s own value, or the default `Const` would
/// start at (1.0) for `None` (and, defensively, for any future variant this editor doesn't know how
/// to show a factor for yet - `GainModel` is `#[non_exhaustive]`), so the factor input is pre-filled
/// sensibly the moment the switch turns a candidate on.
fn format_gain(model: GainModel) -> String {
    match model {
        GainModel::Const(gain) => format!("{}", gain.gain()),
        _ => format!("{}", ConstGain::default().gain()),
    }
}
