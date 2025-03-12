use std::sync::Mutex;

use opossum::{analyzers::AnalyzerType, nodes::NodeGroup, SceneryResources};

#[derive(Default)]
pub struct AppState {
    pub scenery: Mutex<NodeGroup>,
    pub global_conf: Mutex<SceneryResources>,
    pub analyzers: Mutex<Vec<AnalyzerType>>,
}
impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            scenery: Mutex::new(self.scenery.lock().unwrap().clone()),
            global_conf: Mutex::new(self.global_conf.lock().unwrap().clone()),
            analyzers: Mutex::new(self.analyzers.lock().unwrap().clone()),
        }
    }
}
