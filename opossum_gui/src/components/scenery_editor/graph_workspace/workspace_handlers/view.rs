use crate::components::scenery_editor::graph_workspace::{
    GraphsWorkspaceState, workspace_handlers::helper_functions::with_editor_state,
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use uuid::Uuid;

#[derive(Clone, PartialEq, Copy)]
pub struct ViewHandlers {
    center_graph: EventHandler<(Uuid, bool)>,
    zoom_to_fit: EventHandler<(Uuid, bool)>,
    set_zoom: EventHandler<(Uuid, f64)>,
    set_shift: EventHandler<(Uuid, Point2D<f64>)>,
}

impl ViewHandlers {
    pub fn new(workspace: Signal<GraphsWorkspaceState>) -> Self {
        Self {
            center_graph: center_graph_handler(workspace),
            zoom_to_fit: zoom_to_fit_handler(workspace),
            set_zoom: set_zoom_handler(workspace),
            set_shift: set_shift_handler(workspace),
        }
    }
    pub fn center_graph(&self, graph_id: Uuid, save: bool) {
        self.center_graph.call((graph_id, save));
    }

    pub fn zoom_to_fit(&self, graph_id: Uuid, save: bool) {
        self.zoom_to_fit.call((graph_id, save));
    }
    pub fn set_zoom(&self, graph_id: Uuid, zoom: f64) {
        self.set_zoom.call((graph_id, zoom));
    }
    pub fn set_shift(&self, graph_id: Uuid, shift: Point2D<f64>) {
        self.set_shift.call((graph_id, shift));
    }
}

fn set_shift_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, Point2D<f64>)> {
    EventHandler::new(move |(graph_id, shift)| {
        with_editor_state(workspace, graph_id, false, |e| e.shift.set(shift));
    })
}

fn set_zoom_handler(workspace: Signal<GraphsWorkspaceState>) -> EventHandler<(Uuid, f64)> {
    EventHandler::new(move |(graph_id, zoom)| {
        with_editor_state(workspace, graph_id, false, |e| e.zoom.set(zoom));
    })
}
fn center_graph_handler(mut workspace: Signal<GraphsWorkspaceState>) -> EventHandler<(Uuid, bool)> {
    EventHandler::new(move |(graph_id, save)| {
        let mut ws = workspace.write();
        ws.center_graph(graph_id);

        if save {
            ws.needs_saving.set(true);
        }
    })
}

fn zoom_to_fit_handler(mut workspace: Signal<GraphsWorkspaceState>) -> EventHandler<(Uuid, bool)> {
    EventHandler::new(move |(graph_id, save)| {
        let mut ws = workspace.write();
        ws.zoom_to_fit(graph_id);

        if save {
            ws.needs_saving.set(true);
        }
    })
}
