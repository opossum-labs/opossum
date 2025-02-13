use std::collections::HashMap;

use gloo_utils::document;
use log::debug;
use serde_json::Value;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;
use yew::{use_state, Callback, DragEvent, NodeRef, TargetCast, UseReducerHandle, UseStateHandle};

use super::node_element::{Connection, NodeAction, NodeStates};

#[derive(Clone, PartialEq)]
pub struct NodeCallbacks {
    pub on_drag_start: Callback<DragEvent>,
    pub on_drag_end: Callback<(Uuid, i32, i32)>,
    pub on_port_click: Callback<(Uuid, String)>,
    pub on_node_double_click: Callback<Value>,
    pub on_add_log: Callback<String>,
}

pub fn create_node_double_click_callback(node_states: UseReducerHandle<NodeStates>) -> Callback<Value> {
    Callback::from(move |json_val: Value| {
        node_states.dispatch(NodeAction::NodeDoubleClick(json_val.clone()));
    })
}

pub fn create_drag_start_callback(node_states: UseReducerHandle<NodeStates>) -> Callback<DragEvent> {
    Callback::from(move |event: DragEvent| {
        if let Some(target) = event.target_dyn_into::<HtmlElement>() {
            let rect = target.get_bounding_client_rect();
            let offset_x = event.page_x() as i32 - rect.left() as i32;
            let offset_y = event.page_y() as i32 - rect.top() as i32;
            if let Some(node_id) = target.get_elements_by_class_name("node-content").get_with_index(0).map(|n|n.id()){
                node_states.dispatch(NodeAction::SetDragStartOffset((offset_x, offset_y), node_id));
            }
            // debug!("nr of elements:{}",target.get_elements_by_class_name("node-content").);
        }
    })
}

pub fn create_drag_end_callback(node_states: UseReducerHandle<NodeStates>) -> Callback<(Uuid, i32, i32)> {
        let node_states = node_states.clone();
        Callback::from(move |(id, x, y): (Uuid, i32, i32)| {
            if let Some(updated_node) = node_states.nodes().iter().find(|node| node.id() == id).clone(){
                let mut node =updated_node.clone();
                node.set_x(x -node.offset().0); // X-Position mit Offset aktualisieren
                node.set_y(y -node.offset().1); // Y-Position mit Offset aktualisieren
                node_states.dispatch(NodeAction::UpdateNode(node.clone())); 
            }
        })
}

pub fn create_add_log_callback(logs: UseStateHandle<Vec<String>>) -> Callback<String> {
    Callback::from(move |log_msg: String| {
        let mut new_logs = (*logs).clone();
        new_logs.push(format!("{}", log_msg));
        logs.set(new_logs);

        // Scroll down automatically
        if let Some(Ok(log_container)) = document().get_element_by_id("log-container").map(|e| e.dyn_into::<web_sys::HtmlElement>()){
            log_container.set_scroll_top(log_container.scroll_height());
        }
    })
}


fn create_on_port_click_callback(
    node_states: UseReducerHandle<NodeStates>,
) -> Callback<(Uuid, String)> {
    Callback::from(move |(node_id, port_type): (Uuid, String)| {
        if let Some((selected_id, selected_type)) = node_states.selected_port().clone() {
            if selected_type != port_type && selected_id != node_id {
                let to_id = if selected_type == "input" {
                    selected_id
                } else {
                    node_id
                };
                let mut conns = node_states.connections().clone();
                if conns.check_connection_validity(to_id)
                {
                    conns.insert_connection(selected_type, selected_id, node_id);
                    node_states.dispatch(NodeAction::UpdateConnections(conns.clone()));
                }
                node_states.dispatch(NodeAction::SelectPort(None));
            }
        } else {
            // Aktuellen Port auswählen
            node_states.dispatch(NodeAction::SelectPort(Some((node_id, port_type))));
        }
    })
}

pub fn create_node_callbacks(
    node_states: UseReducerHandle<NodeStates>,
    // offset: UseStateHandle<(i32, i32)>,
    logs: UseStateHandle<Vec<String>>,
) -> NodeCallbacks {
    
    NodeCallbacks {
        on_drag_start: create_drag_start_callback(node_states.clone()),
        on_drag_end: create_drag_end_callback(node_states.clone()),
        on_port_click: create_on_port_click_callback( node_states.clone()),
        on_node_double_click: create_node_double_click_callback(node_states.clone()),
        on_add_log: create_add_log_callback(logs.clone()),
    }
}