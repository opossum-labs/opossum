use crate::components::scenery_editor::{
    GraphState, graph_editor::graph_workspace::GraphsWorkspaceState,
};
use dioxus::prelude::*;
use uuid::Uuid;

#[derive(Clone, PartialEq, Copy)]
pub struct WorkspaceHandlers {
    add_new_group_tab: EventHandler<(String, Uuid)>,
    set_root_scenery_id: EventHandler<Uuid>,
    remove_tabs: EventHandler<Vec<Uuid>>,
    set_needs_saving: EventHandler<bool>,
    clear_workspace: EventHandler<()>,
    set_active_tab: EventHandler<Option<Uuid>>,
}

impl WorkspaceHandlers {
    pub fn new(workspace: Signal<GraphsWorkspaceState>) -> Self {
        Self {
            add_new_group_tab: add_new_group_tab_handler(workspace),
            set_root_scenery_id: set_root_scenery_id_handler(workspace),
            remove_tabs: remove_tabs_handler(workspace),
            set_needs_saving: set_needs_saving_handler(workspace),
            clear_workspace: clear_workspace_handler(workspace),
            set_active_tab: set_active_tab_handler(workspace),
        }
    }
    pub fn add_new_group_tab(&self, name: String, id: Uuid) {
        self.add_new_group_tab.call((name, id));
    }

    pub fn set_root_scenery_id(&self, id: Uuid) {
        self.set_root_scenery_id.call(id);
    }

    pub fn remove_tabs(&self, ids: Vec<Uuid>) {
        self.remove_tabs.call(ids);
    }

    pub fn set_needs_saving(&self, value: bool) {
        self.set_needs_saving.call(value);
    }

    pub fn clear_workspace(&self) {
        self.clear_workspace.call(());
    }

    pub fn set_active_tab(&self, id: Option<Uuid>) {
        self.set_active_tab.call(id);
    }
}

fn add_new_group_tab_handler(
    mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(String, Uuid)> {
    EventHandler::new(move |(name, id)| {
        let mut ws = workspace.write();

        let graph_state = GraphState {
            name,
            id,
            ..Default::default()
        };

        ws.tabs.write().insert(id, Signal::new(graph_state));

        ws.tab_order.write().push(id);
        ws.active_tab.set(Some(id));
    })
}

fn set_root_scenery_id_handler(mut workspace: Signal<GraphsWorkspaceState>) -> EventHandler<Uuid> {
    EventHandler::new(move |id| {
        workspace.write().root_scenery_id.set(id);
    })
}

fn remove_tabs_handler(mut workspace: Signal<GraphsWorkspaceState>) -> EventHandler<Vec<Uuid>> {
    EventHandler::new(move |ids| {
        workspace.write().remove_tabs(ids);
    })
}

fn set_needs_saving_handler(mut workspace: Signal<GraphsWorkspaceState>) -> EventHandler<bool> {
    EventHandler::new(move |value| {
        workspace.write().needs_saving.set(value);
    })
}

fn clear_workspace_handler(mut workspace: Signal<GraphsWorkspaceState>) -> EventHandler<()> {
    EventHandler::new(move |_| {
        workspace.set(GraphsWorkspaceState::default());
    })
}

fn set_active_tab_handler(
    mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<Option<Uuid>> {
    EventHandler::new(move |id| {
        workspace.write().active_tab.set(id);
    })
}
