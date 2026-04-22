#![allow(clippy::derive_partial_eq_without_eq)]
pub mod alignment_editor;
pub mod general_editor;
pub mod properties_editor;

pub(super) use alignment_editor::{
    RotationAlignmentInputs, TranslationAlignmentInputs, on_new_rotation, on_new_translation,
};

use crate::components::{
    node_editor::{
        node_config_editor::NodeChangeEvent,
        optical_node_editor::{
            alignment_editor::{AlignmentEditor, PositioningEditor},
            general_editor::GeneralEditor,
            properties_editor::PropertiesEditor,
        },
    },
    scenery_editor::SelectedNode,
};
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use opossum_core::types::api_types::NodeInfo;

#[component]
pub fn OpticalNodeEditor(
    active_node: Memo<SelectedNode>,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let node_id = use_memo(move || active_node.read().node_id);

    let mut ui_node_info_sig = use_signal(NodeInfo::default);
    let mut readonly = use_signal(|| false);
    let resource_future = use_resource(move || async move {
        let node_id = active_node.read().node_id;
        match api::get_node_properties(node_id).await {
            Ok(node_info) => {
                readonly.set(node_info.node_type == "reference");
                ui_node_info_sig.set(node_info.clone());
                Some(node_info)
            }
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                None
            }
        }
    });

    let on_change_property = EventHandler::new(move |evt: NodeChangeEvent| {
        // if let NodeChangeAction::Property(ref key, ref proptype) = evt.action {
        //     let _ = ui_node_attr_sig
        //         .write()
        //         .properties
        //         .set(key, proptype.clone());
        // }
        // on_change.call(evt);
    });

    if let Some(Some(node_attr)) = &*resource_future.read_unchecked()
        && node_attr.uuid == ui_node_info_sig.read().uuid
    {
        rsx! {
            div { class: "noselect",
                h6 { "Node Configuration" }
                div {
                    class: "accordion accordion-borderless bg-dark noselect",
                    id: "accordionNodeConfig",

                    GeneralEditor {
                        node_info: ui_node_info_sig,
                        active_node,
                        on_change,
                        readonly: readonly(),
                    }
                    PropertiesEditor {
                        node_id,
                        node_attr: ui_node_info_sig,
                        on_change_property,
                        readonly: readonly(),
                    }
                    PositioningEditor {
                        node_id,
                        node_info: ui_node_info_sig,
                        on_change,
                        readonly: readonly(),
                    }
                    AlignmentEditor {
                        node_id,
                        node_info: ui_node_info_sig,
                        on_change,
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
