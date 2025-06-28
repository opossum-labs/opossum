#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::node_editor::property_editor::PropertiesEditor;
use crate::components::node_editor::{
    alignment_editor::AlignmentEditor, general_editor::GeneralEditor,
};
use crate::{api, components::scenery_editor::node::NodeElement, HTTP_API_CLIENT, OPOSSUM_UI_LOGS};
use dioxus::prelude::*;
use opossum_backend::{Fluence, Isometry};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum NodeChange {
    Name(String),
    LIDT(Fluence),
    Alignment(Isometry),
    Inverted(bool),
    Property(String, Value),
    Isometry(Isometry),
}

#[component]
pub fn NodeEditor(mut node: Signal<Option<NodeElement>>) -> Element {
    let node_change = use_context_provider(|| Signal::new(None::<NodeChange>));

    let active_node_opt = node();
    use_effect(move || {
        let node_change_opt = node_change.read().clone();
        if let (Some(node_changed), Some(active_node)) = (node_change_opt, active_node_opt.clone())
        {
            node_change_api_call_selection(node_changed, active_node, node);
        }
    });

    let resource_future = use_resource(move || async move {
        let node = node.read();
        if let Some(node) = &*(node) {
            match api::get_node_properties(&HTTP_API_CLIENT(), node.id()).await {
                Ok(node_attr) => Some(node_attr),
                Err(err_str) => {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                    None
                }
            }
        } else {
            None
        }
    });

    if let Some(Some(node_attr)) = &*resource_future.read_unchecked() {
        rsx! {
            div {
                h6 { "Node Configuration" }
                div {
                    class: "accordion accordion-borderless bg-dark ",
                    id: "accordionNodeConfig",
                    GeneralEditor {
                        node_id: node_attr.uuid(),
                        node_type: node_attr.node_type(),
                        node_name: node_attr.name(),
                        node_lidt: *node_attr.lidt(),
                    }
                    PropertiesEditor {
                        node_properties: node_attr.properties().clone(),
                        node_change,
                    }
                    // LensEditor {
                    //     hidden: node_attr.node_type() != "lens",
                    //     node_change,
                    //     front_curvature_opt: node_attr.get_property("front curvature").cloned().ok(),
                    //     rear_curvature_opt: node_attr.get_property("rear curvature").cloned().ok(),
                    //     center_thickness_opt: node_attr.get_property("center thickness").cloned().ok(),
                    //     refractive_index_opt: node_attr.get_property("refractive index").cloned().ok(),
                    // }
                    AlignmentEditor { alignment: *node_attr.alignment() }
                }
            }
        }
    } else {
        rsx! {
            div { "No node selected" }
        }
    }
}

fn node_change_api_call_selection(
    node_changed: NodeChange,
    mut active_node: NodeElement,
    mut node: Signal<Option<NodeElement>>,
) {
    match node_changed {
        NodeChange::Name(name) => {
            spawn(async move {
                if let Err(err_str) =
                    api::update_node_name(&HTTP_API_CLIENT(), active_node.id(), name.clone()).await
                {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                } else {
                    active_node.set_name(name);
                    node.set(Some(active_node));
                }
            });
        }
        NodeChange::LIDT(lidt) => {
            spawn(async move {
                if let Err(err_str) =
                    api::update_node_lidt(&HTTP_API_CLIENT(), active_node.id(), lidt).await
                {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                };
            });
        }
        NodeChange::Alignment(iso) => {
            spawn(async move {
                if let Err(err_str) =
                    api::update_node_alignment(&HTTP_API_CLIENT(), active_node.id(), iso).await
                {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                }
            });
        }
        NodeChange::Property(key, prop) => {
            spawn(async move {
                if let Err(err_str) =
                    api::update_node_property(&HTTP_API_CLIENT(), active_node.id(), (key, prop))
                        .await
                {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                };
            });
        }
        NodeChange::Isometry(iso) => {
            spawn(async move {
                if let Err(err_str) =
                    api::update_node_isometry(&HTTP_API_CLIENT(), active_node.id(), iso).await
                {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                };
            });
        }
        NodeChange::Inverted(_) => todo!(),
    };
}
