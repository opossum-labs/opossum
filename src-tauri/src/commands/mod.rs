use std::sync::Mutex;
use tauri::{command, State};
use uuid::Uuid;

use crate::OPMGUIModel;

// Eine Methode, um den Graphen zu ändern
#[command]
pub fn add_node(state: State<Mutex<OPMGUIModel>>, node_type: String) -> Result<String, String> {
    Ok("success".to_string())
    
    // if let Ok(state) = &mut state.lock(){
    //    if let Ok(uuid) = state.add_default_node(&node_type){
    //         // return Ok(uuid);
    //    }else{
    //     Err("error_on_adding_node".to_string())
    //         // return Err("Error on adding node".into());
    //    }
    // }
    // else{
    //     Err("error_on_locking_state".to_string())
    //     // return Err("Error on locking state".into());
    // }
}

#[command]
pub fn print_beep_boop() -> Result<String, String> {
    Ok("beep boop".to_string())
}
