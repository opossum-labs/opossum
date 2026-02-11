#![allow(clippy::derive_partial_eq_without_eq)]
pub mod alignment_editor;
pub mod general_editor;
pub mod properties_editor;

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
use opossum_core::{
    nodes::fluence_detector::Fluence,
    prelude::{Isometry, Properties},
};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct UINodeAttr {
    pub node_id: Uuid,
    pub node_type: String,
    pub name: String,
    pub lidt: Fluence,
    pub inverted: bool,
    pub properties: Properties,
    pub position: Option<Isometry>,
    pub alignment: Option<Isometry>,
}

#[component]
pub fn OpticalNodeEditor(node_id: Memo<Uuid>, on_change: EventHandler<NodeChangeEvent>) -> Element {
    let mut ui_node_attr_sig = use_signal(UINodeAttr::default);
    let resource_future = use_resource(move || async move {
        let node_id = *node_id.read();
        match api::get_node_properties(node_id).await {
            Ok(node_attr) => {
                let ui_node_attr = UINodeAttr {
                    node_id,
                    node_type: node_attr.node_type(),
                    name: node_attr.name(),
                    lidt: *node_attr.lidt(),
                    inverted: node_attr.inverted(),
                    properties: node_attr.properties().clone(),
                    position: node_attr.isometry(),
                    alignment: *node_attr.alignment(),
                };
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
                    }
                    PropertiesEditor {
                        node_id,
                        node_attr: ui_node_attr_sig,
                        on_change_property,
                    }
                    PositioningEditor {
                        node_id,
                        node_attr: ui_node_attr_sig,
                        on_change,
                    }
                    AlignmentEditor {
                        node_id,
                        node_attr: ui_node_attr_sig,
                        on_change,
                    }
                }
            }
        }
    } else {
        rsx! {
            div { "No data" }
        }
    }

    // rsx! {
    //     div { class: "noselect",
    //         h6 { "Node Configuration" }
    //         div {
    //             class: "accordion accordion-borderless bg-dark noselect",
    //             id: "accordionNodeConfig",

    //             GeneralEditor {
    //                 node_id,
    //                 node_type: node_attr.node_type(),
    //                 name: node_attr.name(),
    //                 lidt: *node_attr.lidt(),
    //                 inverted: node_attr.inverted(),
    //                 on_change,
    //             }
    //             PropertiesEditor { node_id, node_properties_sig, on_change }
    //             PositioningEditor {
    //                 node_id,
    //                 position_opt: node_attr.isometry(),
    //                 node_properties_sig,
    //                 node_type: node_attr.node_type(),
    //                 on_change,
    //             }
    //             AlignmentEditor {
    //                 node_id,
    //                 alignment: node_attr.alignment().unwrap_or(Isometry::identity()),
    //                 node_properties_sig,
    //                 node_type: node_attr.node_type(),
    //                 on_change,
    //             }
    //         }
    //     }

    // }
    //     }
    //     else{
    //         rsx!{}
    //     }
    // match &*resource_future.read() {
    //     Some(Some(node_attr)) if node_attr.uuid() == node_id  => {
    //         rsx! {
    //             div { class: "noselect",
    //                 h6 { "Node Configuration" }
    //                 div {
    //                     class: "accordion accordion-borderless bg-dark noselect",
    //                     id: "accordionNodeConfig",

    //                     GeneralEditor {
    //                         node_id,
    //                         node_type: node_attr.node_type(),
    //                         name: node_attr.name(),
    //                         lidt: *node_attr.lidt(),
    //                         inverted: node_attr.inverted(),
    //                         on_change,
    //                     }
    //                     PropertiesEditor {
    //                         node_id,
    //                         node_properties_sig,
    //                         on_change,
    //                     }
    //                     PositioningEditor {
    //                         node_id,
    //                         position_opt: node_attr.isometry(),
    //                         node_properties_sig,
    //                         node_type: node_attr.node_type(),
    //                         on_change,
    //                     }
    //                     AlignmentEditor {
    //                         node_id,
    //                         alignment: node_attr.alignment().unwrap_or(Isometry::identity()),
    //                         node_properties_sig,
    //                         node_type: node_attr.node_type(),
    //                         on_change,
    //                     }
    //                 }
    //             }

    //         }
    //     }
    //     Some(Some(_stale)) => {
    //     // ← DAS ist dein „continue“
    //     rsx! {
    //         div { class: "noselect text-secondary", "Switching node…" }
    //     }
    // }
    //     _ => rsx! {
    //         div { "No data" }
    //     },
    // }
}
