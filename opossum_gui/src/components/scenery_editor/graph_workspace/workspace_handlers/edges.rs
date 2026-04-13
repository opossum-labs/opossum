use crate::components::scenery_editor::{
    edges::edges_component::EdgeCreation,
    graph_workspace::{
        EditorStateStoreExt, GraphsWorkspaceState,
        workspace_handlers::helper_functions::{with_edges, with_editor_state},
    },
};
use dioxus::prelude::*;
use opossum_core::types::api_types::ConnectInfo;
use uuid::Uuid;

#[derive(Clone, PartialEq, Copy)]
pub struct EdgeHandlers {
    add_edge: EventHandler<(ConnectInfo, Uuid)>,
    delete_edge: EventHandler<(ConnectInfo, Uuid)>,
    update_edge: EventHandler<(ConnectInfo, Uuid)>,
    update_edges: EventHandler<(Vec<ConnectInfo>, Uuid)>,
    add_group_edges: EventHandler<(Uuid, Vec<ConnectInfo>)>,
    set_edge_in_creation: EventHandler<(Option<EdgeCreation>, Uuid)>,
}

impl EdgeHandlers {
    pub fn new(workspace: Signal<GraphsWorkspaceState>) -> Self {
        Self {
            add_edge: add_edge_handler(workspace),
            delete_edge: delete_edge_handler(workspace),
            update_edge: update_edge_handler(workspace),
            update_edges: update_edges_handler(workspace),
            add_group_edges: add_group_edges_handler(workspace),
            set_edge_in_creation: set_edge_in_creation_handler(workspace),
        }
    }

    pub fn set_edge_in_creation(&self, edge_creation: Option<EdgeCreation>, graph_id: Uuid) {
        self.set_edge_in_creation.call((edge_creation, graph_id));
    }

    pub fn add_edge(&self, edge: ConnectInfo, graph_id: Uuid) {
        self.add_edge.call((edge, graph_id));
    }

    pub fn delete_edge(&self, edge: ConnectInfo, graph_id: Uuid) {
        self.delete_edge.call((edge, graph_id));
    }

    pub fn update_edge(&self, edge: ConnectInfo, graph_id: Uuid) {
        self.update_edge.call((edge, graph_id));
    }

    pub fn update_edges(&self, edges: Vec<ConnectInfo>, graph_id: Uuid) {
        self.update_edges.call((edges, graph_id));
    }

    pub fn add_group_edges(&self, group_id: Uuid, edges: Vec<ConnectInfo>) {
        self.add_group_edges.call((group_id, edges));
    }
}

fn set_edge_in_creation_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(Option<EdgeCreation>, Uuid)> {
    EventHandler::new(move |(edge_in_creation, graph_id)| {
        with_editor_state(workspace, graph_id, false, |e| {
            e.edge_in_creation().set(edge_in_creation);
        });
    })
}

fn add_edge_handler(workspace: Signal<GraphsWorkspaceState>) -> EventHandler<(ConnectInfo, Uuid)> {
    EventHandler::new(move |(edge, graph_id)| {
        with_edges(workspace, graph_id, true, |edges| {
            edges.push(edge);
        });
    })
}

fn delete_edge_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(ConnectInfo, Uuid)> {
    EventHandler::new(move |(edge, graph_id)| {
        with_edges(workspace, graph_id, true, |edges| {
            edges.retain(|e| e != &edge);
        });
    })
}

fn update_edge_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(ConnectInfo, Uuid)> {
    EventHandler::new(move |(ci, graph_id): (ConnectInfo, Uuid)| {
        with_edges(workspace, graph_id, true, |edges| {
            if let Some(e) = edges
                .iter_mut()
                .find(|e| e.src_uuid() == ci.src_uuid() && e.target_uuid() == ci.target_uuid())
            {
                *e = ci;
            }
        });
    })
}

fn update_edges_handler(
    mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(Vec<ConnectInfo>, Uuid)> {
    EventHandler::new(move |(connections, graph_id)| {
        let mut ws = workspace.write();

        if let Some(mut edges) = ws.get_graph_edges_mut(graph_id) {
            edges.set(connections);
        }

        ws.needs_saving.set(true);
    })
}

fn add_group_edges_handler(
    mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, Vec<ConnectInfo>)> {
    EventHandler::new(move |(group_id, edges)| {
        if let Some(mut graph_edges) = workspace.write().get_graph_edges_mut(group_id) {
            graph_edges.set(edges);
        }
    })
}
