use std::collections::HashMap;

use dioxus::{
    html::geometry::euclid::default::{Point2D, Rect, Size2D},
    prelude::*,
};
use uuid::Uuid;

use crate::components::scenery_editor::{
    DragStatus, NodeType,
    constants::{MAX_ZOOM, MIN_ZOOM},
    graph_workspace::{
        EditorStateStoreExt, GraphState, GraphStateStoreExt,
    },
};

#[derive(Clone, PartialEq)]
pub struct SelectedNode {
    pub node_id: Uuid,
    pub graph_id: Uuid,
    pub node_type: NodeType,
}

#[derive(Clone, PartialEq, Default, Store)]
pub struct GraphsWorkspaceState {
    tabs: HashMap<Uuid, GraphState>,
    tab_order: Vec<Uuid>,
    active_tab: Uuid,
    root_scenery_id: Uuid,
    editor_area: Rect<f64>,
    needs_saving: bool,
    drag_status: DragStatus,
    selection_box: Option<Rect<f64>>,
    drop_in_group: Option<(Uuid, usize)>,
    nodes_cut: bool,
}

#[store(pub)]
impl<Lens> Store<GraphsWorkspaceState, Lens>{
    // pub(in super::super) fn get_graph_store(&self, graph_id: Uuid) -> Option<Store<GraphStore>> {
    //     self.tabs
    //         .get(&graph_id)
    //         .copied()
    //         .map(|g| g.graph_store().into())
    // }
    // pub(in super::super) fn get_graph_state(&self, graph_id: Uuid) -> Option<Store<GraphState>> {
    //     self.tabs.get(&graph_id).copied()
    // }
    // pub(in super::super) fn get_tab(&self, graph_id: Uuid) -> Option<Store<GraphState>> {
    //     self.tabs.get(&graph_id).copied()
    // }
    // pub fn get_graph_store_read(&self, graph_id: Uuid) -> Option<GraphStore> {
    //     self.tabs
    //         .get(graph_id)
    //         .map(|g| g.graph_store())
    // }
    // pub(in super::super) fn get_editor_state(&self, graph_id: Uuid) -> Option<Store<EditorState>> {
    //     self.tabs
    //         .get(&graph_id)
    //         .map(|g| g.editor_state().into())
    // }
    // pub(in super::super) fn get_graph_edges_mut(
    //     &self,
    //     graph_id: Uuid,
    // ) -> Option<Store<Vec<ConnectInfo>, impl Writable<Target = Vec<ConnectInfo>>>> {
    //     self.tabs
    //         .get(&graph_id)
    //         .map(|g| g.graph_store().edges())
    // }
    fn get_graph_bounding_box(&self, graph_id: Uuid) -> Option<Rect<f64>> {
        self.tabs()
            .get(graph_id)
            .map(|g| g.graph_store().read().get_bounding_box())
    }

    fn center_graph(&mut self, graph_id: Uuid) {
        let bounding_box_opt = self.get_graph_bounding_box(graph_id);
        let view_center = self.get_view_port_center();
        if let (Some(graph), Some(bounding_box)) =
            (self.tabs().get(graph_id), bounding_box_opt)
        {
            let center = bounding_box.center();
            let zoom = *graph.editor_state().zoom().read();
            graph.editor_state().shift().set(Point2D::new(
                center.x.mul_add(-zoom, view_center.x),
                center.y.mul_add(-zoom, view_center.y),
            ));
        }
    }

    fn remove_tabs(&mut self, tab_ids: &Vec<Uuid>) {
        for id in tab_ids {
            self.tabs().write().remove(id);
            self.tab_order().write().retain(|x| x != id);
        }
        let act_tab=*self.active_tab().read();
        if tab_ids.contains(&act_tab) {
            self.active_tab().set(*self.root_scenery_id().read());
        }
    }

    fn zoom_to_fit(&mut self, graph_id: Uuid) {
        let bounding_box_opt = self.get_graph_bounding_box(graph_id);
        let view_box = self.get_view_port_size();
        let view_center = self.get_view_port_center();
        
        if let (Some(graph), Some(bounding_box)) =
            (self.tabs().get(graph_id), bounding_box_opt)
        {
            let padding_fac = 0.95;
            let zoom = *graph.editor_state().zoom().read();
            let height_fac = view_box.height * padding_fac / zoom / bounding_box.height();
            let width_fac = view_box.width * padding_fac / zoom / bounding_box.width();
            graph.editor_state().zoom().set((zoom * width_fac.min(height_fac)).clamp(MIN_ZOOM, MAX_ZOOM));


            let center = bounding_box.center();
            let zoom = *graph.editor_state().zoom().read();
            graph.editor_state().shift().set(Point2D::new(
                center.x.mul_add(-zoom, view_center.x),
                center.y.mul_add(-zoom, view_center.y),
            ));
        }
    }

    fn get_view_port_center(&self) -> Point2D<f64> {
        let editor_size = *self.editor_area().read();
        Point2D::new(editor_size.width() / 2., editor_size.height() / 2.)
    }
    fn get_view_port_size(&self) -> Size2D<f64> {
        self.editor_area().read().size
    }
}
