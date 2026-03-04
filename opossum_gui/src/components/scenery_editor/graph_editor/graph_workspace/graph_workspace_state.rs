use std::collections::{BTreeMap, HashMap};

use dioxus::{
    html::geometry::{
        Pixels, PixelsSize,
        euclid::{Rect, Size2D, UnknownUnit, default::Point2D},
    },
    prelude::*,
};
use opossum_core::{
    opm_document::AnalyzerInfo,
    types::api_types::{ConnectInfo, NewAnalyzerInfo, NodeInfo},
};
use uuid::Uuid;

use crate::{
    OPOSSUM_UI_LOGS,
    components::scenery_editor::{
        GraphState, GraphStore,
        constants::{MAX_ZOOM, MIN_ZOOM},
        graph_editor::graph_editor_component::EditorState,
    },
};

#[derive(Clone, Eq, PartialEq, Default)]
pub struct GraphsWorkspaceState {
    pub tabs: Signal<BTreeMap<Uuid, Signal<GraphState>>>,
    pub active_tab: Signal<Option<Uuid>>,
    pub root_scenery_id: Signal<Uuid>,
    pub needs_saving: Signal<bool>,
    pub editor_rect: Signal<Rect<f64, Pixels>>,
}

impl GraphsWorkspaceState {
    fn get_graph_store(&self, graph_id: Uuid) -> Option<Signal<GraphStore>> {
        self.tabs
            .read()
            .get(&graph_id)
            .map(|g| g.read().graph_store)
    }
    fn get_editor_state(&self, graph_id: Uuid) -> Option<Signal<EditorState>> {
        self.tabs
            .read()
            .get(&graph_id)
            .map(|g| g.read().editor_state)
    }
    fn _get_active_editor(&self) -> Option<Signal<EditorState>> {
        if let Some(graph_id) = *self.active_tab.read() {
            self.tabs
                .read()
                .get(&graph_id)
                .map(|g| g.read().editor_state)
        } else {
            None
        }
    }
    fn get_graph_edges(&self, graph_id: Uuid) -> Option<Signal<Vec<ConnectInfo>>> {
        self.tabs
            .read()
            .get(&graph_id)
            .map(|g| g.read().graph_store.read().edges().into())
    }
    fn get_graph_bounding_box(&self, graph_id: Uuid) -> Option<Rect<f64, UnknownUnit>> {
        self.tabs
            .read()
            .get(&graph_id)
            .map(|g| g.read().graph_store.read().get_bounding_box())
    }

    pub fn center_graph(&mut self, graph_id: Uuid) {
        let bounding_box_opt = self.get_graph_bounding_box(graph_id);
        let view_center = self.get_view_port_center();
        if let (Some(mut editor), Some(bounding_box)) =
            (self.get_editor_state(graph_id), bounding_box_opt)
        {
            let center = bounding_box.center();
            let zoom = *editor.read().zoom.read();
            editor.write().shift.set(Point2D::new(
                center.x.mul_add(-zoom, view_center.x),
                center.y.mul_add(-zoom, view_center.y),
            ));
        }
    }

    fn zoom_to_fit(&mut self, graph_id: Uuid) {
        let bounding_box_opt = self.get_graph_bounding_box(graph_id);
        let view_box = self.get_view_port_size();
        let view_center = self.get_view_port_center();

        if let (Some(mut editor), Some(bounding_box)) =
            (self.get_editor_state(graph_id), bounding_box_opt)
        {
            let padding_fac = 0.95;
            let zoom = *editor.read().zoom.read();
            let height_fac = view_box.height * padding_fac / zoom / bounding_box.height();
            let width_fac = view_box.width * padding_fac / zoom / bounding_box.width();
            editor
                .write()
                .zoom
                .set((zoom * width_fac.min(height_fac)).clamp(MIN_ZOOM, MAX_ZOOM));

            let center = bounding_box.center();
            let zoom = *editor.read().zoom.read();
            editor.write().shift.set(Point2D::new(
                center.x.mul_add(-zoom, view_center.x),
                center.y.mul_add(-zoom, view_center.y),
            ));
        }
    }

    pub fn get_view_port_center(&self) -> Point2D<f64> {
        let editor_size = *self.editor_rect.read();
        Point2D::new(editor_size.width() / 2., editor_size.height() / 2.)
    }
    pub fn get_view_port_size(&self) -> Size2D<f64, Pixels> {
        self.editor_rect.read().size
    }
}

