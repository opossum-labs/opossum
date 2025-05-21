use crate::{api, components::scenery_editor::node::NodeElement, HTTP_API_CLIENT, OPOSSUM_UI_LOGS};
use dioxus::{html::geometry::euclid::num::Zero, prelude::*};
use nalgebra::Point3;
use opossum_backend::{Isometry, NodeAttr};
use uom::si::f64::Angle;
use uom::si::{angle::degree, f64::Length, length::meter};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum NodeChange {
    Name(String),
    LIDT(f64),
    TranslationX(Length),
    TranslationY(Length),
    TranslationZ(Length),
    RotationRoll(Angle),
    RotationPitch(Angle),
    RotationYaw(Angle),
    Inverted(bool),
    NodeConst(String), // AlignLikeNodeAtDistance(Uuid, Length),
}

#[component]
pub fn NodeEditor(mut node: Signal<Option<NodeElement>>) -> Element {
    let mut node_change = use_context_provider(|| Signal::new(None::<NodeChange>));
    let active_node_opt = node();
    use_effect(move || {
        let node_change_opt = node_change.read().clone();
        let mut node = node.clone();
        if let (Some(node_change), Some(mut active_node)) =
            (node_change_opt, active_node_opt.clone())
        {
            match node_change {
                NodeChange::Name(name) => {
                    spawn(async move {
                        if let Err(err_str) = api::update_node_name(
                            &HTTP_API_CLIENT(),
                            active_node.id(),
                            name.clone(),
                        )
                        .await
                        {
                            OPOSSUM_UI_LOGS.write().add_log(&err_str);
                        } else {
                            println!("setting name");
                            active_node.set_name(name);
                            node.set(Some(active_node));
                        }
                    });
                }
                // NodeChange::LIDT(lidt) => {
                //         if let Some(node) = *node.read() {
                //             spawn(async move {
                //                 api::set_node_lidt(&HTTP_API_CLIENT(), node.id(), lidt.clone())
                //                 .await
                //                 .unwrap()});
                //         }
                // }
                _ => {}
            }
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
                    id: "accordionGeneral",
                    div { class: "accordion-item bg-dark text-light",
                        h2 { class: "accordion-header", id: "generalHeading",
                            button {
                                class: "accordion-button collapsed bg-dark text-light",
                                r#type: "button",
                                "data-mdb-collapse-init": "",
                                "data-mdb-target": "#generalCollapse",
                                "aria-expanded": "false",
                                "aria-controls": "generalCollapse",
                                "General"
                            }
                        }
                        div {
                            id: "generalCollapse",
                            class: "accordion-collapse collapse  bg-dark",
                            "aria-labelledby": "generalHeading",
                            "data-mdb-parent": "#accordionGeneral",
                            div { class: "accordion-body  bg-dark",
                                NodePropInput {
                                    name: "NodeId".to_string(),
                                    placeholder: "Node ID".to_string(),
                                    node_change: NodeChange::NodeConst(format!("{}", node_attr.uuid())),
                                }
                                NodePropInput {
                                    name: "NodeType".to_string(),
                                    placeholder: "Node Type".to_string(),
                                    node_change: NodeChange::NodeConst(format!("{}", node_attr.node_type().to_string())),
                                
                                }
                                NodePropInput {
                                    name: "NodeName".to_string(),
                                    placeholder: "Node Name".to_string(),
                                    node_change: NodeChange::Name(node_attr.name().to_string()),
                                }
                                NodePropInput {
                                    name: "LIDT".to_string(),
                                    placeholder: "LIDT in J/cm²".to_string(),
                                    node_change: NodeChange::LIDT(node_attr.lidt().value / 10000.),
                                }
                            }
                        }
                    }
                
                }
                div {
                    class: "accordion accordion-borderless bg-dark ",
                    id: "accordionAlignment",
                    div { class: "accordion-item bg-dark text-light",
                        h2 { class: "accordion-header", id: "alignmentHeading",
                            button {
                                class: "accordion-button collapsed bg-dark text-light",
                                r#type: "button",
                                "data-mdb-collapse-init": "",
                                "data-mdb-target": "#alignmentCollapse",
                                "aria-expanded": "false",
                                "aria-controls": "alignmentCollapse",
                                "Alignment"
                            }
                        }
                        div {
                            id: "alignmentCollapse",
                            class: "accordion-collapse collapse  bg-dark",
                            "aria-labelledby": "alignmentHeading",
                            "data-mdb-parent": "#accordionAlignment",
                            div { class: "accordion-body  bg-dark",
                                NodePropInput {
                                    name: "XTranslation".to_string(),
                                    placeholder: "X Translation in m".to_string(),
                                    node_change: NodeChange::TranslationX(
                                        node_attr.alignment().as_ref().map_or(Length::zero(), |a| a.translation().x),
                                    ),
                                }
                                NodePropInput {
                                    name: "YTranslation".to_string(),
                                    placeholder: "Y Translation in m".to_string(),
                                    node_change: NodeChange::TranslationY(
                                        node_attr.alignment().as_ref().map_or(Length::zero(), |a| a.translation().y),
                                    ),
                                
                                }
                                NodePropInput {
                                    name: "ZTranslation".to_string(),
                                    placeholder: "Z Translation in m".to_string(),
                                    node_change: NodeChange::TranslationZ(
                                        node_attr.alignment().as_ref().map_or(Length::zero(), |a| a.translation().z),
                                    ),
                                
                                }
                                NodePropInput {
                                    name: "Roll".to_string(),
                                    placeholder: "Roll angle in degree".to_string(),
                                    node_change: NodeChange::RotationRoll(
                                        node_attr.alignment().as_ref().map_or(Angle::zero(), |a| a.rotation().x),
                                    ),
                                }
                                NodePropInput {
                                    name: "Pitch".to_string(),
                                    placeholder: "Pitch angle in degree".to_string(),
                                    node_change: NodeChange::RotationRoll(
                                        node_attr.alignment().as_ref().map_or(Angle::zero(), |a| a.rotation().y),
                                    ),
                                
                                }
                                NodePropInput {
                                    name: "Yaw".to_string(),
                                    placeholder: "Yaw angle in degree".to_string(),
                                    node_change: NodeChange::RotationRoll(
                                        node_attr.alignment().as_ref().map_or(Angle::zero(), |a| a.rotation().z),
                                    ),
                                }
                            }
                        }
                    }
                }
            

            // node_type: String,
            // name: String,
            // ports: OpticPorts,
            // uuid: Uuid,
            // lidt: Fluence,
            // #[serde(default)]
            // props: Properties,
            // #[serde(skip_serializing_if = "Option::is_none")]
            // isometry: Option<Isometry>,
            // #[serde(default)]
            // inverted: bool,
            // #[serde(skip_serializing_if = "Option::is_none")]
            // alignment: Option<Isometry>,
            // #[serde(skip)]
            // global_conf: Option<Arc<Mutex<SceneryResources>>>,
            // #[serde(skip_serializing_if = "Option::is_none")]
            // align_like_node_at_distance: Option<(Uuid, Length)>,
            // #[serde(skip_serializing_if = "Option::is_none")]
            // gui_position: Option<Point3<i32>>,
            }
        }
    } else {
        rsx! {
            div { "No node selected" }
        }
    }
}

