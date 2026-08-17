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
    info!("🔄 Render: OpticalNodeEditor");
    let node_id = use_memo(move || active_node.read().node_id);
    let mut node_info_sig = use_signal(NodeInfo::default);
    let mut node_properties_sig = use_signal(Properties::default);
    let mut readonly = use_signal(|| false);

    // Fine-grained selector memos to decouple sub-editor renders
    let memo_node_name = use_memo(move || node_info_sig.read().name.clone());
    let memo_node_type = use_memo(move || node_info_sig.read().node_type.clone());
    let memo_node_inverted = use_memo(move || node_info_sig.read().inverted);
    let memo_node_alignment = use_memo(move || node_info_sig.read().alignment.unwrap_or_default());
    let memo_node_isometry = use_memo(move || node_info_sig.read().isometry);

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

    let resource_future: Resource<(Option<NodeInfo>, Option<Properties>)> =
        use_resource(move || async move {
            // Reactively subscribe to external details refresh events
            crate::NODE_DETAILS_REFRESH();

            let current_node_id = active_node.read().node_id;

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
                    if *node_properties_sig.peek() != properties_res.properties {
                        node_properties_sig.set(properties_res.properties.clone());
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

    if let Some((Some(node_info), Some(_))) = &*resource_future.read_unchecked()
        && node_info.uuid == node_info_sig.read().uuid
    {
        rsx! {
            div { class: "noselect",
                h6 { "Node Configuration" }
                div {
                    class: "accordion accordion-borderless bg-dark noselect",
                    id: "accordionNodeConfig",

                    // GeneralEditor {
                    //     node_id,
                    //     name: memo_node_name,
                    //     node_type: memo_node_type,
                    //     inverted: memo_node_inverted,
                    //     active_node,
                    //     on_change,
                    //     readonly: readonly(),
                    // }
                    // PortConfigEditor {
                    //     node_id,
                    //     node_info: node_info_sig,
                    //     on_change,
                    //     readonly: readonly(),
                    // }
                    // PropertiesEditor {
                    //     node_id,
                    //     node_properties_sig,
                    //     node_info_sig,
                    //     on_change: on_property_change,
                    //     readonly: readonly(),
                    // }
                    PositioningEditor {
                        node_id,
                        position_opt: memo_node_isometry,
                        on_change,
                        readonly: readonly(),
                    }
                                // AlignmentEditor {
                //     node_id,
                //     alignment: memo_node_alignment,
                //     node_type: memo_node_type,
                //     node_properties_sig,
                //     on_change,
                //     readonly: readonly(),
                // }
                }
            }
        }
    } else {
        rsx! {
            div { "No data" }
        }
    }
}