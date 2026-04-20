use crate::{
    OPOSSUM_UI_LOGS,
    components::scenery_editor::{
        DragStatus, GraphState,
        edges::edges_component::EdgeCreation,
        graph_workspace::{
            GraphsWorkspaceState,
            workspace_handlers::helper_functions::{with_editor_state, with_graph_store},
            workspace_state::GraphInfo,
        },
    },
};
use dioxus::{
    html::geometry::euclid::default::{Point2D, Rect, Size2D},
    prelude::*,
};
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
    set_editor_area: EventHandler<Rect<f64>>,
    clear_selected_nodes: EventHandler<Uuid>,
    #[allow(clippy::type_complexity)]
    apply_drag: EventHandler<(Uuid, DragStatus, Point2D<f64>, f64, Point2D<f64>)>,
    set_nodes_cut: EventHandler<bool>,
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
            set_editor_area: set_editor_area_handler(workspace),
            clear_selected_nodes: clear_selected_nodes_handler(workspace),
            apply_drag: apply_drag_handler(workspace),
            set_nodes_cut: set_nodes_cut_handler(workspace),
        }
    }
    pub fn set_nodes_cut(&self, nodes_cut: bool) {
        self.set_nodes_cut.call(nodes_cut);
    }
    pub fn apply_drag(
        &self,
        graph_id: Uuid,
        drag_status: DragStatus,
        relative_shift: Point2D<f64>,
        current_zoom: f64,
        mouse_to_graph_shift: Point2D<f64>,
    ) {
        self.apply_drag.call((
            graph_id,
            drag_status,
            relative_shift,
            current_zoom,
            mouse_to_graph_shift,
        ));
    }
    pub fn clear_selected_nodes(&self, graph_id: Uuid) {
        self.clear_selected_nodes.call(graph_id);
    }
    pub fn set_editor_area(&self, editor_area: Rect<f64>) {
        self.set_editor_area.call(editor_area);
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

fn set_nodes_cut_handler(mut workspace: Signal<GraphsWorkspaceState>) -> EventHandler<bool> {
    EventHandler::new(move |nodes_cut: bool| {
        workspace.write().nodes_cut = nodes_cut;
    })
}

#[allow(clippy::type_complexity)]
fn apply_drag_handler(
    mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, DragStatus, Point2D<f64>, f64, Point2D<f64>)> {
    EventHandler::new(
        move |(graph_id, drag_status, relative_shift, current_zoom, mouse_to_graph_shift): (
            Uuid,
            DragStatus,
            Point2D<f64>,
            f64,
            Point2D<f64>,
        )| {
            let node_edge_shift = Point2D::new(
                relative_shift.x / current_zoom,
                relative_shift.y / current_zoom,
            );

            match drag_status {
                DragStatus::Graph => {
                    with_editor_state(workspace, graph_id, false, |e| {
                        e.apply_shift(relative_shift);
                    });
                }
                DragStatus::Nodes => {
                    with_graph_store(workspace, graph_id, false, |g| {
                        let selected_nodes = g.selected_nodes();
                        for (id, _) in selected_nodes {
                            g.shift_node_position(id, node_edge_shift);
                        }
                    });
                }
                DragStatus::Edge(edge_creation_start) => {
                    with_editor_state(workspace, graph_id, false, |e| {
                        e.edge_in_creation.with_mut(|edge_option| {
                            let edge = edge_option.get_or_insert_with(|| {
                                EdgeCreation::new(
                                    edge_creation_start.src_node,
                                    edge_creation_start.src_port.clone(),
                                    edge_creation_start.src_port_type,
                                    edge_creation_start.start_pos,
                                )
                            });
                            edge.shift_end(node_edge_shift);
                        });
                    });
                }
                DragStatus::SelectionBox(rect) => {
                    let editor_origin = workspace.read().editor_area.read().origin;

                    let graph_pos = Point2D::new(
                        (mouse_to_graph_shift.x - editor_origin.x) / current_zoom,
                        (mouse_to_graph_shift.y - editor_origin.y) / current_zoom,
                    );

                    let width = graph_pos.x - rect.origin.x;
                    let height = graph_pos.y - rect.origin.y;

                    let new_rect_orig_x = if width < 0. {
                        graph_pos.x
                    } else {
                        rect.origin.x
                    };
                    let new_rect_orig_y = if height < 0. {
                        graph_pos.y
                    } else {
                        rect.origin.y
                    };

                    let new_rect = Rect::new(
                        Point2D::new(new_rect_orig_x, new_rect_orig_y),
                        Size2D::new(width.abs(), height.abs()),
                    );

                    workspace.write().selection_box.set(Some(new_rect));
                }
                DragStatus::None | DragStatus::NodeInit => {}
            }
        },
    )
}

fn clear_selected_nodes_handler(workspace: Signal<GraphsWorkspaceState>) -> EventHandler<Uuid> {
    EventHandler::new(move |graph_id| {
        with_graph_store(workspace, graph_id, false, |g| {
            g.clear_selected_nodes();
        });
    })
}

fn set_editor_area_handler(mut workspace: Signal<GraphsWorkspaceState>) -> EventHandler<Rect<f64>> {
    EventHandler::new(move |editor_area| {
        workspace.write().editor_area.set(editor_area);
    })
}
fn set_selection_box_handler(
    mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<Option<Rect<f64>>> {
    EventHandler::new(move |selection_box| {
        workspace.write().selection_box.set(selection_box);
    })
}

fn set_drop_in_group_handler(
    mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<Option<(Uuid, usize)>> {
    EventHandler::new(move |drop_in_group| {
        workspace.write().drop_in_group.set(drop_in_group);
    })
}
fn set_drag_status_handler(
    mut workspace: Signal<GraphsWorkspaceState>,
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
