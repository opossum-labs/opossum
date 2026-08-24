#![allow(clippy::derive_partial_eq_without_eq)]
pub mod alignment_editor;
pub mod general_editor;
pub mod port_config_editor;
pub mod properties_editor;

pub(super) use alignment_editor::{
    RotationAlignmentInputs, TranslationAlignmentInputs, on_new_rotation, on_new_translation,
};
use uuid::Uuid;

use crate::components::node_editor::{
    accordion::open_accordion_section,
    node_config_editor::NodeChangeEvent,
    optical_node_editor::{
        alignment_editor::{AlignmentEditor, PositioningEditor},
        general_editor::GeneralEditor,
        port_config_editor::PortConfigEditor,
        properties_editor::PropertiesEditor,
    },
};
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use opossum_core::{
    prelude::Properties,
    types::api_types::{NodeEditorPanel, NodeInfo},
};

#[component]
pub fn OpticalNodeEditor(
    // Replaced the complex SelectedNode struct with specific granular primitive props
    node_id: ReadSignal<Uuid>,
    graph_id: ReadSignal<Uuid>,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    info!("🔄 Render: OpticalNodeEditor");

    // We no longer need a memo to extract the node_id, as it's passed directly.
    let mut node_info_sig = use_signal(NodeInfo::default);
    let mut node_properties = use_signal(Properties::default);
    let mut readonly = use_signal(|| false);

    // Fine-grained selector memos to decouple sub-editor renders
    let memo_node_name = use_memo(move || node_info_sig.read().name.clone());
    let memo_node_type = use_memo(move || node_info_sig.read().node_type.clone());
    let memo_node_inverted = use_memo(move || node_info_sig.read().inverted);
    let memo_node_isometry = use_memo(move || node_info_sig.read().isometry);
    let memo_node_alignment = use_memo(move || node_info_sig.read().alignment.unwrap_or_default());

    let resource_future: Resource<(Option<NodeInfo>, Option<Properties>)> =
        use_resource(move || async move {
            // Reactively subscribe to external details refresh events
            crate::NODE_DETAILS_REFRESH();

            // Use the node_id directly from the prop
            let current_node_id = *node_id.read();

            // Avoid fetching if there is no valid ID selected
            if current_node_id == Uuid::nil() {
                return (None, None);
            }

            let node_info = match api::get_node_info(current_node_id).await {
                Ok(new_info) => {
                    let is_ref = new_info.node_type == "reference";
                    if *readonly.peek() != is_ref {
                        readonly.set(is_ref);
                    }
                    if *node_info_sig.peek() != new_info {
                        node_info_sig.set(new_info.clone());
                    }
                    Some(new_info)
                }
                Err(err_str) => {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                    None
                }
            };

            let properties = match api::get_node_properties(current_node_id).await {
                Ok(properties_res) => {
                    if *node_properties.peek() != properties_res.properties {
                        node_properties.set(properties_res.properties.clone());
                    }
                    Some(properties_res.properties)
                }
                Err(err_str) => {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                    None
                }
            };

            (node_info, properties)
        });

    // Handle accordion opening on undo/redo
    use_effect(move || {
        let Some((uuid, panel)) = *crate::PENDING_PANEL_OPEN.read() else {
            return;
        };
        // Check against the prop directly
        if uuid != *node_id.read() {
            return;
        }
        let Some((Some(node_info), Some(_))) = &*resource_future.read_unchecked() else {
            return;
        };
        if node_info.uuid != uuid {
            return; // Stale in-flight fetch for a previous node
        }
        if panel != NodeEditorPanel::PortConfig {
            open_accordion_section(panel);
            *crate::PENDING_PANEL_OPEN.write() = None;
        }
    });

    // Since node_id is directly passed as a signal down to us, it is always the "active" one.
    // We only need to check if it's a valid, non-nil UUID.
    let is_active = *node_id.read() != Uuid::nil();

    if let Some((Some(node_info), Some(_))) = &*resource_future.read_unchecked()
        && node_info.uuid == node_info_sig.read().uuid
    {
        rsx! {
            div { class: "noselect",
                h6 { "Node Configuration" }
                div {
                    class: "accordion accordion-borderless bg-dark noselect",
                    id: "accordionNodeConfig",

                    GeneralEditor {
                        node_id: *node_id.read(),
                        name: memo_node_name.read().clone(),
                        node_type: memo_node_type.read().clone(),
                        inverted: *memo_node_inverted.read(),
                        is_active,
                        graph_id: *graph_id.read(),
                        on_change,
                        readonly: readonly(),
                    }
                    PortConfigEditor {
                        node_id: *node_id.read(),
                        node_info: node_info_sig,
                        on_change,
                        readonly: readonly(),
                    }
                    PropertiesEditor {
                        node_id: *node_id.read(),
                        node_properties,
                        node_info_sig,
                        on_change,
                        readonly: readonly(),
                    }
                    PositioningEditor {
                        node_id,
                        position_opt: memo_node_isometry,
                        on_change,
                        readonly: readonly(),
                    }
                    AlignmentEditor {
                        node_id,
                        alignment: memo_node_alignment,
                        node_type: memo_node_type,
                        node_properties,
                        on_change,
                        readonly: readonly(),
                    }
                }
            }
        }
    } else {
        rsx! {
            div { class: "noselect", "No data" }
        }
    }
}
