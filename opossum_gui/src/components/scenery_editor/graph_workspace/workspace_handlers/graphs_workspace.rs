use crate::components::scenery_editor::graph_workspace::{
    GraphsWorkspaceState,
    workspace_handlers::{
        edges::EdgeHandlers, node_handlers::NodeHandlers, view::ViewHandlers,
        workspace::WorkspaceHandlers,
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
    pub fn new(workspace: Store<GraphsWorkspaceState>) -> Self {
        Self {
            workspace: WorkspaceHandlers::new(workspace),
            nodes: NodeHandlers::new(workspace),
            edges: EdgeHandlers::new(workspace),
            view: ViewHandlers::new(workspace),
        }
    }
}
