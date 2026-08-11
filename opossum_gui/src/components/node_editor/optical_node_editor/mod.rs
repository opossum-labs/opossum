#![allow(clippy::derive_partial_eq_without_eq)]
pub mod alignment_editor;
pub mod general_editor;
pub mod port_config_editor;
pub mod properties_editor;

pub(super) use alignment_editor::{
    RotationAlignmentInputs, TranslationAlignmentInputs, on_new_rotation, on_new_translation,
};

use crate::components::{
    node_editor::{
        accordion::open_accordion_section,
        node_config_editor::{NodeChangeAction, NodeChangeEvent},
        optical_node_editor::{
            alignment_editor::{AlignmentEditor, PositioningEditor},
            general_editor::GeneralEditor,
            port_config_editor::PortConfigEditor,
            properties_editor::PropertiesEditor,
        },
    },
    scenery_editor::SelectedNode,
};
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use opossum_core::{
    prelude::Properties,
    types::api_types::{NodeEditorPanel, NodeInfo},
};

#[component]
pub fn OpticalNodeEditor(
    active_node: Memo<SelectedNode>,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let node_id = use_memo(move || active_node.read().node_id);
    let graph_id = use_memo(move || active_node.read().graph_id);
    let mut node_info_sig = use_signal(NodeInfo::default);
    let mut node_properties_sig = use_signal(Properties::default);
    let mut readonly = use_signal(|| false);

    let on_property_change = EventHandler::new(move |node_change: NodeChangeEvent| {
        if let NodeChangeAction::Property(name, proptype) = &node_change.action {
            if let Err(e) = node_properties_sig.write().set(name, proptype.clone()) {
                OPOSSUM_UI_LOGS.write().add_log(&format!(
                    "Error setting new property value of proptype '{name}': {e}"
                ));
            } else {
                on_change.call(node_change);
            }
        }
    });

    let mut resource_future: Resource<(Option<NodeInfo>, Option<Properties>)> =
        use_resource(move || async move {
            let node_id = active_node.read().node_id;
            let node_info = match api::get_node_info(node_id).await {
                Ok(node_info) => {
                    readonly.set(node_info.node_type == "reference");
                    node_info_sig.set(node_info.clone());
                    Some(node_info)
                }
                Err(err_str) => {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                    None
                }
            };
            let properties = match api::get_node_properties(node_id).await {
                Ok(properties_res) => {
                    node_properties_sig.set(properties_res.properties.clone());
                    Some(properties_res.properties)
                }
                Err(err_str) => {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                    None
                }
            };

            (node_info, properties)
        });

    use_effect(move || {
        crate::NODE_DETAILS_REFRESH();
        resource_future.restart();
    });

    // Undo/redo just auto-selected this node because it (or its tab) wasn't already displayed - open
    // the panel it changed once this node's data has actually loaded (the accordion DOM for a freshly
    // selected node doesn't exist until then). Reading `resource_future` here (not just the pending
    // signal) means this effect naturally re-runs once the fetch resolves, instead of needing to poll.
    use_effect(move || {
        let Some((uuid, panel)) = *crate::PENDING_PANEL_OPEN.read() else {
            return;
        };
        if uuid != *node_id.read() {
            return;
        }
        let Some((Some(node_info), Some(_))) = &*resource_future.read_unchecked() else {
            return;
        };
        if node_info.uuid != uuid {
            return; // stale in-flight fetch for a previous node
        }
        // PortConfig has its own, separately-loaded resource in `PortConfigEditor` - it clears the
        // pending signal itself once *its* data has loaded, to avoid the same missing-DOM race here.
        if panel != NodeEditorPanel::PortConfig {
            open_accordion_section(panel);
            *crate::PENDING_PANEL_OPEN.write() = None;
        }
    });

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
                        node_info: node_info_sig,
                        active_node,
                        on_change,
                        readonly: readonly(),
                    }
                    PortConfigEditor {
                        node_id,
                        node_info: node_info_sig,
                        on_change,
                        readonly: readonly(),
                    }
                    PropertiesEditor {
                        node_id,
                        graph_id,
                        node_properties_sig,
                        node_info_sig,
                        on_change: on_property_change,
                        readonly: readonly(),
                    }
                    PositioningEditor {
                        node_id,
                        node_info: node_info_sig,
                        on_change,
                        readonly: readonly(),
                    }
                    AlignmentEditor {
                        node_id,
                        node_info: node_info_sig,
                        on_change,
                        node_properties_sig,
                        readonly: readonly(),
                    }
                }
            }
        }
    } else {
        rsx! {
            div { "No data" }
        }
    }
}
