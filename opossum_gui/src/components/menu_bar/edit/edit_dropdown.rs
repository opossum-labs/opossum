use dioxus::prelude::*;
use opossum_backend::AnalyzerType;
use uuid::Uuid;

use crate::{
    components::{
        menu_bar::{edit::node_dropdown_menu::NodeDropDownMenu, sub_menu_item::MenuItem},
        node_components::NodesStore,
    },
    HTTP_API_CLIENT, NODES_STORE, OPOSSUM_UI_LOGS,
};

#[component]
pub fn EditDropdownMenu() -> Element {
    let future = use_resource({
        move || async move {
            match HTTP_API_CLIENT().get_node_types().await {
                Ok(node_types) => Some(node_types),
                Err(err_str) => {
                    OPOSSUM_UI_LOGS.write().add_log(err_str);
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
    rsx! {
        div { class: "title-bar-item dropdown",
            "Edit"
            div { class: "dropdown-content title-bar-dropdown-content",
                NodeDropDownMenu {
                    title: "Add Node",
                    element_list: node_list,
                    on_element_click: use_add_node,
                }
                NodeDropDownMenu {
                    title: "Add Analyzer",
                    element_list: NodesStore::available_analyzers(),
                    on_element_click: use_add_analyzer,
                }
                MenuItem {
                    class: "title-bar-submenu-item".to_owned(),
                    onclick: Some(use_delete_scenery()),
                    display: "Clear Scenery".to_owned(),
                }
            }
        }
    }
}

pub fn use_delete_scenery() -> Callback<Event<MouseData>> {
    use_callback(move |_: Event<MouseData>| {
        spawn(async move {
            match HTTP_API_CLIENT().delete_scenery().await {
                Ok(_) => {
                    NODES_STORE.write().delete_nodes();
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log("Scenery cleared successfully!".to_owned());
                }
                Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(err_str),
            }
        });
    })
}

pub fn use_add_node(n_type: String, group_id: Uuid) -> Callback<Event<MouseData>> {
    use_callback(move |_: Event<MouseData>| {
        let n_type = n_type.clone();
        spawn(async move {
            match HTTP_API_CLIENT().post_add_node(n_type, group_id).await {
                Ok(node_info) => match HTTP_API_CLIENT()
                    .get_node_properties(node_info.uuid())
                    .await
                {
                    Ok(node_attr) => NODES_STORE.write().add_node(node_info, node_attr),
                    Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(err_str),
                },
                Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(err_str),
            }
        });
    })
}

pub fn use_add_analyzer(analyzer: AnalyzerType, _group_id: Uuid) -> Callback<Event<MouseData>> {
    use_callback(move |_: Event<MouseData>| {
        let analyzer = analyzer.clone();
        spawn(async move {
            match HTTP_API_CLIENT().post_add_analyzer(analyzer.clone()).await {
                Ok(_) => {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log(format!("Added analyzer: {}", analyzer));
                    NODES_STORE.write().add_analyzer(analyzer)
                }
                Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(err_str),
            }
        });
    })
}
