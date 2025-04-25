use dioxus::prelude::*;

use crate::{api, HTTP_API_CLIENT, OPOSSUM_UI_LOGS};

#[component]
pub fn NodesMenu() -> Element {
    let future = use_resource({
        move || async move {
            match api::get_node_types(&HTTP_API_CLIENT()).await {
                Ok(node_types) => Some(node_types),
                Err(err_str) => {
                    OPOSSUM_UI_LOGS.write().add_log(&err_str);
                    None
                }
            }
        }
    });

    let node_list = match &*future.read_unchecked() {
        Some(Some(response)) => response
            .iter()
            .map(|n| format!("{n}"))
            .collect::<Vec<String>>(),
        Some(None) => vec!["error receiving node list from server".to_owned()],
        _ => vec!["loading node list from server".to_owned()],
    };
    rsx!{
        for element in node_list.into_iter() {
            {
                rsx! {
                    li {
                        a { class: "dropdown-item", role: "button", {element} }
                    }
                }
            }
        }
    }
}