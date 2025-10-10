use actix_web::dev::ServerHandle;
use opossum_core::{OpticRef, opm_document::OpmDocument};
use parking_lot::Mutex;

#[derive(Default)]
pub struct AppState {
    pub document: Mutex<OpmDocument>,
    pub server_handle: Mutex<Option<ServerHandle>>,
    pub node_copy_cache: Mutex<Option<OpticRef>>,
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
            node_copy_cache: Mutex::new(None::<OpticRef>),
        }
    }
}
