use std::sync::Mutex;

use opossum::OpmDocument;

#[derive(Default)]
pub struct AppState {
    pub document: Mutex<OpmDocument>,
}
impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            document: Mutex::new(self.document.lock().unwrap().clone()),
        }
    }
}
