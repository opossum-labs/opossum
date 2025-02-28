use std::sync::Mutex;

use opossum::nodes::NodeGroup;

#[derive(Default)]
pub struct AppState {
    pub scenery: Mutex<NodeGroup>,
}
impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            scenery: Mutex::new(self.scenery.lock().unwrap().clone()),
        }
    }
}
