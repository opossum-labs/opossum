use std::collections::HashMap;

use dioxus::{prelude::*, stores::hashmap::GetWrite};
use opossum_core::types::api_types::ConnectInfo;
use uuid::Uuid;

use crate::components::scenery_editor::{
    EditorState, GraphState, GraphStore, GraphsWorkspaceStateStoreExt, graph_workspace::{GraphStateStoreExt, GraphStoreStoreExt, GraphsWorkspaceState}
};

pub(super) fn with_graph_store<F>(
    workspace: Store<GraphsWorkspaceState>,
    graph_id: Uuid,
    mark_dirty: bool,
    f: F,
) where
    F: FnOnce(&mut Store<GraphStore, MappedMutSignal<GraphStore,GetWrite<Uuid, MappedMutSignal<HashMap<Uuid, GraphState>, WriteSignal<GraphsWorkspaceState>>>>>),

{   
    if let Some(mut graph_store) = workspace.tabs().get(graph_id).map(|g|g.graph_store()) {
        f(&mut graph_store);
    }

    if mark_dirty {
        workspace.needs_saving().set(true);
    }
}

pub(super) fn with_tab<F>(
    workspace: Store<GraphsWorkspaceState>,
    tab_id: Uuid,
    mark_dirty: bool,
    f: F,
) where
    F: FnOnce(&mut Store<GraphState, GetWrite<Uuid, MappedMutSignal<HashMap<Uuid, GraphState>, WriteSignal<GraphsWorkspaceState>>>>),
{
    if let Some(mut tab) = workspace.tabs().get(tab_id) {
        f(&mut tab);
    }

    if mark_dirty {
        workspace.needs_saving().set(true);
    }
}
pub(super) fn for_each_tab<F>(
    workspace: Store<GraphsWorkspaceState>,
    mark_dirty: bool,
    mut f: F,
) where
    F: FnMut(&mut Store<GraphState, GetWrite<Uuid, MappedMutSignal<HashMap<Uuid, GraphState>, WriteSignal<GraphsWorkspaceState>>>>),
{
    workspace.tabs()
        .values()
        .for_each(|mut tab| f(&mut tab));

    if mark_dirty {
        workspace.needs_saving().set(true);
    }
}

pub(super) fn with_editor_state<F>(
    workspace: Store<GraphsWorkspaceState>,
    graph_id: Uuid,
    mark_dirty: bool,
    f: F,
) where
    F: FnOnce(&mut Store<EditorState, MappedMutSignal<EditorState,GetWrite<Uuid, MappedMutSignal<HashMap<Uuid, GraphState>, WriteSignal<GraphsWorkspaceState>>>>>),
{
    if let Some(mut editor_state) = workspace.tabs().get(graph_id).map(|g|g.editor_state()) {
        f(&mut editor_state);
    }

    if mark_dirty {
        workspace.needs_saving().set(true);
    }
}

pub(super) fn with_edges<F>(
    workspace: Store<GraphsWorkspaceState>,
    graph_id: Uuid,
    mark_dirty: bool,
    f: F,
) where
    F: FnOnce(&mut Store<Vec<ConnectInfo>, MappedMutSignal<Vec<ConnectInfo>, MappedMutSignal<GraphStore,GetWrite<Uuid, MappedMutSignal<HashMap<Uuid, GraphState>, WriteSignal<GraphsWorkspaceState>>>>>>),
{
    if let Some(mut edges) = workspace.tabs().get(graph_id).map(|g|g.graph_store().edges()) {
        f(&mut edges);
    }

    if mark_dirty {
        workspace.needs_saving().set(true);
    }
}