#[derive(Clone, PartialEq, Copy)]
pub struct WorkSpaceSignalHandlers {
    pub add_new_group_tab: EventHandler<(String, Uuid)>,
    pub set_root_scenery_id: EventHandler<Uuid>,
    pub remove_tab: EventHandler<Uuid>,
    pub set_needs_saving: EventHandler<bool>,
    pub clear_workspace: EventHandler<()>,
    pub add_root_scenery_nodes: EventHandler<Vec<NodeInfo>>,
    pub add_root_scenery_analyzers: EventHandler<Vec<AnalyzerInfo>>,
    pub add_root_scenery_edges: EventHandler<Vec<ConnectInfo>>,
    pub set_active_tab: EventHandler<Option<Uuid>>,
    pub add_optical_node: EventHandler<(NodeInfo, Uuid)>,
    pub add_reference_node: EventHandler<(NodeInfo, Uuid)>,
    pub add_analyzer_node: EventHandler<(NewAnalyzerInfo, Uuid, Uuid)>,
    pub remove_nodes: EventHandler<(Vec<Uuid>, Uuid)>,
    pub update_node_positions: EventHandler<(HashMap<Uuid, Point2D<f64>>, Uuid)>,
    pub add_edge: EventHandler<(ConnectInfo, Uuid)>,
    pub delete_edge: EventHandler<(ConnectInfo, Uuid)>,
    pub update_edge: EventHandler<(ConnectInfo, Uuid)>,
    pub center_graph: EventHandler<(Uuid, bool)>,
    pub zoom_to_fit: EventHandler<(Uuid, bool)>,
    pub invert_node: EventHandler<(Uuid, bool, Uuid)>,
    pub update_edges: EventHandler<(Vec<ConnectInfo>, Uuid)>,
    pub set_node_name: EventHandler<(String, Uuid, Uuid)>,
}

