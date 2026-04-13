use dioxus::prelude::*;
use opossum_core::types::api_types::ConnectInfo;
use uuid::Uuid;

use crate::components::scenery_editor::{
    EditorState, GraphState, GraphStore, graph_workspace::{GraphStoreStoreExt, GraphsWorkspaceState}
};

pub(super) fn with_graph_store<F>(
    mut workspace: Signal<GraphsWorkspaceState>,
    graph_id: Uuid,
    mark_dirty: bool,
    f: F,
) where
    F: FnOnce(&mut GraphStore),
{
    let mut ws = workspace.write();

    if let Some(mut graph_store) = ws.get_graph_store(graph_id) {
        f(&mut graph_store.write());
    }

    if mark_dirty {
        ws.needs_saving.set(true);
    }
}

pub(super) fn with_tab<F>(
    mut workspace: Signal<GraphsWorkspaceState>,
    tab_id: Uuid,
    mark_dirty: bool,
    f: F,
) where
    F: FnOnce(&mut Store<GraphState>),
{
    let mut ws = workspace.write();

    if let Some(mut tab) = ws.get_tab(tab_id) {
        f(&mut tab);
    }

    if mark_dirty {
        ws.needs_saving.set(true);
    }
}
pub(super) fn for_each_tab<F>(
    mut workspace: Signal<GraphsWorkspaceState>,
    mark_dirty: bool,
    mut f: F,
) where
    F: FnMut(&mut Store<GraphState>),
{
    let mut ws = workspace.write();

    ws.tabs
        .write()
        .iter_mut()
        .for_each(|(_, mut tab)| f(&mut tab));

    if mark_dirty {
        ws.needs_saving.set(true);
    }
}

pub(super) fn with_editor_state<F>(
    mut workspace: Signal<GraphsWorkspaceState>,
    graph_id: Uuid,
    mark_dirty: bool,
    f: F,
) where
    F: FnOnce(&mut EditorState),
{
    let mut ws = workspace.write();

    if let Some(mut editor_state) = ws.get_editor_state(graph_id) {
        f(&mut editor_state.write());
    }

    if mark_dirty {
        ws.needs_saving.set(true);
    }
}

pub(super) fn with_edges<F>(
    mut workspace: Signal<GraphsWorkspaceState>,
    graph_id: Uuid,
    mark_dirty: bool,
    f: F,
) where
    F: FnOnce(&mut Vec<ConnectInfo>),
{
    let mut ws = workspace.write();

    if let Some(mut edges) = ws.get_graph_edges_mut(graph_id) {
        f(&mut edges.write());
    }

    if mark_dirty {
        ws.needs_saving.set(true);
    }
}
