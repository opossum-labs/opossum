use crate::components::scenery_editor::{
    edges::{edge_element::EdgeElement, edges_component::EdgeCreation},
    graph_workspace::{
        EditorStateStoreExt, GraphStoreStoreImplExt, GraphsWorkspaceState,
        workspace_handlers::helper_functions::{with_edges, with_editor_state, with_graph_store},
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
    pub fn new(workspace: Store<GraphsWorkspaceState>) -> Self {
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

    pub fn add_group_edges(&self, group_id: Uuid, edges: Vec<ConnectInfo>) {
        self.add_group_edges.call((group_id, edges));
    }
}

fn set_edge_in_creation_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(Option<EdgeCreation>, Uuid)> {
    EventHandler::new(move |(edge_in_creation, graph_id)| {
        with_editor_state(workspace, graph_id, false, |e| {
            e.edge_in_creation().set(edge_in_creation);
        });
    })
}

fn add_edge_handler(workspace: Store<GraphsWorkspaceState>) -> EventHandler<(ConnectInfo, Uuid)> {
    EventHandler::new(move |(edge, graph_id)| {
        with_graph_store(workspace, graph_id, true, |graph_store| {
            // Call generated store extension method directly on the store handle
            graph_store.add_edge(edge);
        });
    })
}

fn delete_edge_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(ConnectInfo, Uuid)> {
    EventHandler::new(move |(edge, graph_id)| {
        with_graph_store(workspace, graph_id, true, |graph_store| {
            // Call generated store extension method directly on the store handle
            graph_store.remove_edge(&edge);
        });
    })
}

fn update_edge_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(ConnectInfo, Uuid)> {
    EventHandler::new(move |(ci, graph_id): (ConnectInfo, Uuid)| {
        with_edges(workspace, graph_id, true, |edges| {
            if let Some(mut e) = edges.iter().find(|e| {
                e.read().src_uuid() == ci.src_uuid() && e.read().target_uuid() == ci.target_uuid()
            }) {
                let current_index = e.read().edge_index();
                e.set(EdgeElement::new(ci, current_index));
            }
        });
    })
}

fn update_edges_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(Vec<ConnectInfo>, Uuid)> {
    EventHandler::new(move |(connections, graph_id)| {
        with_graph_store(workspace, graph_id, true, |graph_store| {
            for ci in connections {
                graph_store.add_edge(ci);
            }
        });
    })
}

fn add_group_edges_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, Vec<ConnectInfo>)> {
    EventHandler::new(move |(group_id, edges)| {
        with_graph_store(workspace, group_id, true, |graph_store| {
            for ci in edges {
                graph_store.add_edge(ci);
            }
        });
    })
}
