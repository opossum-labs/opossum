use crate::{
    OPOSSUM_UI_LOGS,
    components::scenery_editor::{
        DragStatus, EditorState, GraphState, GraphStore,
        edges::edges_component::EdgeCreation,
        graph_workspace::{
            EditorStateStoreExt, GraphStateStoreExt, GraphStoreStoreExt, GraphStoreStoreImplExt,
            GraphsWorkspaceState, GraphsWorkspaceStateStoreExt, GraphsWorkspaceStateStoreImplExt,
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
    ensure_group_tab: EventHandler<GraphInfo>,
    set_root_scenery_id: EventHandler<Uuid>,
    remove_tabs: EventHandler<Vec<Uuid>>,
    set_needs_saving: EventHandler<bool>,
    clear_workspace: EventHandler<()>,
    set_active_tab: EventHandler<Uuid>,
    add_port_map: EventHandler<((Uuid, Uuid), (String, String))>,
    remove_port_maps_for_node: EventHandler<(Uuid, Uuid)>,
    set_drag_status: EventHandler<DragStatus>,
    set_drop_in_group: EventHandler<Option<(Uuid, usize)>>,
    set_selection_box: EventHandler<Option<Rect<f64>>>,
    set_editor_area: EventHandler<Rect<f64>>,
    clear_selected_nodes: EventHandler<Uuid>,
    #[allow(clippy::type_complexity)]
    apply_drag: EventHandler<(Uuid, Point2D<f64>, f64, Point2D<f64>)>,
    set_nodes_cut: EventHandler<bool>,
}

impl WorkspaceHandlers {
    pub fn new(workspace: Store<GraphsWorkspaceState>) -> Self {
        Self {
            add_new_group_tab: add_new_group_tab_handler(workspace),
            ensure_group_tab: ensure_group_tab_handler(workspace),
            set_root_scenery_id: set_root_scenery_id_handler(workspace),
            remove_tabs: remove_tabs_handler(workspace),
            set_needs_saving: set_needs_saving_handler(workspace),
            clear_workspace: clear_workspace_handler(workspace),
            set_active_tab: set_active_tab_handler(workspace),
            add_port_map: add_port_map_handler(workspace),
            remove_port_maps_for_node: remove_port_maps_for_node_handler(workspace),
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
        relative_shift: Point2D<f64>,
        current_zoom: f64,
        mouse_to_graph_shift: Point2D<f64>,
    ) {
        self.apply_drag
            .call((graph_id, relative_shift, current_zoom, mouse_to_graph_shift));
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
    /// Silently seed a group's tab data if it doesn't already exist, without opening it in the tab
    /// bar (no `tab_order`/`active_tab` change). Lets background writes (port maps, nodes, edges)
    /// for a group the user has never navigated into actually land, instead of silently no-op'ing
    /// against a tab that was never created.
    pub fn ensure_group_tab(&self, graph_info: GraphInfo) {
        self.ensure_group_tab.call(graph_info);
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
    pub fn remove_port_maps_for_node(&self, group_id: Uuid, node_id: Uuid) {
        self.remove_port_maps_for_node.call((group_id, node_id));
    }
}

fn set_nodes_cut_handler(workspace: Store<GraphsWorkspaceState>) -> EventHandler<bool> {
    EventHandler::new(move |nodes_cut: bool| {
        workspace.nodes_cut().set(nodes_cut);
    })
}

#[allow(clippy::type_complexity)]
fn apply_drag_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, Point2D<f64>, f64, Point2D<f64>)> {
    let drag_status = workspace.drag_status();
    EventHandler::new(
        move |(graph_id, relative_shift, current_zoom, mouse_to_graph_shift): (
            Uuid,
            Point2D<f64>,
            f64,
            Point2D<f64>,
        )| {
            let drag_status = drag_status.read().clone();
            let node_edge_shift = Point2D::new(
                relative_shift.x / current_zoom,
                relative_shift.y / current_zoom,
            );

            match drag_status {
                DragStatus::Graph => {
                    with_editor_state(workspace, graph_id, false, |e| {
                        e.write().apply_shift(relative_shift);
                    });
                }
                DragStatus::Nodes => {
                    with_graph_store(workspace, graph_id, false, |g| {
                        let selected_nodes = g.read().selected_nodes();
                        for (id, _) in selected_nodes {
                            g.shift_node_position(id, node_edge_shift);
                        }
                    });
                }
                DragStatus::Edge(edge_creation_start) => {
                    if let Some(e) = workspace.tabs().get(graph_id).map(|g| g.editor_state()) {
                        let mut edge_in_creation = e.edge_in_creation();
                        let mut e_write = edge_in_creation.write();
                        let edge = e_write.get_or_insert_with(|| {
                            EdgeCreation::new(
                                edge_creation_start.src_node,
                                edge_creation_start.src_port.clone(),
                                edge_creation_start.src_port_type,
                                edge_creation_start.start_pos,
                            )
                        });
                        edge.shift_end(node_edge_shift);
                    }
                }

                DragStatus::ArmedSelection(start) => {
                    let editor_origin = workspace.editor_area().read().origin;

                    let graph_pos = Point2D::new(
                        (mouse_to_graph_shift.x - editor_origin.x) / current_zoom,
                        (mouse_to_graph_shift.y - editor_origin.y) / current_zoom,
                    );

                    let dx = graph_pos.x - start.x;
                    let dy = graph_pos.y - start.y;

                    let dist_sq = dx.mul_add(dx, dy * dy);

                    if dist_sq < 25.0 {
                        return;
                    }
                    let rect = Rect::new(start, Size2D::new(0.0, 0.0));

                    workspace.drag_status().set(DragStatus::SelectionBox(rect));
                }
                DragStatus::SelectionBox(rect) => {
                    let editor_origin = workspace.editor_area().read().origin;

                    let graph_pos = Point2D::new(
                        (mouse_to_graph_shift.x - editor_origin.x) / current_zoom,
                        (mouse_to_graph_shift.y - editor_origin.y) / current_zoom,
                    );

                    let min_x = rect.origin.x.min(graph_pos.x);
                    let min_y = rect.origin.y.min(graph_pos.y);

                    let max_x = rect.origin.x.max(graph_pos.x);
                    let max_y = rect.origin.y.max(graph_pos.y);

                    let new_rect = Rect::new(
                        Point2D::new(min_x, min_y),
                        Size2D::new(max_x - min_x, max_y - min_y),
                    );

                    if rect == new_rect {
                        return;
                    }

                    workspace.selection_box().set(Some(new_rect));
                }

                DragStatus::None | DragStatus::NodeInit => {}
            }
        },
    )
}

fn clear_selected_nodes_handler(workspace: Store<GraphsWorkspaceState>) -> EventHandler<Uuid> {
    EventHandler::new(move |graph_id| {
        with_graph_store(workspace, graph_id, false, |g| {
            g.clear_selected_nodes();
        });
    })
}

fn set_editor_area_handler(workspace: Store<GraphsWorkspaceState>) -> EventHandler<Rect<f64>> {
    EventHandler::new(move |editor_area| {
        workspace.editor_area().set(editor_area);
    })
}
fn set_selection_box_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<Option<Rect<f64>>> {
    EventHandler::new(move |selection_box| {
        workspace.selection_box().set(selection_box);
    })
}

fn set_drop_in_group_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<Option<(Uuid, usize)>> {
    EventHandler::new(move |drop_in_group| {
        workspace.drop_in_group().set(drop_in_group);
    })
}
fn set_drag_status_handler(workspace: Store<GraphsWorkspaceState>) -> EventHandler<DragStatus> {
    EventHandler::new(move |drag_status| {
        workspace.drag_status().set(drag_status);
    })
}

fn remove_port_maps_for_node_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, Uuid)> {
    EventHandler::new(move |(group_id, node_id): (Uuid, Uuid)| {
        if let Some(graph_store) = workspace.tabs().get(group_id).map(|g| g.graph_store()) {
            graph_store
                .mapped_ports()
                .write()
                .remove_all_from_uuid(node_id);
        }
    })
}

fn add_port_map_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<((Uuid, Uuid), (String, String))> {
    EventHandler::new(
        move |((group_id, mapped_node_id), (group_port_name, mapped_node_port_name)): (
            (Uuid, Uuid),
            (String, String),
        )| {
            if let Some(graph_store) = workspace.tabs().get(group_id).map(|g| g.graph_store())
                && let Err(e) = graph_store.mapped_ports().write().add(
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

fn add_new_group_tab_handler(workspace: Store<GraphsWorkspaceState>) -> EventHandler<GraphInfo> {
    EventHandler::new(move |graph_info: GraphInfo| {
        let id = graph_info.id;
        // The tab's data may already have been silently seeded by `ensure_group_tab` (e.g. a
        // subgroup a node was dragged into before it was ever opened) - don't blow that away, the
        // caller always follows up with a full refetch anyway, which reconciles either way.
        if !workspace.tabs().read().contains_key(&id) {
            let graph_state =
                GraphState::new(GraphStore::default(), EditorState::default(), graph_info);
            workspace.tabs().write().insert(id, graph_state);
        }
        if !workspace.tab_order().read().contains(&id) {
            workspace.tab_order().write().push(id);
        }
        workspace.active_tab().set(id);
    })
}

/// Silently seed `graph_info.id`'s tab data if it doesn't already exist, without adding it to
/// `tab_order` or making it the active tab - see [`WorkspaceHandlers::ensure_group_tab`].
fn ensure_group_tab_handler(workspace: Store<GraphsWorkspaceState>) -> EventHandler<GraphInfo> {
    EventHandler::new(move |graph_info: GraphInfo| {
        let id = graph_info.id;
        if !workspace.tabs().read().contains_key(&id) {
            let graph_state =
                GraphState::new(GraphStore::default(), EditorState::default(), graph_info);
            workspace.tabs().write().insert(id, graph_state);
        }
    })
}

fn set_root_scenery_id_handler(workspace: Store<GraphsWorkspaceState>) -> EventHandler<Uuid> {
    EventHandler::new(move |id| {
        workspace.root_scenery_id().set(id);
    })
}

fn remove_tabs_handler(mut workspace: Store<GraphsWorkspaceState>) -> EventHandler<Vec<Uuid>> {
    EventHandler::new(move |ids: Vec<Uuid>| {
        workspace.remove_tabs(&ids);
    })
}

fn set_needs_saving_handler(workspace: Store<GraphsWorkspaceState>) -> EventHandler<bool> {
    EventHandler::new(move |value| {
        workspace.needs_saving().set(value);
    })
}

fn clear_workspace_handler(mut workspace: Store<GraphsWorkspaceState>) -> EventHandler<()> {
    EventHandler::new(move |()| {
        workspace.set(GraphsWorkspaceState::default());
    })
}

fn set_active_tab_handler(workspace: Store<GraphsWorkspaceState>) -> EventHandler<Uuid> {
    EventHandler::new(move |id| {
        workspace.active_tab().set(id);
    })
}
