use std::collections::HashMap;

use dioxus::{
    html::geometry::euclid::default::{Point2D, Rect, Size2D},
    prelude::*,
};
use opossum_core::types::api_types::ConnectInfo;
use uuid::Uuid;

use crate::components::scenery_editor::{
    NodeType,
    constants::{MAX_ZOOM, MIN_ZOOM},
    graph_editor::{
        DragStatus,
        graph_workspace::{EditorState, GraphState, GraphStore},
    },
};

#[derive(Clone, PartialEq)]
pub struct SelectedNode {
    pub node_id: Uuid,
    pub graph_id: Uuid,
    pub node_type: NodeType,
}

#[derive(Clone, Eq, PartialEq, Default)]
pub struct GraphsWorkspaceState {
    pub tabs: Signal<HashMap<Uuid, Signal<GraphState>>>,
    pub tab_order: Signal<Vec<Uuid>>,
    pub active_tab: Signal<Uuid>,
    pub root_scenery_id: Signal<Uuid>,
    pub editor_area: Signal<Rect<f64>>,
    pub needs_saving: Signal<bool>,
    pub drag_status: Signal<DragStatus>,
    pub selection_box: Signal<Option<Rect<f64>>>,
    pub drop_in_group: Signal<Option<(Uuid, usize)>>,
}

impl GraphsWorkspaceState {
    pub(in super::super) fn get_graph_store(&self, graph_id: Uuid) -> Option<Signal<GraphStore>> {
        self.tabs
            .read()
            .get(&graph_id)
            .map(|g| g.read().graph_store)
    }
    pub(in super::super) fn get_graph_state(&self, graph_id: Uuid) -> Option<Signal<GraphState>> {
        self.tabs.read().get(&graph_id).copied()
    }
    pub(in super::super) fn get_tab(&self, graph_id: Uuid) -> Option<Signal<GraphState>> {
        self.tabs.read().get(&graph_id).copied()
    }
    pub fn get_graph_store_read(&self, graph_id: Uuid) -> Option<ReadSignal<GraphStore>> {
        self.tabs
            .read()
            .get(&graph_id)
            .map(|g| g.read().graph_store.into())
    }
    pub(in super::super) fn get_editor_state(&self, graph_id: Uuid) -> Option<Signal<EditorState>> {
        self.tabs
            .read()
            .get(&graph_id)
            .map(|g| g.read().editor_state)
    }
    pub(in super::super) fn get_graph_edges(
        &self,
        graph_id: Uuid,
    ) -> Option<Signal<Vec<ConnectInfo>>> {
        self.tabs
            .read()
            .get(&graph_id)
            .map(|g| g.read().graph_store.read().edges())
    }
    pub fn get_graph_bounding_box(&self, graph_id: Uuid) -> Option<Rect<f64>> {
        self.tabs
            .read()
            .get(&graph_id)
            .map(|g| g.read().graph_store.read().get_bounding_box())
    }

    pub(in super::super) fn center_graph(&self, graph_id: Uuid) {
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

    pub(in super::super) fn remove_tabs(&mut self, tab_ids: &Vec<Uuid>) {
        for id in tab_ids {
            self.tabs.write().remove(id);
            self.tab_order.write().retain(|x| x != id);
        }

        let act_tab = *self.active_tab.read();
        if tab_ids.contains(&act_tab) {
            let root_id = *self.root_scenery_id.read();
            self.active_tab.set(root_id);
        }
    }

    pub(in super::super) fn zoom_to_fit(&self, graph_id: Uuid) {
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
        let editor_size = *self.editor_area.read();
        Point2D::new(editor_size.width() / 2., editor_size.height() / 2.)
    }
    pub fn get_view_port_size(&self) -> Size2D<f64> {
        self.editor_area.read().size
    }
}