#[component]
pub fn NodePropInput(name: String, placeholder: String, node_change: NodeChange) -> Element {
    let mut node_change_signal = use_context::<Signal<Option<NodeChange>>>();
    let input_name = format!("prop{name}");

    let (init_value, input_type, readonly) = match node_change {
        NodeChange::Name(ref node_name) => (node_name.clone(), "text", false),
        NodeChange::LIDT(lidt) => (format!("{:.2}", lidt / 10000.), "number", false),
        NodeChange::TranslationX(x_trans) => {
            (format!("{:.6}", x_trans.get::<meter>()), "number", false)
        }
        NodeChange::TranslationY(y_trans) => {
            (format!("{:.6}", y_trans.get::<meter>()), "number", false)
        }
        NodeChange::TranslationZ(z_trans) => {
            (format!("{:.6}", z_trans.get::<meter>()), "number", false)
        }
        NodeChange::RotationRoll(roll) => (format!("{:.6}", roll.get::<degree>()), "number", false),
        NodeChange::RotationPitch(pitch) => {
            (format!("{:.6}", pitch.get::<degree>()), "number", false)
        }
        NodeChange::RotationYaw(yaw) => (format!("{:.6}", yaw.get::<degree>()), "number", false),
        NodeChange::Inverted(inverted) => (format!("{inverted}"), "checkbox", false),
        NodeChange::NodeConst(ref val) => (val.clone(), "text", true),
    };

    if input_type == "checkbox" {
        rsx! {
            div { class: "form-check",
                input {
                    class: "form-check-input",
                    r#type: input_type,
                    id: input_name.clone(),
                    name: input_name.clone(),
                    value: init_value,
                    checked: true,
                                // onchange: {
                //     move |event: Event<FormData>| {
                //         if let Ok(new_distance) = event.data.parsed::<f64>() {
                //             edge.set_distance(new_distance);
                //             let edge = edge.clone();
                //             spawn(async move { graph_store.update_edge(&edge).await });
                //         }
                //     }
                // },
                }
                label { class: "form-label-check", r#for: input_name, {placeholder.clone()} }
            }
        }
    } else {
        rsx! {
            div { class: "form-floating", "data-mdb-input-init": "",
                input {
                    class: "form-control bg-dark text-light",
                    r#type: input_type,
                    id: input_name.clone(),
                    name: input_name.clone(),
                    placeholder: placeholder.clone(),
                    value: init_value,
                    "readonly": readonly,
                    onchange: {
                        move |event: Event<FormData>| {
                            match node_change {
                                NodeChange::Name(_) => {
                                    if let Ok(name) = event.data.parsed::<String>() {
                                        node_change_signal.set(Some(NodeChange::Name(name)));
                                    }
                                }
                                NodeChange::LIDT(_) => {
                                    if let Ok(lidt) = event.data.parsed::<f64>() {
                                        node_change_signal.set(Some(NodeChange::LIDT(lidt)));
                                    }
                                }
                                NodeChange::TranslationX(_) => {
                                    if let Ok(trans_x) = event.data.parsed::<f64>() {
                                        node_change_signal
                                            .set(
                                                Some(
                                                    NodeChange::TranslationX(Length::new::<meter>(trans_x)),
                                                ),
                                            );
                                    }
                                }
                                NodeChange::TranslationY(_) => {
                                    if let Ok(trans_y) = event.data.parsed::<f64>() {
                                        node_change_signal
                                            .set(
                                                Some(
                                                    NodeChange::TranslationY(Length::new::<meter>(trans_y)),
                                                ),
                                            );
                                    }
                                }
                                NodeChange::TranslationZ(_) => {
                                    if let Ok(trans_z) = event.data.parsed::<f64>() {
                                        node_change_signal
                                            .set(
                                                Some(
                                                    NodeChange::TranslationZ(Length::new::<meter>(trans_z)),
                                                ),
                                            );
                                    }
                                }
                                NodeChange::RotationRoll(_) => {
                                    if let Ok(roll) = event.data.parsed::<f64>() {
                                        node_change_signal
                                            .set(
                                                Some(NodeChange::RotationRoll(Angle::new::<degree>(roll))),
                                            );
                                    }
                                }
                                NodeChange::RotationPitch(_) => {
                                    if let Ok(pitch) = event.data.parsed::<f64>() {
                                        node_change_signal
                                            .set(
                                                Some(NodeChange::RotationPitch(Angle::new::<degree>(pitch))),
                                            );
                                    }
                                }
                                NodeChange::RotationYaw(_) => {
                                    if let Ok(yaw) = event.data.parsed::<f64>() {
                                        node_change_signal
                                            .set(
                                                Some(NodeChange::RotationYaw(Angle::new::<degree>(yaw))),
                                            );
                                    }
                                }
                                NodeChange::Inverted(_) => {
                                    if let Ok(inverted) = event.data.parsed::<bool>() {
                                        node_change_signal.set(Some(NodeChange::Inverted(inverted)));
                                    }
                                }
                                NodeChange::NodeConst(_) => {}
                            };
                        }
                    },
                }
                label { class: "form-label", r#for: input_name, {placeholder.clone()} }
            }
        }
    }
}

// #[component]
// pub fn AccordionElement(number: i32, node_attr: NodeAttr) -> Element{
//     rsx!{
//         div{
//             class:"accordion accordion-borderless bg-dark ",
//             id: format!("accordionElement{number}"),
//             div{
//                 class:"accordion-item bg-dark text-light",
//                 h2{
//                     class:"accordion-header",
//                     id:format!("accordionHeading{number}"),
//                     button{
//                         class:"accordion-button collapsed bg-dark text-light",
//                         type:"button",
//                         "data-mdb-collapse-init": "",
//                         "data-mdb-target":format!("#accordionCollapse{number}"),
//                         "aria-expanded":"false",
//                         "aria-controls":format!("accordionCollapse{number}"),
//                         "General"
//                     }
//                 }
//                 div{
//                     id:format!("accordionCollapse{number}"),
//                     class:"accordion-collapse collapse  bg-dark",
//                     "aria-labelledby":format!("accordionHeading{number}"),
//                     "data-mdb-parent":format!("#accordionElement{number}"),
//                     div{
//                         class:"accordion-body  bg-dark",
//                         p{{format!("ID: {}", node_attr.uuid())}}
//                         p{ {format!("Node Type: {}", node_attr.node_type())} }
//                         p { {format!("Node Name: {}", node_attr.name())} }
//                         p { {format!("LIDT: {:4.2} J/cm²", node_attr.lidt().value/10000.)} }
//                     }
//                 }
//             }
//         }
//     }
// }
// const MAIN_CSS: Asset = asset!("./assets/main.css");
// // const PLOTLY_JS: Asset = asset!("./assets/plotly.js");
// // const THREE_MOD_JS: Asset = asset!("./assets/three_mod.js");
// // const ORBIT_CTRLS: Asset = asset!("./assets/orbitControls.js");
// const MDB_CSS: Asset = asset!("./assets/mdb.min.css");
// const MDB_JS: Asset = asset!("./assets/mdb.umd.min.js");
// const MDB_SUB_CSS: Asset = asset!("./assets/mdb_submenu.css");
// #[component]
// pub fn UserInfoWindow() -> Element {
//     let menu_item_selected = use_signal(|| None::<MenuSelection>);

// 	rsx! {
//         document::Stylesheet { href: MAIN_CSS }
//         document::Stylesheet { href: MDB_CSS }
//         document::Stylesheet { href: MDB_SUB_CSS }
//         document::Script { src: MDB_JS }
//         div { class: "d-flex flex-column text-bg-dark vh-100",
//             div {
//                 MenuBar { menu_item_selected }
//             }
//         }
// 	}
// }
