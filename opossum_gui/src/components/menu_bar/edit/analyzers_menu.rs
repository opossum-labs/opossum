use dioxus::prelude::*;
// use crate::{api, HTTP_API_CLIENT, OPOSSUM_UI_LOGS};

#[component]
pub fn AnalyzersMenu(analyzer_selected: Signal<String>) -> Element {
    // let future = use_resource({
    //     move || async move {
    //         match api::get_node_types(&HTTP_API_CLIENT()).await {
    //             Ok(node_types) => Some(node_types),
    //             Err(err_str) => {
    //                 OPOSSUM_UI_LOGS.write().add_log(&err_str);
    //                 None
    //             }
    //         }
    //     }
    // });

    // let analyzer_list = match &*future.read_unchecked() {
    //     Some(Some(response)) => response
    //         .iter()
    //         .map(|n| format!("{n}"))
    //         .collect::<Vec<String>>(),
    //     Some(None) => vec!["error receiving analyzer list from server".to_owned()],
    //     _ => vec!["loading analyzer list from server".to_owned()],
    // };
    let analyzer_list: Vec<String> =
        vec!["Energy".into(), "Raytracing".into(), "GhostFocus".into()];
    rsx! {
        for element in analyzer_list.into_iter() {
            {
                rsx! {
                    li {
                        a {
                            class: "dropdown-item",
                            role: "button",
                            onclick: move |_| analyzer_selected.set(element.clone()),
                            {element.clone()}
                        }
                    }
                }
            }
        }
    }
}
