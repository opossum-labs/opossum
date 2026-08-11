use crate::{
    AMP_LIST_REFRESH, NODE_DETAILS_REFRESH, OPOSSUM_UI_LOGS, api,
    components::scenery_editor::GraphsWorkspaceAction,
};
use dioxus::prelude::*;
use opossum_core::types::api_types::AmplifierDto;

/// Document-wide list of every amplifying node, as the sidebar's second view.
///
/// Unlike the node-properties view this is not bound to the selection: it answers "where does this
/// setup amplify at all?", including nodes in groups whose tab isn't open. Each entry takes the user
/// to its node, where the parameters are then edited in the properties view.
#[component]
pub fn AmpOverview() -> Element {
    let amplifiers = use_resource(move || async move {
        // Both counters are read unconditionally so this refetches on any relevant change:
        // `AMP_LIST_REFRESH` for document structure (delete, paste, group, undo, load),
        // `NODE_DETAILS_REFRESH` for the per-node edits (amp config, rename).
        AMP_LIST_REFRESH();
        NODE_DETAILS_REFRESH();
        match api::get_amplifiers().await {
            Ok(amplifiers) => amplifiers,
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                Vec::new()
            }
        }
    });

    let amplifiers = amplifiers.read_unchecked().clone().unwrap_or_default();

    rsx! {
        div { class: "noselect",
            h6 { "Amplifiers" }
            if amplifiers.is_empty() {
                div { class: "text-muted small fst-italic",
                    "No amplifying components. Right-click a lens, wedge or cylindric lens on the canvas and choose \"As amplifier\"."
                }
            }
            for amplifier in amplifiers {
                AmplifierCard { key: "{amplifier.uuid}", amplifier }
            }
        }
    }
}

/// One entry of the overview. Clicking it reveals the node on the canvas - the parameters themselves
/// are edited in the properties view, so this card deliberately holds no inputs and therefore needs
/// no dirty tracking of its own.
#[component]
fn AmplifierCard(amplifier: AmplifierDto) -> Element {
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let AmplifierDto {
        uuid,
        name,
        node_type,
        group_id,
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
                div { class: "text-muted small", "{node_type}" }
            }
        }
    }
}
