use dioxus::prelude::*;
use opossum_backend::Fluence;
use uuid::Uuid;

use crate::components::node_editor::{accordion::AccordionItem, node_editor_component::{NodeChange, NodePropInput}};

#[component]
pub fn GeneralEditor(uuid: Uuid, node_type: String, node_name: String, node_lidt: Fluence ) -> Element{
let accordion_content = vec![
            rsx!{
                NodePropInput {
                    name: "NodeId".to_string(),
                    placeholder: "Node ID".to_string(),
                    node_change: NodeChange::NodeConst(format!("{}", uuid)),
                }
                NodePropInput {
                    name: "NodeType".to_string(),
                    placeholder: "Node Type".to_string(),
                    node_change: NodeChange::NodeConst(format!("{node_type}")),
                }
                NodePropInput {
                    name: "NodeName".to_string(),
                    placeholder: "Node Name".to_string(),
                    node_change: NodeChange::Name(node_name),
                }
                NodePropInput {
                    name: "LIDT".to_string(),
                    placeholder: "LIDT in J/cm²".to_string(),
                    node_change: NodeChange::LIDT(node_lidt),
                }
            }
        ];
    rsx!{
        AccordionItem{elements: accordion_content, header: "General", header_id: "generalHeading", parent_id: "accordionNodeConfig", content_id: "generalCollapse"}
    }

}