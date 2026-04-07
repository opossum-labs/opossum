use crate::components::scenery_editor::graph_workspace::GraphsWorkspaceState;
use dioxus::prelude::*;
use uuid::Uuid;

#[derive(Clone, PartialEq, Copy)]
pub struct ViewHandlers {
    center_graph: EventHandler<(Uuid, bool)>,
    zoom_to_fit: EventHandler<(Uuid, bool)>,
}

impl ViewHandlers {
    pub fn new(workspace: Signal<GraphsWorkspaceState>) -> Self {
        Self {
            center_graph: center_graph_handler(workspace),
            zoom_to_fit: zoom_to_fit_handler(workspace),
        }
    }
    pub fn center_graph(&self, graph_id: Uuid, save: bool) {
        self.center_graph.call((graph_id, save));
    }

    pub fn zoom_to_fit(&self, graph_id: Uuid, save: bool) {
        self.zoom_to_fit.call((graph_id, save));
    }
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
