#![allow(clippy::derive_partial_eq_without_eq)]
use super::NodeElement;
use super::NodeType;
use crate::components::scenery_editor::{
    graph_editor::graph_editor_component::{DragStatus, EditorState},
    graph_store::GraphStore,
    node::graph_node_components::GraphNodeContent,
    ports::ports_component::NodePorts,
};
use dioxus::prelude::*;
const NODE_BEAMSPLITTER: Asset = asset!("./assets/icons/node_beamsplitter.svg");
const NODE_CYLINDRIC_LENS: Asset = asset!("./assets/icons/node_cylindric_lens.svg");
const NODE_ENERGY_METER: Asset = asset!("./assets/icons/node_energymeter.svg");
const NODE_FILTER: Asset = asset!("./assets/icons/node_filter.svg");
const NODE_FLUENCE: Asset = asset!("./assets/icons/node_fluence.svg");
const NODE_GRATING: Asset = asset!("./assets/icons/node_grating.svg");
const NODE_GROUP: Asset = asset!("./assets/icons/node_group.svg");
const NODE_LENS: Asset = asset!("./assets/icons/node_lens.svg");
const NODE_MIRROR: Asset = asset!("./assets/icons/node_mirror.svg");
const NODE_PARABOLA: Asset = asset!("./assets/icons/node_parabola.svg");
const NODE_PARAXIAL: Asset = asset!("./assets/icons/node_paraxial.svg");
const NODE_PROPAGATION: Asset = asset!("./assets/icons/node_propagation.svg");
const NODE_SOURCE: Asset = asset!("./assets/icons/node_source.svg");
const NODE_SPECTROMETER: Asset = asset!("./assets/icons/node_spectrometer.svg");
const NODE_SPOTDIAGRAM: Asset = asset!("./assets/icons/node_spotdiagram.svg");
const NODE_UNKNOWN: Asset = asset!("./assets/icons/node_unknown.svg");
const NODE_WEDGE: Asset = asset!("./assets/icons/node_wedge.svg");

#[component]
pub fn Node(node: NodeElement, node_activated: Signal<Option<NodeElement>>) -> Element {
    let mut editor_status = use_context::<EditorState>();
    let mut graph_store = use_context::<GraphStore>();
    let position = node.pos();
    let active_node_id = graph_store.active_node();
    let is_active = active_node_id.map_or("", |active_node_id| {
        if active_node_id == node.id() {
            "active-node"
        } else {
            ""
        }
    });
    let id = node.id();
    let z_index = node.z_index();
    let node_icon = match node.node_type.clone() {
        NodeType::Optical(node_type) => match node_type.as_str() {
            // "dummy" => Some(NODE_UNKNOWN),
            "beam splitter" => Some(NODE_BEAMSPLITTER),
            "energy meter" => Some(NODE_ENERGY_METER),
            "group" => Some(NODE_GROUP),
            "ideal filter" => Some(NODE_FILTER),
            "reflective grating" => Some(NODE_GRATING),
            // "reference" => Some(NODE_UNKNOWN),
            "lens" => Some(NODE_LENS),
            "cylindric lens" => Some(NODE_CYLINDRIC_LENS),
            "source" => Some(NODE_SOURCE),
            "spectrometer" => Some(NODE_SPECTROMETER),
            "spot diagram" => Some(NODE_SPOTDIAGRAM),
            // "wavefront monitor" => Some(NODE_UNKNOWN),
            "paraxial surface" => Some(NODE_PARAXIAL),
            "ray propagation" => Some(NODE_PROPAGATION),
            "fluence detector" => Some(NODE_FLUENCE),
            "wedge" => Some(NODE_WEDGE),
            "mirror" => Some(NODE_MIRROR),
            "parabolic mirror" => Some(NODE_PARABOLA),
            _ => Some(NODE_UNKNOWN),
        },
        NodeType::Analyzer(_) => None,
    };
    rsx! {
        div {
            tabindex: 0, // necessary to allow to receive keyboard focus
            class: "node {is_active}",
            draggable: false,
            style: format!("left: {}px; top: {}px; z-index: {z_index};", position.x, position.y),
            onmousedown: move |event: MouseEvent| {
                editor_status.drag_status.set(DragStatus::Node(id));
                let previously_selected = graph_store.active_node();
                if previously_selected != Some(id) {
                    graph_store.set_node_active(id);
                    node_activated.set(Some(node.clone()));
                }
                event.stop_propagation();
            },
            onkeydown: move |event| {
                if event.data().key() == Key::Delete {
                    spawn(async move { graph_store.delete_node(id).await });
                }
                event.stop_propagation();
            },
            GraphNodeContent {
                node_name: node.name(),
                node_type: node.node_type().clone(),
                node_body: rsx! {
                    div {
                        class: "node-body",
                        draggable: false,
                        style: format!("height: {}px;", node.node_body_height()),
                        if node_icon.is_some() {
                            img {
                                src: node_icon.unwrap(),
                                width: "50px",
                                style: "display: block; margin: auto;",
                            }
                        }
                        NodePorts { node: node.clone() }
                    }
                },
            }
        }
    }
}
// #[must_use]
// fn use_node_context_menu(node_id: Uuid) -> Callback<Event<MouseData>> {
//     use_callback(move |evt: Event<MouseData>| {
//         println!("Node context menu clicked");
//         evt.prevent_default();
//         let mut cx_menu = CONTEXT_MENU.write();
//         *cx_menu = CxMenu::new(
//             evt.page_coordinates().x,
//             evt.page_coordinates().y,
//             vec![("Delete node".to_owned(), use_delete_node(node_id))],
//         );
//     })
// }
