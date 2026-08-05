use std::collections::VecDeque;

use actix_web::dev::ServerHandle;
use opossum_core::{
    core_optics::OpticRef, opm_document::OpmDocument, types::api_types::AnalyzerItemDto,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::undo::Command;

/// Maximum number of entries kept per undo/redo stack. Entries are small (single-field diffs, or at
/// most a handful of captured nodes for a delete/paste), so this is a generous ceiling, not a tight
/// memory budget the way a whole-document-snapshot approach would have needed.
pub const MAX_UNDO_DEPTH: usize = 100;

#[derive(Default)]
pub struct AppState {
    pub document: Mutex<OpmDocument>,
    pub server_handle: Mutex<Option<ServerHandle>>,
    pub node_copy_cache: Mutex<Vec<NodeCacheItem>>,
    pub undo_stack: Mutex<VecDeque<Command>>,
    pub redo_stack: Mutex<VecDeque<Command>>,
}
impl AppState {
    /// Sets the server handle to stop.
    pub fn register_server_handle(&self, handle: ServerHandle) {
        *self.server_handle.lock() = Some(handle);
    }

    /// Pushes `command` onto the undo stack (evicting the oldest entry past [`MAX_UNDO_DEPTH`]) and
    /// clears the redo stack, per standard undo/redo semantics: a new edit invalidates any pending redo.
    pub fn push_undo(&self, command: Command) {
        let mut undo_stack = self.undo_stack.lock();
        undo_stack.push_back(command);
        if undo_stack.len() > MAX_UNDO_DEPTH {
            undo_stack.pop_front();
        }
        drop(undo_stack);
        self.redo_stack.lock().clear();
    }

    /// Clears both the undo and redo stacks - called whenever the document is replaced or reset
    /// (load a file, start a new project), since history from a different document is meaningless.
    pub fn clear_undo_history(&self) {
        self.undo_stack.lock().clear();
        self.redo_stack.lock().clear();
    }
}
impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            document: Mutex::new(self.document.lock().clone()),
            server_handle: Mutex::new(self.server_handle.lock().clone()),
            node_copy_cache: Mutex::new(Vec::new()),
            undo_stack: Mutex::new(VecDeque::new()),
            redo_stack: Mutex::new(VecDeque::new()),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum NodeCacheItem {
    Optical(OpticRef),
    Analyzer(AnalyzerItemDto),
}
