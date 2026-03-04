use crate::components::scenery_editor::graph_editor::graph_workspace::{
    GraphsWorkspaceState,
    workspace_handlers::{
        edge_handlers::EdgeHandlers, node_handlers::NodeHandlers, view_handlers::ViewHandlers,
        workspace_handlers::WorkspaceHandlers,
    },
};
use dioxus::prelude::*;

#[derive(Clone, PartialEq, Copy)]
pub struct WorkSpaceSignalHandlers {
    pub workspace: WorkspaceHandlers,
    pub nodes: NodeHandlers,
    pub edges: EdgeHandlers,
    pub view: ViewHandlers,
}

impl WorkSpaceSignalHandlers {
    pub fn new(workspace: Signal<GraphsWorkspaceState>) -> Self {
        Self {
            workspace: WorkspaceHandlers::new(workspace),
            nodes: NodeHandlers::new(workspace),
            edges: EdgeHandlers::new(workspace),
            view: ViewHandlers::new(workspace),
        }
    }
}
