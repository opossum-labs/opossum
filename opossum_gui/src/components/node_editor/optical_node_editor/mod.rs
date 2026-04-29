#![allow(clippy::derive_partial_eq_without_eq)]
pub mod alignment_editor;
pub mod aperture_editor;
pub mod general_editor;
pub mod port_config_editor;
pub mod properties_editor;

pub(super) use alignment_editor::{
    RotationAlignmentInputs, TranslationAlignmentInputs, on_new_rotation, on_new_translation,
};

use crate::components::{
    node_editor::{
        node_config_editor::NodeChangeEvent,
        optical_node_editor::{
            alignment_editor::{AlignmentEditor, PositioningEditor},
            aperture_editor::ApertureEditor,
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
    prelude::{Aperture, Properties},
    types::api_types::NodeInfo,
};

#[component]
pub fn OpticalNodeEditor(
    active_node: Memo<SelectedNode>,
    on_change: EventHandler<NodeChangeEvent>,
) -> Element {
    let node_id = use_memo(move || active_node.read().node_id);

    let mut node_info_sig = use_signal(NodeInfo::default);
    let mut node_properties_sig = use_signal(Properties::default);
    let mut readonly = use_signal(|| false);
    let resource_future: Resource<(Option<NodeInfo>, Option<Properties>)> =
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
                        node_properties_sig,
                        node_info_sig,
                        on_change,
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
                        readonly: readonly(),
                    }
                    ApertureEditor {
                        node_id,
                        aperture: Aperture::default(),
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
