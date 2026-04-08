#![allow(clippy::derive_partial_eq_without_eq)]
pub mod alignment_editor;
pub mod general_editor;
pub mod properties_editor;

pub(super) use alignment_editor::{
    RotationAlignmentInputs, TranslationAlignmentInputs, on_new_rotation, on_new_translation,
};

use crate::components::node_editor::{
    node_config_editor::{NodeChangeAction, NodeChangeEvent},
    optical_node_editor::{
        alignment_editor::{AlignmentEditor, PositioningEditor},
        general_editor::GeneralEditor,
        properties_editor::PropertiesEditor,
    },
};
use crate::{OPOSSUM_UI_LOGS, api};
use dioxus::prelude::*;
use opossum_core::prelude::{Isometry, Properties};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct UINodeAttr {
    pub node_id: Uuid,
    pub node_type: String,
    pub name: String,
    // pub lidt: Fluence,
    pub inverted: bool,
    pub properties: Properties,
    pub position: Option<Isometry>,
    pub alignment: Option<Isometry>,
}

#[component]
pub fn OpticalNodeEditor(node_id: Memo<Uuid>, on_change: EventHandler<NodeChangeEvent>) -> Element {
    let mut ui_node_attr_sig = use_signal(UINodeAttr::default);
    let mut readonly = use_signal(|| false);
    let resource_future = use_resource(move || async move {
        let node_id = *node_id.read();
        match api::get_node_properties(node_id).await {
            Ok((node_attr, is_reference)) => {
                let ui_node_attr = UINodeAttr {
                    node_id,
                    node_type: node_attr.node_type(),
                    name: node_attr.name(),
                    // lidt: *node_attr.lidt(),
                    inverted: node_attr.inverted(),
                    properties: node_attr.properties().clone(),
                    position: node_attr.isometry(),
                    alignment: *node_attr.alignment(),
                };
                readonly.set(is_reference);
                ui_node_attr_sig.set(ui_node_attr.clone());
                Some(ui_node_attr)
            }
            Err(err_str) => {
                OPOSSUM_UI_LOGS.write().add_log(&err_str);
                None
            }
        }
    });

    let on_change_property = EventHandler::new(move |evt: NodeChangeEvent| {
        if let NodeChangeAction::Property(ref key, ref proptype) = evt.action {
            let _ = ui_node_attr_sig
                .write()
                .properties
                .set(key, proptype.clone());
        }
        on_change.call(evt);
    });

    if let Some(Some(node_attr)) = &*resource_future.read_unchecked()
        && node_attr.node_id == ui_node_attr_sig.read().node_id
    {
        rsx! {
            div { class: "noselect",
                h6 { "Node Configuration" }
                div {
                    class: "accordion accordion-borderless bg-dark noselect",
                    id: "accordionNodeConfig",

                    GeneralEditor {
                        node_attr: ui_node_attr_sig,
                        node_id,
                        on_change,
                        readonly: readonly(),
                    }
                    PropertiesEditor {
                        node_id,
                        node_attr: ui_node_attr_sig,
                        on_change_property,
                        readonly: readonly(),
                    }
                    PositioningEditor {
                        node_id,
                        node_attr: ui_node_attr_sig,
                        on_change,
                        readonly: readonly(),
                    }
                    AlignmentEditor {
                        node_id,
                        node_attr: ui_node_attr_sig,
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
