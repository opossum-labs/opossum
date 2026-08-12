use crate::{
    AMP_LIST_REFRESH, OPOSSUM_UI_LOGS, api, components::scenery_editor::GraphsWorkspaceAction,
};
use dioxus::prelude::*;
use opossum_core::types::api_types::AmplifierDto;
use uuid::Uuid;

/// Document-wide list of every amplifying node, as the sidebar's second view.
///
/// Unlike the node-properties view this is not bound to the selection: it answers "where does this
/// setup amplify at all?", including nodes in groups whose tab isn't open. Each entry takes the user
/// to its node, where the parameters are then edited in the properties view.
#[component]
pub fn AmpOverview() -> Element {
    let amplifiers = use_resource(move || async move {
        // Every change that can alter this list bumps `AMP_LIST_REFRESH`: document structure
        // (delete, paste, group, undo, load), setting an amp config, and renaming a node. The
        // catch-all `NODE_DETAILS_REFRESH` is deliberately *not* read - it fires on every property
        // edit of any node, which would refetch the whole list for e.g. a lens radius.
        AMP_LIST_REFRESH();
        match api::get_amplifiers().await {
            Ok(amplifiers) => amplifiers,
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                Vec::new()
            }
        }
    });

    let amplifiers = amplifiers.read_unchecked().clone().unwrap_or_default();
    let mut by_group = use_signal(|| false);
    let mut selected_group = use_signal(|| None::<Uuid>);

    // A large laser system is built from subsystems whose amplifiers repeat, so the interesting
    // question is often "what amplifies inside *this* subsystem". Only groups that actually hold an
    // amplifier are offered - anything else would be a dead entry.
    let groups = groups_of(&amplifiers);
    // The selection can go stale when the document changes underneath it (the group was deleted, or
    // its last amplifier turned passive), so fall back to the first group that is still there.
    let active_group = selected_group()
        .filter(|id| groups.iter().any(|(group_id, _)| group_id == id))
        .or_else(|| groups.first().map(|(group_id, _)| *group_id));

    let shown: Vec<AmplifierDto> = amplifiers
        .iter()
        .filter(|amplifier| !by_group() || Some(amplifier.group_id) == active_group)
        .cloned()
        .collect();

    rsx! {
        div {
            h6 { "Amplifiers" }
            if amplifiers.is_empty() {
                div { class: "amp-empty",
                    "No amplifying components. Right-click a lens, wedge or cylindric lens on the canvas and choose \"As amplifier\"."
                }
            } else {
                // Deliberately not MDB's `.btn`/`.btn-outline-*`: that stylesheet is loaded after
                // this project's own, so its palette would win any specificity tie - and its
                // inactive outline colour is barely legible on this dark panel.
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
                for amplifier in shown {
                    AmplifierCard {
                        key: "{amplifier.uuid}",
                        amplifier,
                        show_group: !by_group(),
                    }
                }
            }
        }
    }
}

/// The distinct groups the given amplifiers live in, as `(uuid, name)` sorted by name.
///
/// Derived from the amplifier list rather than fetched separately, which is what makes "only groups
/// that contain an amplifier" true by construction.
fn groups_of(amplifiers: &[AmplifierDto]) -> Vec<(Uuid, String)> {
    let mut groups: Vec<(Uuid, String)> = Vec::new();
    for amplifier in amplifiers {
        if !groups.iter().any(|(id, _)| *id == amplifier.group_id) {
            groups.push((amplifier.group_id, amplifier.group_name.clone()));
        }
    }
    groups.sort_by(|(_, a), (_, b)| a.cmp(b));
    groups
}

/// One entry of the overview. Clicking it reveals the node on the canvas - the parameters themselves
/// are edited in the properties view, so this card deliberately holds no inputs and therefore needs
/// no dirty tracking of its own.
///
/// `show_group` names the containing group on the card. It is on while the whole document is listed,
/// where "which subsystem is this in?" is the open question, and off once the list is already
/// filtered to a single group, where the answer would be on every card.
#[component]
fn AmplifierCard(amplifier: AmplifierDto, show_group: bool) -> Element {
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
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
            class: "card bg-dark border-secondary mb-2 amp-overview-card",
            onclick: move |_| {
                workspace_processor.send(GraphsWorkspaceAction::RevealNode {
                    node_id: uuid,
                    graph_id: group_id,
                });
            },
            div { class: "card-body p-2 text-light",
                div { class: "d-flex justify-content-between align-items-center",
                    span { class: "fw-bold small", "{name}" }
                    span { class: "badge bg-warning text-dark", "{amp_model}" }
                }
                div { class: "amp-card-sub",
                    "{node_type}"
                    if show_group {
                        span { " · {group_name}" }
                    }
                }
            }
        }
    }
}
