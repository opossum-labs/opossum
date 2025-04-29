use dioxus::prelude::*;
use opossum_backend::{nodes::NewNode, scenery::NewAnalyzerInfo, AnalyzerType};
use uuid::Uuid;

use crate::{
    api::{self},
    components::scenery_editor::NODES_STORE,
    HTTP_API_CLIENT, OPOSSUM_UI_LOGS,
};

pub fn delete_scenery() {
    spawn(async move {
        match api::delete_scenery(&HTTP_API_CLIENT()).await {
            Ok(_) => {
                NODES_STORE.write().delete_nodes();
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log("Scenery cleared successfully!");
            }
            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
        }
    });
}

pub fn add_node(n_type: String, group_id: Uuid) {
    let new_node_info = NewNode::new(n_type, (0, 0, 0));
    spawn(async move {
        match api::post_add_node(&HTTP_API_CLIENT(), new_node_info, group_id).await {
            Ok(node_info) => {
                match api::get_node_properties(&HTTP_API_CLIENT(), node_info.uuid()).await {
                    Ok(node_attr) => NODES_STORE.write().add_node(&node_info, &node_attr),
                    Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
                }
            }
            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
        }
    });
}

pub fn add_analyzer(analyzer_type: AnalyzerType) {
    let analyzer_type = analyzer_type.clone();
    let new_analyzer_info = NewAnalyzerInfo::new(analyzer_type.clone(), (0, 0, 0));
    spawn(async move {
        match api::post_add_analyzer(&HTTP_API_CLIENT(), new_analyzer_info).await {
            Ok(_) => {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log(&format!("Added analyzer: {analyzer_type}"));
                NODES_STORE.write().add_analyzer(&analyzer_type);
            }
            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
        }
    });
}
