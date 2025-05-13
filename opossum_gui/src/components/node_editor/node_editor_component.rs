use dioxus::{desktop::tao::{platform::windows::{self, IconExtWindows}, window::Icon}, prelude::*};
use uuid::Uuid;

use crate::{api, components::{app, menu_bar::menu_bar_component::{MenuBar, MenuSelection}}, HTTP_API_CLIENT, OPOSSUM_UI_LOGS};

#[component]
pub fn NodeEditor(node: ReadOnlySignal<Option<Uuid>>) -> Element {
    let resource_future = use_resource(move || async move{
        let node_uuid = node.read();
        if let Some(node_uuid) = &*(node_uuid) {       
            println!("NodeEditor: {:?}", node_uuid);
            match api::get_node_properties(&HTTP_API_CLIENT(), *node_uuid)
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
                h5 { "Node Properties" }
                div{
                    class:"accordion accordion-borderless bg-dark ",
                    id:"accordionFlushExample",
                    div{  
                        class:"accordion-item bg-dark text-light",
                        h2{
                            class:"accordion-header",
                            id:"flush-headingOne",
                            button{
                                class:"accordion-button collapsed bg-dark text-light",
                                type:"button",
                                "data-mdb-collapse-init": "",
                                "data-mdb-target":"#flush-collapseOne",
                                "aria-expanded":"false",
                                "aria-controls":"flush-collapseOne",
                                "header"
                            }
                        }
                        div{
                            id:"flush-collapseOne",
                            class:"accordion-collapse collapse  bg-dark",
                            "aria-labelledby":"flush-headingOne",
                            "data-mdb-parent":"#accordionFlushExample",
                            div{
                                class:"accordion-body  bg-dark",
                                "test"
                            }
                        }
                    }
                }
                p { {format!("ID: {}", node_attr.uuid())} }
                p { {format!("Node Type: {}", node_attr.node_type())} }
                p { {format!("Node Name: {}", node_attr.name())} }
                p { {format!("LIDT: {:4.2} J/cm²", node_attr.lidt().value/10000.)} }
                

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