use dioxus::prelude::*;
use uuid::Uuid;

use crate::components::{
    logger::logger_component::Logger,
    node_property_config::node_config_menu::NodePropertyConfigMenu,
};

#[component]
pub fn App() -> Element {
    use_context_provider(|| Signal::new(Uuid::nil()));
    // let mut main_window = use_signal(|| None::<Rc<MountedData>>);
    rsx! {
        // div { class: "container-fluid",
        // div { class: "row",
        //     div { class: "d-flex h-100 text-bg-dark",
        //         div { class: "col-2 flex-shrink-0", NodePropertyConfigMenu {} }
        //         div { class: "col", "Node Editor" }
        //     }
        // }
        div { class: "d-flex flex-column h-100 text-bg-dark",

                div { class: "container-fluid",
                    div { class: "row",
                        div { class: "col-2", NodePropertyConfigMenu {} }
                        div { class: "col", "Node Editor" }
                    }
                }

            footer {
                class: "footer mt-auto py-2",
                style: "background-color:rgb(119, 119, 119);",
                div { class: "container-fluid", Logger {} }
            }
        }
    }
}
