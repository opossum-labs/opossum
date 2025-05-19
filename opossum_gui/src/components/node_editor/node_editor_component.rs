
use nalgebra::Point3;
use opossum_backend::{Isometry, NodeAttr};
use uom::si::{angle::degree, f64::Length};
use uuid::Uuid;
use dioxus::prelude::*;
use uom::si::f64::Angle;
use crate::{api, components::scenery_editor::node::NodeElement, HTTP_API_CLIENT, OPOSSUM_UI_LOGS};

#[derive(Debug)]
pub enum NodeChange {
    Name(String),
    LIDT(f64),
    Translation(Point3<Length>),
    Rotation(Point3<Angle>),
    Inverted(bool),
    // AlignLikeNodeAtDistance(Uuid, Length),
}

#[component]
pub fn NodeEditor(node: ReadOnlySignal<Option<NodeElement>>, changed: Signal<bool>) -> Element {
    let node_change = use_context_provider(|| Signal::new(None::<NodeChange>));

    // use_effect(move || {
    //     let node_change = node_change.read();
    //     if let Some(node_change) = &*(node_change) {
    //         match node_change {
    //             NodeChange::Name(name) => {
    //                 if let Some(node) = *node.read() {
    //                     spawn(async move {
    //                         api::set_node_name(&HTTP_API_CLIENT(), node.id(), name.clone())
    //                         .await
    //                         .unwrap()});
                        
    //                 }
    //             }
    //             NodeChange::LIDT(lidt) => {
    //                     if let Some(node) = *node.read() {
    //                         spawn(async move {
    //                             api::set_node_lidt(&HTTP_API_CLIENT(), node.id(), lidt.clone())
    //                             .await
    //                             .unwrap()});
    //                     }
    //             }
    //             _ => {}
    //         }
    //     }});


    let resource_future = use_resource(move || async move{
        let node = node.read();
        if let Some(node) = &*(node) {       
            match api::get_node_properties(&HTTP_API_CLIENT(), node.id())
                .await
            {
                Ok(node_attr) => Some(node_attr),
                Err(err_str) => {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                    None},
            }
        }
        else {
            None
        }
        });

    if let Some(Some(node_attr)) = &*resource_future.read_unchecked() {
        rsx!{
            div {
                h6 { "Node Configuration" }
                div{
                    class:"accordion accordion-borderless bg-dark ",
                    id:"accordionGeneral",
                    div{  
                        class:"accordion-item bg-dark text-light",
                        h2{
                            class:"accordion-header",
                            id:"generalHeading",
                            button{
                                class:"accordion-button collapsed bg-dark text-light",
                                type:"button",
                                "data-mdb-collapse-init": "",
                                "data-mdb-target":"#generalCollapse",
                                "aria-expanded":"false",
                                "aria-controls":"generalCollapse",
                                "General"
                            }
                        }
                        div{
                            id:"generalCollapse",
                            class:"accordion-collapse collapse  bg-dark",
                            "aria-labelledby":"generalHeading",
                            "data-mdb-parent":"#accordionGeneral",
                            div{
                                class:"accordion-body  bg-dark",
                                NodePropInput{
                                    name: "NodeId".to_string(),
                                    input_type: "text".to_string(),
                                    readonly: true,
                                    placeholder: "Node ID".to_string(),
                                    init_value: format!("{}", node_attr.uuid()),
                                }
                                NodePropInput{
                                    name: "NodeType".to_string(),
                                    input_type: "text".to_string(),
                                    readonly: true,
                                    placeholder: "Node Type".to_string(),
                                    init_value: node_attr.node_type().to_string(),

                                }
                                NodePropInput{
                                    name: "NodeName".to_string(),
                                    input_type: "text".to_string(),
                                    readonly: false,
                                    placeholder: "Node Name".to_string(),
                                    init_value: node_attr.name().to_string(),
                                    

                                }
                                NodePropInput{
                                    name: "LIDT".to_string(),
                                    input_type: "number".to_string(),
                                    readonly: false,
                                    placeholder: "LIDT in J/cm²".to_string(),
                                    init_value: format!("{:.2}", node_attr.lidt().value/10000.),

                                }
                            }
                        }
                    }
                    
                }
                div{
                    class:"accordion accordion-borderless bg-dark ",
                    id:"accordionAlignment",
                    div{  
                        class:"accordion-item bg-dark text-light",
                        h2{
                            class:"accordion-header",
                            id:"alignmentHeading",
                            button{
                                class:"accordion-button collapsed bg-dark text-light",
                                type:"button",
                                "data-mdb-collapse-init": "",
                                "data-mdb-target":"#alignmentCollapse",
                                "aria-expanded":"false",
                                "aria-controls":"alignmentCollapse",
                                "Alignment"
                            }
                        }
                        div{
                            id:"alignmentCollapse",
                            class:"accordion-collapse collapse  bg-dark",
                            "aria-labelledby":"alignmentHeading",
                            "data-mdb-parent":"#accordionAlignment",
                            div{
                                class:"accordion-body  bg-dark",
                                NodePropInput{
                                    name: "XTranslation".to_string(),
                                    input_type: "number".to_string(),
                                    readonly: false,
                                    placeholder: "X Translation in m".to_string(),
                                    init_value: format!("{:.6}", node_attr.alignment().as_ref().map_or(0., |a| a.translation().x.value)),

                                }
                                NodePropInput{
                                    name: "YTranslation".to_string(),
                                    input_type: "number".to_string(),
                                    readonly: false,
                                    placeholder: "Y Translation in m".to_string(),
                                    init_value: format!("{:.6}", node_attr.alignment().as_ref().map_or(0., |a| a.translation().y.value)),

                                }
                                NodePropInput{
                                    name: "ZTranslation".to_string(),
                                    input_type: "number".to_string(),
                                    readonly: false,
                                    placeholder: "Z Translation in m".to_string(),
                                    init_value: format!("{:.6}", node_attr.alignment().as_ref().map_or(0., |a| a.translation().z.value)),

                                }
                                NodePropInput{
                                    name: "Roll".to_string(),
                                    input_type: "number".to_string(),
                                    readonly: false,
                                    placeholder: "Roll angle in degree".to_string(),
                                    init_value: format!("{:.6}", node_attr.alignment().as_ref().map_or(0., |a| a.rotation().x.get::<degree>())),

                                }
                                NodePropInput{
                                    name: "Pitch".to_string(),
                                    input_type: "number".to_string(),
                                    readonly: false,
                                    placeholder: "Pitch angle in degree".to_string(),
                                    init_value: format!("{:.6}", node_attr.alignment().as_ref().map_or(0., |a| a.rotation().y.get::<degree>())),

                                }
                                NodePropInput{
                                    name: "Yaw".to_string(),
                                    input_type: "number".to_string(),
                                    readonly: false,
                                    placeholder: "Yaw angle in degree".to_string(),
                                    init_value: format!("{:.6}", node_attr.alignment().as_ref().map_or(0., |a| a.rotation().z.get::<degree>())),
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
            div { "No node selected" 
            // this opens a window but currently (dixous v0.6.3) this does not work
            // https://github.com/DioxusLabs/dioxus/issues/3080

            // button {
            //     onclick: move |_| {
            //         let mut dom = VirtualDom::new(UserInfoWindow);
            //         // dom.rebuild_to_vec();
			// 		dioxus::desktop::window().new_window(dom, dioxus::desktop::Config::new()
            //         .with_icon(
            //             Icon::from_path("./assets/favicon.ico", None).expect("Could not load icon"),
            //         ));
            //     },
            //     "Open Config Window"
            // }
            }
        }
        
    }

}

#[component]
pub fn NodePropInput(name: String, input_type: String, readonly: bool, placeholder: String, init_value: String
    ) -> Element{
    let mut node_change_signal = use_context::<Signal<Option<NodeChange>>>();
    let input_name = format!("prop{name}");
    rsx!{
        div{ 
        class:"form-floating", 
        "data-mdb-input-init":"",
        input{
            class:"form-control bg-dark text-light",
            type:input_type, 
            id:input_name.clone(), 
            name:input_name.clone(), 
            placeholder:placeholder.clone(), 
            value: init_value,
            "readonly":readonly,
            // onchange: {
            //     move |event: Event<FormData>| {
            //         if let Ok(new_distance) = event.data.parsed::<f64>() {
            //             println!("new number {new_distance}");
            //             edge.set_distance(new_distance);
            //             let edge = edge.clone();
            //             spawn(async move { graph_store.update_edge(&edge).await });
            //         }
            //     }
            // }
        } 
        label 
        {
            class:"form-label",
            for:input_name,
            {placeholder.clone()}}
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
