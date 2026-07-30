use std::{
    process::Child,
    sync::{Arc, Mutex},
};
mod api;
mod app_config;
mod components;

use api::http_client::HTTPClient;
use app_config::AppConfig;
use components::{context_menu::cx_menu::CxMenu, logger::Logs};
use dioxus::signals::{GlobalSignal, Signal};
use opossum_core::types::api_types::NodeEditorPanel;
use uuid::Uuid;

pub use components::app::App;

static OPOSSUM_UI_LOGS: GlobalSignal<Logs> = Signal::global(Logs::new);
pub static HTTP_API_CLIENT: GlobalSignal<HTTPClient> = Signal::global(HTTPClient::new);
static CONTEXT_MENU: GlobalSignal<Option<CxMenu>> = Signal::global(|| None::<CxMenu>);
/// Bumped whenever a `DocumentChange` (from undo/redo) touches node/analyzer details that aren't
/// mirrored into `GraphStore` - the properties panel reads this as an extra, always-changing dependency
/// so it refetches even when the selected node's identity hasn't changed (which Dioxus's equality-dedup'd
/// memos would otherwise treat as "nothing to do").
static NODE_DETAILS_REFRESH: GlobalSignal<usize> = Signal::global(|| 0);
/// Set when `apply_document_changes` decides an undo/redo affected a node other than the one(s)
/// currently selected in the active tab - the node it just switched to and the panel to open once
/// its editor has loaded. Consumed (cleared) by whichever `OpticalNodeEditor`/`PortConfigEditor`
/// instance matches the uuid.
static PENDING_PANEL_OPEN: GlobalSignal<Option<(Uuid, NodeEditorPanel)>> = Signal::global(|| None);
/// The `(graph_id, node_id)` an undo/redo last auto-selected, if any - preferred again by the next
/// call if it's still a member of that call's own affected set, so e.g. a redo right after an undo
/// stays put instead of jumping to a *different* member of the same reported set purely because
/// `Command::Batch` reverses a multi-tab cascade's order between undo and redo. Ignored (falls back to
/// picking the first-reported entry) whenever it isn't relevant to the current response.
static LAST_AUTO_SELECTED_NODE: GlobalSignal<Option<(Uuid, Uuid)>> = Signal::global(|| None);
/// Same idea as `LAST_AUTO_SELECTED_NODE`, for the structural (no specific node to select) tab-jump -
/// but only ever consulted as a fallback, for response shapes that don't (yet) tag a reliable origin
/// `graph_id` at the source (`describe_move_nodes`/`describe_group_structure_change`). Whenever a
/// `DocumentChange` entry *does* carry that information (an unambiguous single-tab change, or a
/// `GraphNeedsRefresh` with `is_origin: true`), `apply_document_changes` uses it directly instead of
/// this heuristic - see its `primary_structural_graph_id` local.
static LAST_AUTO_JUMPED_GRAPH: GlobalSignal<Option<Uuid>> = Signal::global(|| None);
/// `(can_undo, can_redo)` availability, mirrored from the backend's undo/redo stacks. Every edit path
/// (canvas coroutine, node editor, viewport gestures) writes this so the Edit menu's Undo/Redo entries
/// reflect reality; the backend is the source of truth on undo/redo. A global (rather than a
/// prop-threaded handler) so all edit paths can update it without plumbing.
static UNDO_REDO_STATUS: GlobalSignal<(bool, bool)> = Signal::global(|| (false, false));

pub static APP_CONFIG: GlobalSignal<AppConfig> = Signal::global(|| {
    AppConfig::from_file().unwrap_or_else(|_| {
        // Use default if loading fails
        let default_config = AppConfig::default();
        // Try to write new config
        if let Err(e) = default_config.to_file() {
            eprintln!("Warning: Couldn't write default config: {e}");
        }
        default_config
    })
});

#[derive(Clone, Default)]
pub struct ProcessHandle {
    #[allow(dead_code)]
    inner: Option<Arc<Mutex<Child>>>,
}

#[cfg(not(debug_assertions))]
impl ProcessHandle {
    pub fn new(child: Child) -> Self {
        Self {
            inner: Some(Arc::new(Mutex::new(child))),
        }
    }
    pub fn kill(&self) {
        println!("Attempting to terminate backend server...");
        if let Some(child) = &self.inner {
            let mut handle = child.lock().unwrap();
            match handle.kill() {
                Ok(_) => {
                    // Wait for the process to ensure it's fully cleaned up
                    let _ = handle.wait();
                    println!("Backend server terminated successfully.");
                }
                Err(e) => eprintln!("Error terminating backend server: {}", e),
            }
        }
    }
}
