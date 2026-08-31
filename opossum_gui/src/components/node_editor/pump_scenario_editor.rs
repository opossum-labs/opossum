use crate::{
    ACTIVE_PUMP_SCENARIO, OPOSSUM_UI_LOGS, PUMP_SCENARIO_LIST_REFRESH, api,
    components::{
        node_editor::{
            inputs::{input_components::LabeledSelect, select_options_from_enum_iterator},
            pump_source_editor::PumpSourceEditor,
        },
        scenery_editor::GraphsWorkspaceAction,
    },
};
use dioxus::prelude::*;
use opossum_core::{
    gain::{ConstGain, GainModel, MonochromaticSmallSignalGain, PumpScenario},
    reciprocal_meter,
    types::api_types::ScenarioAmplifierDto,
    utils::default_from_name::DefaultFromName,
};
use uuid::Uuid;

/// Document-wide editor for pump scenarios - the operating points a model can be analyzed in.
///
/// Replaces the former property-driven amplifier overview: a node amplifies because a scenario
/// names it, not because of a value sitting on the node itself. Unlike the node-properties view
/// this is not bound to the current selection - it answers "what operating points does this model
/// have, and what does each of them amplify?", including nodes in groups whose tab isn't open.
///
/// Exactly one scenario is **active** whenever the document has at least one (a GUI-only choice, see
/// [`ACTIVE_PUMP_SCENARIO`]): that is the one the canvas status line and the context menu's
/// amplifier toggle reflect, since a node can only ever show one status on the canvas even though it
/// may belong to several scenarios. "No scenario active" only ever happens with zero scenarios in
/// the document - [`GraphsWorkspaceAction::EnsureActivePumpScenario`] is what keeps that invariant,
/// sent here after every create/delete this panel performs.
#[component]
pub fn PumpScenarioEditor() -> Element {
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
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
                    // Activates it right away if it's the document's first scenario - otherwise a
                    // no-op, since a previously active scenario is still perfectly valid.
                    workspace_processor.send(GraphsWorkspaceAction::EnsureActivePumpScenario);
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
                {
                    // The last scenario is not deletable while any node is still marked as an
                    // amplifier candidate - see `ScenarioCard`'s doc comment.
                    let is_only_scenario = scenarios.len() == 1;
                    rsx! {
                        for scenario in scenarios {
                            ScenarioCard {
                                key: "{scenario.id}",
                                scenario_id: scenario.id,
                                scenario: scenario.scenario,
                                is_expanded: expanded() == Some(scenario.id),
                                is_only_scenario,
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
    }
}

/// One scenario: name (renamable), a delete control, and - when expanded - the nodes it amplifies.
/// Clicking the card at all (not a dedicated button) makes it the active scenario, shown via a
/// lightened background rather than an icon - see `activate` below.
#[component]
fn ScenarioCard(
    scenario_id: Uuid,
    scenario: PumpScenario,
    is_expanded: bool,
    /// Whether this is the only scenario the document has - passed down rather than re-derived here
    /// so every card agrees on the same scenario list `PumpScenarioEditor` already fetched.
    is_only_scenario: bool,
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
    let mut original_name = use_signal(|| saved_name.clone());
    // Resync when the backend's name changes from outside this card's own save - an undo/redo, or a
    // rename made elsewhere. `PUMP_SCENARIO_LIST_REFRESH` is what re-fetches `scenario` and re-runs
    // this component with the new prop; without this comparison, `use_signal`'s init closures only
    // ever run once (this component instance persists across that refetch, keyed by `scenario_id`),
    // so the input would keep showing whatever text was here before the change took effect even
    // though the document already reverted - which is exactly what made an undo/redo of a rename
    // look like it did nothing.
    if *original_name.peek() != saved_name {
        original_name.set(saved_name.clone());
        name_input.set(saved_name);
    }
    let is_active = ACTIVE_PUMP_SCENARIO() == Some(id);
    // Deleting the document's only scenario while a node is still marked as an amplifier candidate
    // would recreate exactly the dead end `ensure_a_pump_scenario_exists` exists to avoid: a
    // candidate with nowhere to configure its gain model. Blocked here rather than left to the
    // backend, which has nothing that would reject it - the candidate set and the scenario list are
    // independent on that side.
    let delete_blocked = is_only_scenario && !crate::AMPLIFIER_CANDIDATES.read().is_empty();

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

    // Clicking anywhere on the card (the name, the amplifiers toggle, the body) makes it the active
    // scenario - there's no separate "activate" control to reach for. A no-op once it already is.
    // The delete button opts out via `stop_propagation`, so removing a scenario never activates it
    // first just because the click passed through the card on its way there.
    let activate = move || {
        if !is_active {
            workspace_processor.send(GraphsWorkspaceAction::SetActivePumpScenario(Some(id)));
        }
    };

    rsx! {
        div {
            class: if is_active { "card bg-dark border-secondary mb-2 scenario-card active" } else { "card bg-dark border-secondary mb-2 scenario-card" },
            onclick: move |_| activate(),
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
                            title: if delete_blocked { "Can't delete the last scenario while a node is still marked as an amplifier - unmark it first" } else { "Delete this scenario" },
                            class: "scenario-delete-btn",
                            disabled: delete_blocked,
                            onclick: move |event: Event<MouseData>| {
                                event.stop_propagation();
                                if delete_blocked {
                                    return;
                                }
                                spawn(async move {
                                    api::eval_action_run(
                                        api::delete_pump_scenario(id).await,
                                        Some(move |()| {
                                            // If this was the active scenario, activates another
                                            // remaining one instead of leaving the selection empty -
                                            // see `EnsureActivePumpScenario`'s doc comment. A no-op if
                                            // some other scenario was already active.
                                            workspace_processor
                                                .send(GraphsWorkspaceAction::EnsureActivePumpScenario);
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
                    if is_expanded {
                        "\u{25be} amplifiers"
                    } else {
                        "\u{25b8} amplifiers"
                    }
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
                    for (label, shows_one_group) in [("All", false), ("By group", true)] {
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
                        for (group_id, group_name) in groups {
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
        config,
    } = amplifier;
    let gain_model = config.gain_model();
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();

    // Local text form of the gain factor, re-synced whenever the fetched `gain_model` actually
    // changes (a model picked elsewhere, an undo/redo, this very row's own save completing) -
    // same "compare and pull in" shape `FlushableTextInput` uses, without needing that component's
    // dirty-tracking: nothing here re-renders mid-keystroke, only after a completed save.
    let mut is_collapsed = use_signal(|| true);
    let mut gain_str = use_signal(|| format_gain(gain_model));
    let mut ssg_g0_str = use_signal(|| format_ssg_g0(gain_model));
    let mut last_gain_model = use_signal(|| gain_model);
    if *last_gain_model.peek() != gain_model {
        last_gain_model.set(gain_model);
        gain_str.set(format_gain(gain_model));
        ssg_g0_str.set(format_ssg_g0(gain_model));
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

    // The one parameter of the monochromatic model: its peak gain coefficient g₀ (in m⁻¹, and free
    // to be negative for an absorbing medium). The grid it is resolved on lives on the analytic
    // pump, not here.
    let mut save_ssg = move || {
        let raw = ssg_g0_str.peek().trim().to_string();
        match raw.parse::<f64>().map_err(|e| e.to_string()).and_then(|v| {
            MonochromaticSmallSignalGain::new(reciprocal_meter!(v)).map_err(|e| e.to_string())
        }) {
            Ok(ssg) => set_model(GainModel::MonochromaticSmallSignalGain(ssg)),
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&format!(
                    "'{raw}' is not a valid gain coefficient: {err_str}"
                ));
                ssg_g0_str.set(format_ssg_g0(gain_model));
            }
        }
    };

    rsx! {
        div { class: "amp-overview-card",
            div {
                class: "amp-row-header",
                onclick: move |_| is_collapsed.toggle(),
                span { class: "amp-row-arrow",
                    if is_collapsed() {
                        "\u{25b8}"
                    } else {
                        "\u{25be}"
                    }
                }
                span { class: "fw-bold small", "{name}" }
            }
            if !is_collapsed() {
                div { class: "amp-config-block",
                    LabeledSelect {
                        id: format!("amp-{uuid}-gain"),
                        label: "Gain model".to_string(),
                        options: select_options_from_enum_iterator(&gain_model, None),
                        onchange: move |e: Event<FormData>| {
                            // A freshly picked model starts at its own default, which for `Const` is
                            // a factor of 1.0 - selecting it must not change a result on its own.
                            if let Some(picked) = GainModel::default_from_name(&e.value()) {
                                set_model(picked);
                            }
                        },
                    }
                    // The factor belongs to `Const` specifically, not to "amplifies at all" - a
                    // later model has its own parameters and would be mis-edited by this field.
                    if matches!(gain_model, GainModel::Const(_)) {
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
                    if let GainModel::MonochromaticSmallSignalGain(_) = gain_model {
                        div { class: "form-floating border-start",
                            input {
                                class: "form-control bg-dark text-light form-control-sm noselect",
                                r#type: "number",
                                id: format!("ssg-g0-{uuid}"),
                                step: "0.1",
                                value: "{ssg_g0_str}",
                                oninput: move |e| ssg_g0_str.set(e.value()),
                                onblur: move |_| save_ssg(),
                                onkeydown: move |e| {
                                    if e.key() == Key::Enter {
                                        save_ssg();
                                    }
                                },
                            }
                            label {
                                class: "form-label text-secondary",
                                r#for: format!("ssg-g0-{uuid}"),
                                "Peak gain g₀ (m⁻¹)"
                            }
                        }
                    }
                }
                // Pumping only means anything to a model that reads the medium's inversion. For one
                // that works from its own parameters - a fixed factor, or none at all - these
                // settings would have no effect, so they are not offered. Which models those are is
                // the model's own answer, not a list kept here.
                if gain_model.needs_inversion() {
                    PumpSourceEditor {
                        id_prefix: format!("amp-{uuid}"),
                        source: config.pump(),
                        on_change: move |pump| {
                            workspace_processor
                                .send(GraphsWorkspaceAction::SetScenarioPumpSource {
                                    scenario_id,
                                    node_id: uuid,
                                    pump,
                                });
                        },
                    }
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

/// Text form of the monochromatic model's peak gain coefficient in m⁻¹ - the model's own value, or
/// the default (zero) it starts at for any other variant, so the field is pre-filled sensibly the
/// moment the switch turns the model on.
fn format_ssg_g0(model: GainModel) -> String {
    let coefficient = if let GainModel::MonochromaticSmallSignalGain(ssg) = model {
        ssg.peak_gain_coefficient()
    } else {
        MonochromaticSmallSignalGain::default().peak_gain_coefficient()
    };
    format!("{}", coefficient.value)
}