impl WorkSpaceSignalHandlers {
    pub fn new(mut workspace: Signal<GraphsWorkspaceState>) -> Self {
        let add_new_group_tab = {
            let mut workspace = workspace;
            EventHandler::new(move |(title, id): (String, Uuid)| {
                let mut graph_state = GraphState::default();
                graph_state.id = id;
                graph_state.name = title;

                workspace
                    .write()
                    .tabs
                    .write()
                    .insert(id, Signal::new(graph_state));

                workspace.write().active_tab.set(Some(id));
            })
        };

        let set_node_name = {
            EventHandler::new(move |(name, node_id, graph_id)| {
                if let Some(mut graph_store) = workspace.write().get_graph_store(graph_id) {
                    graph_store.write().set_name_of_node(node_id, name);
                }
            })
        };

        let set_root_scenery_id = {
            EventHandler::new(move |id: Uuid| {
                workspace.write().root_scenery_id.set(id);
            })
        };

        let remove_tab = {
            EventHandler::new(move |id: Uuid| {
                workspace.write().tabs.write().remove(&id);
            })
        };

        let set_needs_saving = {
            EventHandler::new(move |needs_saving: bool| {
                workspace.write().needs_saving.set(needs_saving);
            })
        };

        let clear_workspace = {
            EventHandler::new(move |()| {
                workspace.set(GraphsWorkspaceState::default());
            })
        };

        let update_node_positions = {
            EventHandler::new(
                move |(new_positions, graph_id): (HashMap<Uuid, Point2D<f64>>, Uuid)| {
                    if let Some(mut graph_store) = workspace.write().get_graph_store(graph_id) {
                        graph_store.write().update_node_positions(new_positions);
                    }
                    workspace.write().needs_saving.set(true);
                },
            )
        };

        let add_edge = {
            EventHandler::new(move |(connect_info, graph_id): (ConnectInfo, Uuid)| {
                if let Some(mut edges) = workspace.write().get_graph_edges(graph_id) {
                    edges.write().push(connect_info);
                }
                workspace.write().needs_saving.set(true);
            })
        };

        let delete_edge = {
            EventHandler::new(move |(edge_to_delete, graph_id): (ConnectInfo, Uuid)| {
                if let Some(mut edges) = workspace.write().get_graph_edges(graph_id) {
                    edges.write().retain(|e| e != &edge_to_delete);
                }
                workspace.write().needs_saving.set(true);
            })
        };

        let update_edge = {
            EventHandler::new(move |(ci, graph_id): (ConnectInfo, Uuid)| {
                if let Some(mut edges) = workspace.write().get_graph_edges(graph_id) {
                    if let Some(e) = edges.write().iter_mut().find(|e| {
                        e.src_uuid() == ci.src_uuid() && e.target_uuid() == ci.target_uuid()
                    }) {
                        *e = ci;
                    }
                }
                workspace.write().needs_saving.set(true);
            })
        };

        let update_edges = {
            EventHandler::new(move |(connections, graph_id): (Vec<ConnectInfo>, Uuid)| {
                if let Some(mut edges) = workspace.write().get_graph_edges(graph_id) {
                    edges.set(connections);
                }
                workspace.write().needs_saving.set(true);
            })
        };

        let add_optical_node = {
            EventHandler::new(move |(node_info, graph_id): (NodeInfo, Uuid)| {
                if let Some(mut graph_store) = workspace.write().get_graph_store(graph_id) {
                    graph_store.write().add_new_optical_node(&node_info);
                }
                workspace.write().needs_saving.set(true);
            })
        };

        let add_analyzer_node = {
            EventHandler::new(
                move |(analyzer_info, analyzer_id, graph_id): (NewAnalyzerInfo, Uuid, Uuid)| {
                    if let Some(mut graph_store) = workspace.write().get_graph_store(graph_id) {
                        graph_store
                            .write()
                            .add_new_analyzer(analyzer_info, analyzer_id);
                    }
                    workspace.write().needs_saving.set(true);
                },
            )
        };

        let add_reference_node = {
            EventHandler::new(move |(node_info, graph_id): (NodeInfo, Uuid)| {
                if let Some(mut graph_store) = workspace.write().get_graph_store(graph_id) {
                    graph_store.write().add_new_reference_node(&node_info);
                }
                workspace.write().needs_saving.set(true);
            })
        };

        let remove_nodes = {
            EventHandler::new(move |(node_ids, graph_id): (Vec<Uuid>, Uuid)| {
                if let Some(mut graph_store) = workspace.write().get_graph_store(graph_id) {
                    graph_store.write().remove_nodes_by_id(node_ids);
                }
                workspace.write().needs_saving.set(true);
            })
        };

        let add_root_scenery_nodes = {
            EventHandler::new(move |nodes: Vec<NodeInfo>| {
                let graph_id = *workspace.read().root_scenery_id.read();
                if let Some(mut graph_store) = workspace.write().get_graph_store(graph_id) {
                    graph_store.write().add_nodes(&nodes);
                } else {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log("no root scenery found! Cannot add nodes!");
                }
            })
        };

        let invert_node = {
            EventHandler::new(move |(node_id, inverted, graph_id): (Uuid, bool, Uuid)| {
                if let Some(mut graph_store) = workspace.write().get_graph_store(graph_id) {
                    graph_store.write().set_node_inverted(node_id, inverted);
                }
                workspace.write().needs_saving.set(true);
            })
        };

        let add_root_scenery_analyzers = {
            EventHandler::new(move |analyzers: Vec<AnalyzerInfo>| {
                let graph_id = *workspace.read().root_scenery_id.read();
                if let Some(mut graph_store) = workspace.write().get_graph_store(graph_id) {
                    graph_store.write().add_analyzers(&analyzers);
                } else {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log("no root scenery found! Cannot add analyzers!");
                }
            })
        };

        let add_root_scenery_edges = {
            EventHandler::new(move |connect_infos: Vec<ConnectInfo>| {
                let graph_id = *workspace.read().root_scenery_id.read();
                if let Some(mut edges) = workspace.write().get_graph_edges(graph_id) {
                    edges.set(connect_infos);
                } else {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log("no root scenery found! Cannot add edges!");
                }
            })
        };

        let center_graph = {
            EventHandler::new(move |(graph_id, save_changes): (Uuid, bool)| {
                let mut workspace_write = workspace.write();
                workspace_write.center_graph(graph_id);
                if save_changes {
                    workspace_write.needs_saving.set(true);
                }
            })
        };

        let zoom_to_fit = {
            EventHandler::new(move |(graph_id, save_changes): (Uuid, bool)| {
                let mut workspace_write = workspace.write();
                workspace_write.zoom_to_fit(graph_id);
                if save_changes {
                    workspace_write.needs_saving.set(true);
                }
            })
        };

        let set_active_tab = {
            EventHandler::new(move |active_tab: Option<Uuid>| {
                workspace.write().active_tab.set(active_tab);
            })
        };

        Self {
            add_new_group_tab,
            set_root_scenery_id,
            remove_tab,
            set_needs_saving,
            clear_workspace,
            add_root_scenery_nodes,
            add_root_scenery_analyzers,
            add_root_scenery_edges,
            set_active_tab,
            add_optical_node,
            add_reference_node,
            add_analyzer_node,
            remove_nodes,
            update_node_positions,
            add_edge,
            center_graph,
            zoom_to_fit,
            delete_edge,
            update_edge,
            invert_node,
            update_edges,
            set_node_name,
        }
    }
}
