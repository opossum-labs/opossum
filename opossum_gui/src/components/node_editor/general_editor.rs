use dioxus::prelude::*;
use opossum_backend::Fluence;
use uuid::Uuid;

use crate::components::node_editor::{accordion::{AccordionItem, LabeledInput}, node_editor_component::{NodeChange, NodePropInput}};

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
                NameInput{node_name}
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

#[component]
pub fn NameInput(node_name: String) -> Element {
    let node_change_signal = use_context::<Signal<Option<NodeChange>>>();

    rsx! {
        LabeledInput {
            id: "inputNodeName",
            label: "Node Name",
            value: node_name,
            onchange: Some(name_onchange(node_change_signal)),
        },
    }    
}

pub fn name_onchange(
    mut signal: Signal<Option<NodeChange>>,
) -> Callback<Event<FormData>> {
    use_callback(move |e: Event<FormData>| {
        let Ok(name) = e.data.value().parse::<String>();
        signal.set(Some(NodeChange::Name(name)));
    })
}

// #[component]
// pub fn IDInput(node_ID: Uuid) -> Element {
//     let mut node_change_signal = use_context::<Signal<Option<NodeChange>>>();

//     rsx! {
//         LabeledInput {
//             id: "inputNodeID",
//             label: "Node ID",
//             value: node_name,
//             onchange: move |e:Event<FormData>|{                            
//                     let Ok(name) = e.data.parsed::<String>();
//                     node_change_signal.set(Some(NodeChange::(name)));
//             }
//         },
//     }    
// }