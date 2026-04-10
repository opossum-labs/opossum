use actix_web::dev::ServerHandle;
use opossum_core::{
    core_optics::OpticRef,
    opm_document::{AnalyzerInfo, OpmDocument},
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct AppState {
    pub document: Mutex<OpmDocument>,
    pub server_handle: Mutex<Option<ServerHandle>>,
    pub node_copy_cache: Mutex<Vec<NodeCacheItem>>,
}
impl AppState {
    /// Sets the server handle to stop.
    pub fn register_server_handle(&self, handle: ServerHandle) {
        *self.server_handle.lock() = Some(handle);
    }
}
impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            document: Mutex::new(self.document.lock().clone()),
            server_handle: Mutex::new(self.server_handle.lock().clone()),
            node_copy_cache: Mutex::new(Vec::new()),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum NodeCacheItem {
    Optical(OpticRef),
    Analyzer(AnalyzerInfo),
}
