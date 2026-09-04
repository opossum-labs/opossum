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
    Analyzer(Box<AnalyzerItemDto>),
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::undo::{Command, SetViewport};
    use opossum_core::{core_optics::node_attr::HasNodeAttr, types::api_types::Viewport};
    use uuid::Uuid;

    // Helper to generate a lightweight command for undo stack testing
    fn dummy_command() -> Command {
        Command::SetViewport(SetViewport {
            from: Viewport {
                graph_id: Uuid::nil(),
                zoom: 1.0,
                shift: (0.0, 0.0),
            },
            to: Viewport {
                graph_id: Uuid::nil(),
                zoom: 2.0,
                shift: (10.0, 10.0),
            },
            coalescing: false,
        })
    }

    #[test]
    fn test_push_undo_clears_redo_stack() {
        let state = AppState::default();

        // Populate redo stack to ensure a new undo push invalidates pending redos
        state.redo_stack.lock().push_back(dummy_command());
        assert_eq!(state.redo_stack.lock().len(), 1);

        state.push_undo(dummy_command());

        assert_eq!(state.undo_stack.lock().len(), 1);
        assert!(state.redo_stack.lock().is_empty());
    }

    #[test]
    fn test_max_undo_depth_eviction() {
        let state = AppState::default();

        // Push more items than the defined maximum limit
        for _ in 0..(MAX_UNDO_DEPTH + 15) {
            state.push_undo(dummy_command());
        }

        // Verify that the stack size is clamped to MAX_UNDO_DEPTH
        assert_eq!(state.undo_stack.lock().len(), MAX_UNDO_DEPTH);
    }

    #[test]
    fn test_clear_undo_history() {
        let state = AppState::default();

        state.undo_stack.lock().push_back(dummy_command());
        state.redo_stack.lock().push_back(dummy_command());

        assert!(!state.undo_stack.lock().is_empty());
        assert!(!state.redo_stack.lock().is_empty());

        state.clear_undo_history();

        assert!(state.undo_stack.lock().is_empty());
        assert!(state.redo_stack.lock().is_empty());
    }

    #[test]
    fn test_app_state_clone_semantics() {
        let state = AppState::default();
        state.undo_stack.lock().push_back(dummy_command());
        state.redo_stack.lock().push_back(dummy_command());

        let cloned = state.clone();

        // History and copy cache must be clean in the cloned instance
        assert!(cloned.undo_stack.lock().is_empty());
        assert!(cloned.redo_stack.lock().is_empty());
        assert!(cloned.node_copy_cache.lock().is_empty());

        // The document itself should be cloned identically
        assert_eq!(
            state.document.lock().scenery().node_attr().name(),
            cloned.document.lock().scenery().node_attr().name()
        );
    }
}
