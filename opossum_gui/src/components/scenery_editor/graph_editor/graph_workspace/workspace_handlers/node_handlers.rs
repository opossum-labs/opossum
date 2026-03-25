use std::collections::HashMap;

use crate::components::scenery_editor::graph_editor::graph_workspace::{
    GraphsWorkspaceState,
    workspace_handlers::helper_functions::{for_each_tab, with_graph_store, with_tab},
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_core::{
    opm_document::AnalyzerInfo,
    types::api_types::{NewAnalyzerInfo, NodeInfo},
};
use uuid::Uuid;

#[derive(Clone, PartialEq, Copy)]
pub struct NodeHandlers {
    add_optical_node: EventHandler<(NodeInfo, Uuid)>,
    add_reference_node: EventHandler<(NodeInfo, Uuid)>,
    add_analyzer_node: EventHandler<(NewAnalyzerInfo, Uuid, Uuid)>,
    remove_nodes: EventHandler<(Vec<Uuid>, Uuid)>,
    update_node_positions: EventHandler<(HashMap<Uuid, Point2D<f64>>, Uuid)>,
    invert_node: EventHandler<(Uuid, bool, Uuid)>,
    set_node_name: EventHandler<(String, Uuid, Uuid, bool)>,
    add_group_nodes: EventHandler<(Uuid, Vec<NodeInfo>)>,
    add_group_analyzers: EventHandler<(Uuid, Vec<AnalyzerInfo>)>,
    remove_droppable_group: EventHandler<()>,
}

impl NodeHandlers {
    pub fn new(workspace: Signal<GraphsWorkspaceState>) -> Self {
        Self {
            add_optical_node: add_optical_node_handler(workspace),
            add_reference_node: add_reference_node_handler(workspace),
            add_analyzer_node: add_analyzer_node_handler(workspace),
            remove_nodes: remove_nodes_handler(workspace),
            update_node_positions: update_node_positions_handler(workspace),
            invert_node: invert_node_handler(workspace),
            set_node_name: set_node_name_handler(workspace),
            add_group_nodes: add_group_nodes_handler(workspace),
            add_group_analyzers: add_group_analyzers_handler(workspace),
            remove_droppable_group: remove_droppable_group_handler(workspace),
        }
    }
    pub fn add_optical_node(&self, node: NodeInfo, graph_id: Uuid) {
        self.add_optical_node.call((node, graph_id));
    }

    pub fn add_reference_node(&self, node: NodeInfo, graph_id: Uuid) {
        self.add_reference_node.call((node, graph_id));
    }

    pub fn add_analyzer_node(&self, analyzer: NewAnalyzerInfo, analyzer_id: Uuid, graph_id: Uuid) {
        self.add_analyzer_node
            .call((analyzer, analyzer_id, graph_id));
    }

    pub fn remove_nodes(&self, node_ids: Vec<Uuid>, graph_id: Uuid) {
        self.remove_nodes.call((node_ids, graph_id));
    }

    pub fn update_node_positions(&self, positions: HashMap<Uuid, Point2D<f64>>, graph_id: Uuid) {
        self.update_node_positions.call((positions, graph_id));
    }

    pub fn invert_node(&self, node_id: Uuid, inverted: bool, graph_id: Uuid) {
        self.invert_node.call((node_id, inverted, graph_id));
    }

    pub fn set_node_name(&self, name: String, node_id: Uuid, graph_id: Uuid, needs_saving: bool) {
        self.set_node_name
            .call((name, node_id, graph_id, needs_saving));
    }

    pub fn add_group_nodes(&self, group_id: Uuid, nodes: Vec<NodeInfo>) {
        self.add_group_nodes.call((group_id, nodes));
    }

    pub fn add_group_analyzers(&self, group_id: Uuid, analyzers: Vec<AnalyzerInfo>) {
        self.add_group_analyzers.call((group_id, analyzers));
    }

    pub fn remove_droppable_group(&self) {
        self.remove_droppable_group.call(());
    }
}

fn add_optical_node_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(NodeInfo, Uuid)> {
    EventHandler::new(move |(node_info, graph_id)| {
        with_graph_store(workspace, graph_id, true, |store| {
            store.add_new_optical_node(&node_info);
        });
    })
}
fn add_reference_node_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(NodeInfo, Uuid)> {
    EventHandler::new(move |(node_info, graph_id)| {
        with_graph_store(workspace, graph_id, true, |store| {
            store.add_new_reference_node(&node_info);
        });
    })
}
fn add_analyzer_node_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(NewAnalyzerInfo, Uuid, Uuid)> {
    EventHandler::new(move |(info, analyzer_id, graph_id)| {
        with_graph_store(workspace, graph_id, true, |store| {
            store.add_new_analyzer(info, analyzer_id);
        });
    })
}
fn invert_node_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, bool, Uuid)> {
    EventHandler::new(move |(node_id, inverted, graph_id)| {
        with_graph_store(workspace, graph_id, true, |store| {
            store.set_node_inverted(node_id, inverted);
        });
    })
}
fn remove_nodes_handler(
    mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(Vec<Uuid>, Uuid)> {
    EventHandler::new(move |(node_ids, graph_id)| {
        let mut ws = workspace.write();

        if let Some(mut graph_store) = ws.get_graph_store(graph_id) {
            graph_store.write().remove_nodes_by_id(&node_ids);
        }

        ws.remove_tabs(&node_ids);
        ws.needs_saving.set(true);
    })
}
fn update_node_positions_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(HashMap<Uuid, Point2D<f64>>, Uuid)> {
    EventHandler::new(move |(positions, graph_id)| {
        with_graph_store(workspace, graph_id, true, |store| {
            store.update_node_positions(positions);
        });
    })
}
fn set_node_name_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(String, Uuid, Uuid, bool)> {
    EventHandler::new(
        move |(name, node_id, graph_id, needs_saving): (String, Uuid, Uuid, bool)| {
            with_graph_store(workspace, graph_id, needs_saving, |store| {
                store.set_name_of_node(node_id, name.clone());
            });
            with_tab(workspace, node_id, needs_saving, |tab| {
                tab.graph_info.name.clone_from(&name);
            });
            for_each_tab(workspace, needs_saving, |tab| {
                if let Some((_, h_name)) = tab
                    .graph_info
                    .hierarchy
                    .iter_mut()
                    .find(|(h_id, _)| *h_id == node_id)
                {
                    h_name.clone_from(&name);
                }
            });
        },
    )
}
fn add_group_nodes_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, Vec<NodeInfo>)> {
    EventHandler::new(move |(group_id, nodes): (Uuid, Vec<NodeInfo>)| {
        with_graph_store(workspace, group_id, false, |store| {
            store.add_nodes(&nodes);
        });
    })
}
fn add_group_analyzers_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, Vec<AnalyzerInfo>)> {
    EventHandler::new(move |(group_id, analyzers): (Uuid, Vec<AnalyzerInfo>)| {
        with_graph_store(workspace, group_id, false, |store| {
            store.add_analyzers(&analyzers);
        });
    })
}

fn remove_droppable_group_handler(mut workspace: Signal<GraphsWorkspaceState>) -> EventHandler<()> {
    EventHandler::new(move |_| {
        workspace.write().drop_in_group.set(None);
    })
}
