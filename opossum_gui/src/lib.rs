use std::{
    process::Child,
    sync::{Arc, Mutex},
};
mod api;
mod app_config;
mod components;

use api::http_client::HTTPClient;
use app_config::AppConfig;
use components::{context_menu::cx_menu::CxMenu, logger::Logs, scenery_editor::SidebarView};
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
/// Bumped whenever the document changed in a way that can alter the *set* of amplifying nodes -
/// every document-mutating workspace action plus loading/clearing a document.
///
/// The amplifier overview panel is permanently visible once selected, so unlike the node-editor
/// panels it is never remounted by a selection change and would otherwise show a stale list after a
/// delete, paste, group or undo. [`NODE_DETAILS_REFRESH`] alone does not cover those: it is only
/// raised for property-level edits.
static AMP_LIST_REFRESH: GlobalSignal<usize> = Signal::global(|| 0);
/// Which view the sidebar shows, whether it is collapsed to its icon bar, and how wide it is when
/// expanded.
///
/// Global rather than local to the graph editor because four distant places drive the same piece of
/// UI: the icon bar (click), the resize drag (which lives on the outermost container so it survives
/// the pointer leaving the sidebar), and the workspace processor, which has to bring the node
/// properties back into view when an undo/redo jumps to a node - otherwise that change would be
/// applied silently while another view is showing.
static SIDEBAR_VIEW: GlobalSignal<SidebarView> = Signal::global(|| SidebarView::NodeProperties);
static SIDEBAR_COLLAPSED: GlobalSignal<bool> = Signal::global(|| false);
static SIDEBAR_WIDTH: GlobalSignal<f64> = Signal::global(|| 280.0);
/// Set from the backend's authoritative `JumpTarget` when an undo/redo focuses a node: the node it
/// selected and the panel to open once that node's editor has loaded. Consumed (cleared) by whichever
/// `OpticalNodeEditor`/`PortConfigEditor` instance matches the uuid. `apply_document_changes` sets it
/// whether or not it had to switch nodes, so an undo of a detail on the already-selected node still opens
/// its panel.
static PENDING_PANEL_OPEN: GlobalSignal<Option<(Uuid, NodeEditorPanel)>> = Signal::global(|| None);
/// Set from the backend's authoritative `JumpTarget` when an undo/redo touches an analyzer's source
/// mapping: `(analyzer_id, source_port_uuid)`. The analyzer editor has no `NodeEditorPanel`, so this
/// addresses the specific per-source card directly. Consumed (cleared) by whichever source-port card
/// matches both ids, which expands and scrolls itself into view. Set whether or not the analyzer had to be
/// selected, so an undo while the analyzer is already shown still opens the changed card.
static PENDING_SOURCE_CARD_OPEN: GlobalSignal<Option<(Uuid, Uuid)>> = Signal::global(|| None);
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
