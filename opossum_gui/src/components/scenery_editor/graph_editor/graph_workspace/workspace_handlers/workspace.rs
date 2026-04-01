use crate::{
    OPOSSUM_UI_LOGS,
    components::scenery_editor::{
        GraphState,
        graph_editor::{DragStatus, graph_workspace::{GraphsWorkspaceState, workspace_state::GraphInfo}},
    },
};
use dioxus::{html::geometry::euclid::default::Rect, prelude::*};
use uuid::Uuid;

#[derive(Clone, PartialEq, Copy)]
pub struct WorkspaceHandlers {
    add_new_group_tab: EventHandler<GraphInfo>,
    set_root_scenery_id: EventHandler<Uuid>,
    remove_tabs: EventHandler<Vec<Uuid>>,
    set_needs_saving: EventHandler<bool>,
    clear_workspace: EventHandler<()>,
    set_active_tab: EventHandler<Uuid>,
    add_port_map: EventHandler<((Uuid, Uuid), (String, String))>,
    remove_port_map: EventHandler<(Uuid, String)>,
    set_drag_status: EventHandler<DragStatus>,
    set_drop_in_group: EventHandler<Option<(Uuid, usize)>>,
    set_selection_box: EventHandler<Option<Rect<f64>>>,
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
            add_port_map: add_port_map_handler(workspace),
            remove_port_map: remove_port_map_handler(workspace),
            set_drag_status: set_drag_status_handler(workspace),
            set_drop_in_group: set_drop_in_group_handler(workspace),
            set_selection_box: set_selection_box_handler(workspace),
        }
    }
    pub fn set_selection_box(&self, selection_box: Option<Rect<f64>>) {
        self.set_selection_box.call(selection_box);
    }
    pub fn set_drop_in_group(&self, drop_in_group: Option<(Uuid, usize)>) {
        self.set_drop_in_group.call(drop_in_group);
    }
    pub fn set_drag_status(&self, drag_status: DragStatus) {
        self.set_drag_status.call(drag_status);
    }
    pub fn add_new_group_tab(&self, graph_info: GraphInfo) {
        self.add_new_group_tab.call(graph_info);
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

    pub fn set_active_tab(&self, id: Uuid) {
        self.set_active_tab.call(id);
    }
    pub fn add_port_map(
        &self,
        group_id: Uuid,
        group_port_name: String,
        mapped_node_port_name: String,
        mapped_node_id: Uuid,
    ) {
        self.add_port_map.call((
            (group_id, mapped_node_id),
            (group_port_name, mapped_node_port_name),
        ));
    }
    pub fn remove_port_map(&self, group_id: Uuid, group_port_name: String) {
        self.remove_port_map.call((group_id, group_port_name));
    }
}

fn set_selection_box_handler(mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<Option<Rect<f64>>> {
    EventHandler::new(move |selection_box| {
        workspace.write().selection_box.set(selection_box);
    })
}

fn set_drop_in_group_handler(mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<Option<(Uuid, usize)>> {
    EventHandler::new(move |drop_in_group| {
        workspace.write().drop_in_group.set(drop_in_group);
    })
}
fn set_drag_status_handler(mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<DragStatus> {
    EventHandler::new(move |drag_status| {
        workspace.write().drag_status.set(drag_status);
    })
}

fn remove_port_map_handler(
    mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, String)> {
    EventHandler::new(move |(group_id, group_port_name): (Uuid, String)| {
        let ws = workspace.write();

        if let Some(mut graph_store) = ws.get_graph_store(group_id)
            && !graph_store
                .write()
                .mapped_ports
                .write()
                .remove_key(&group_port_name)
        {
            OPOSSUM_UI_LOGS.write().add_log(&format!(
                "Could not remove port mapping of port: {group_port_name}"
            ));
        }
    })
}

fn add_port_map_handler(
    mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<((Uuid, Uuid), (String, String))> {
    EventHandler::new(
        move |((group_id, mapped_node_id), (group_port_name, mapped_node_port_name)): (
            (Uuid, Uuid),
            (String, String),
        )| {
            let ws = workspace.write();

            if let Some(mut graph_store) = ws.get_graph_store(group_id)
                && let Err(e) = graph_store.write().mapped_ports.write().add(
                    &group_port_name,
                    mapped_node_id,
                    &mapped_node_port_name,
                )
            {
                OPOSSUM_UI_LOGS.write().add_log(&e.to_string());
            }
        },
    )
}

fn add_new_group_tab_handler(
    mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<GraphInfo> {
    EventHandler::new(move |graph_info: GraphInfo| {
        let mut ws = workspace.write();

        let id = graph_info.id;
        let graph_state = GraphState {
            graph_info,
            ..Default::default()
        };

        ws.tabs.write().insert(id, Signal::new(graph_state));

        ws.tab_order.write().push(id);
        ws.active_tab.set(id);
    })
}

fn set_root_scenery_id_handler(mut workspace: Signal<GraphsWorkspaceState>) -> EventHandler<Uuid> {
    EventHandler::new(move |id| {
        workspace.write().root_scenery_id.set(id);
    })
}

fn remove_tabs_handler(mut workspace: Signal<GraphsWorkspaceState>) -> EventHandler<Vec<Uuid>> {
    EventHandler::new(move |ids| {
        workspace.write().remove_tabs(&ids);
    })
}

fn set_needs_saving_handler(mut workspace: Signal<GraphsWorkspaceState>) -> EventHandler<bool> {
    EventHandler::new(move |value| {
        workspace.write().needs_saving.set(value);
    })
}

fn clear_workspace_handler(mut workspace: Signal<GraphsWorkspaceState>) -> EventHandler<()> {
    EventHandler::new(move |()| {
        workspace.set(GraphsWorkspaceState::default());
    })
}

fn set_active_tab_handler(mut workspace: Signal<GraphsWorkspaceState>) -> EventHandler<Uuid> {
    EventHandler::new(move |id| {
        workspace.write().active_tab.set(id);
    })
}
