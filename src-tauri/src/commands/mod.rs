use serde::Serialize;
use std::sync::Mutex;
use tauri::{command, State};
use uuid::Uuid;

use crate::OPMGUIModel;

// Eine Methode, um den Graphen zu ändern
#[command]
pub async fn add_node(
    state: State<'_, Mutex<OPMGUIModel>>,
    node_type: String,
) -> Result<String, String> {
    if let Ok(state) = &mut state.lock() {
        if let Ok(uuid) = state.add_default_node(&node_type) {
            Ok(uuid.as_simple().to_string())
        } else {
            Err("Error on adding node".into())
        }
    } else {
        Err("Error on locking state".into())
    }
}

// Eine Methode, um den Graphen zu ändern
#[command]
pub async fn get_node_info(
    state: State<'_, Mutex<OPMGUIModel>>,
    node_id: String,
) -> Result<String, String> {
    if let Ok(state) = &state.lock() {
        let model = state.model();
        let node_id = Uuid::parse_str(&node_id).map_err(|e| e.to_string())?; // Parse the uuid

        if let Some(node) = model.graph().node_by_uuid(node_id) {
            let optic_ref = node
                .optical_ref
                .lock()
                .map_err(|_| "Mutex lock failed".to_string())?;
            let node_attr = optic_ref.node_attr();
            serde_json::to_string(&node_attr).map_err(|e| e.to_string())
        } else {
            Err(format!("No nodes associated with uuid: {}", node_id))
        }
    } else {
        Err("Error on locking state".into())
    }
}

#[command]
pub fn set_inverted(
    state: State<'_, Mutex<OPMGUIModel>>,
    node_id: String,
    inverted: bool,
) -> Result<String, String> {
    if let Ok(state) = &mut state.lock() {
        let model = state.model_mut();
        let node_id = Uuid::parse_str(&node_id).map_err(|e| e.to_string())?; // Parse the uuid
        if let Some(node) = &mut model.graph().node_by_uuid(node_id) {
            let mut optic_ref = node
                .optical_ref
                .lock()
                .map_err(|_| "Mutex lock failed".to_string())?;
            let node_attr = optic_ref.node_attr_mut();
            node_attr.set_inverted(inverted);
            serde_json::to_string(&node_attr).map_err(|e| e.to_string())
        } else {
            Err(format!("No nodes associated with uuid: {}", node_id))
        }
    } else {
        Err("Error on locking state".into())
    }
}

#[command]
pub fn set_name(
    state: State<'_, Mutex<OPMGUIModel>>,
    node_id: String,
    name: String,
) -> Result<String, String> {
    if let Ok(state) = &mut state.lock() {
        let model = state.model_mut();
        let node_id = Uuid::parse_str(&node_id).map_err(|e| e.to_string())?; // Parse the uuid
        if let Some(node) = &mut model.graph().node_by_uuid(node_id) {
            let mut optic_ref = node
                .optical_ref
                .lock()
                .map_err(|_| "Mutex lock failed".to_string())?;
            let node_attr = optic_ref.node_attr_mut();
            node_attr.set_name(name.as_str());
            serde_json::to_string(&node_attr).map_err(|e| e.to_string())
        } else {
            Err(format!("No nodes associated with uuid: {}", node_id))
        }
    } else {
        Err("Error on locking state".into())
    }
}
