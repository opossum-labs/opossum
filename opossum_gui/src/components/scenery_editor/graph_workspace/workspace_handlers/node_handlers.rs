use std::collections::HashMap;

use crate::components::scenery_editor::graph_workspace::{
    GraphsWorkspaceState,
    workspace_handlers::helper_functions::{for_each_tab, with_graph_store, with_tab},
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_core::{
    opm_document::AnalyzerInfo,
    prelude::PortType,
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
    update_group_ports: EventHandler<(Vec<String>, Vec<String>, Uuid)>,
    remove_group_port: EventHandler<(String, Uuid, PortType)>,
    node_click: EventHandler<(Uuid, Uuid, bool, usize, bool)>,
    set_node_active: EventHandler<(Uuid, Uuid, bool, usize)>,
    remove_from_node_selection: EventHandler<(Uuid, Uuid)>,
    add_to_node_selection: EventHandler<(Uuid, Uuid, bool)>,
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
            update_group_ports: update_group_ports_handler(workspace),
            remove_group_port: remove_group_port_handler(workspace),
            node_click: node_click_handler(workspace),
            set_node_active: set_node_active_handler(workspace),
            remove_from_node_selection: remove_from_node_selection_handler(workspace),
            add_to_node_selection: add_to_node_selection_handler(workspace),
        }
    }

    pub fn remove_from_node_selection(&self, graph_id: Uuid, node_id: Uuid) {
        self.remove_from_node_selection.call((graph_id, node_id));
    }
    pub fn add_to_node_selection(&self, graph_id: Uuid, node_id: Uuid, is_optical_node: bool) {
        self.add_to_node_selection
            .call((graph_id, node_id, is_optical_node));
    }

    pub fn set_node_active(
        &self,
        graph_id: Uuid,
        node_id: Uuid,
        is_optical_node: bool,
        z_index: usize,
    ) {
        self.set_node_active
            .call((graph_id, node_id, is_optical_node, z_index));
    }
    pub fn node_click(
        &self,
        graph_id: Uuid,
        node_id: Uuid,
        is_optical_node: bool,
        z_index: usize,
        ctrl_pressed: bool,
    ) {
        self.node_click
            .call((graph_id, node_id, is_optical_node, z_index, ctrl_pressed));
    }

    pub fn remove_group_port(&self, removed_port: String, group_id: Uuid, port_type: PortType) {
        self.remove_group_port
            .call((removed_port, group_id, port_type));
    }
    pub fn update_group_ports(
        &self,
        input_ports: Vec<String>,
        output_ports: Vec<String>,
        group_id: Uuid,
    ) {
        self.update_group_ports
            .call((input_ports, output_ports, group_id));
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

fn add_to_node_selection_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, Uuid, bool)> {
    EventHandler::new(
        move |(graph_id, node_id, is_optical_node): (Uuid, Uuid, bool)| {
            with_graph_store(workspace, graph_id, false, |g| {
                g.add_to_node_selection(node_id, is_optical_node);
            });
        },
    )
}

fn remove_from_node_selection_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, Uuid)> {
    EventHandler::new(move |(graph_id, node_id): (Uuid, Uuid)| {
        with_graph_store(workspace, graph_id, false, |g| {
            g.remove_from_node_selection(node_id);
        });
    })
}

fn set_node_active_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, Uuid, bool, usize)> {
    EventHandler::new(
        move |(graph_id, node_id, is_optical_node, z_index): (Uuid, Uuid, bool, usize)| {
            with_graph_store(workspace, graph_id, false, |g| {
                g.set_node_active(node_id, z_index, is_optical_node);
            });
        },
    )
}

fn node_click_handler(
    workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, Uuid, bool, usize, bool)> {
    EventHandler::new(
        move |(graph_id, node_id, is_optical_node, z_index, ctrl_pressed): (
            Uuid,
            Uuid,
            bool,
            usize,
            bool,
        )| {
            with_graph_store(workspace, graph_id, false, |g| {
                if ctrl_pressed {
                    if g.selected_nodes().contains_key(&node_id) {
                        g.remove_from_node_selection(node_id);
                    } else {
                        g.add_to_node_selection(node_id, is_optical_node);
                    }
                } else if !g.selected_nodes().contains_key(&node_id) {
                    g.set_node_active(node_id, z_index, is_optical_node);
                }
            });
        },
    )
}

fn remove_group_port_handler(
    mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(String, Uuid, PortType)> {
    EventHandler::new(
        move |(removed_port, group_id, port_type): (String, Uuid, PortType)| {
            let root_id = *workspace.read().root_scenery_id.read();
            let ws = workspace.write();

            if let Some(graph_state) = ws.get_graph_state(group_id) {
                let parent_id = graph_state
                    .read()
                    .graph_info
                    .get_parent_id()
                    .unwrap_or(root_id);
                if let Some(mut graph_store) = ws.get_graph_store(parent_id) {
                    graph_store
                        .write()
                        .remove_port_of_node(group_id, &removed_port, port_type);
                }
            }
        },
    )
}

fn update_group_ports_handler(
    mut workspace: Signal<GraphsWorkspaceState>,
) -> EventHandler<(Vec<String>, Vec<String>, Uuid)> {
    EventHandler::new(
        move |(input_ports, output_ports, group_id): (Vec<String>, Vec<String>, Uuid)| {
            let ws = workspace.write();

            if let Some(graph_state) = ws.get_graph_state(group_id) {
                let parent_hierarchy_pos = graph_state.read().graph_info.hierarchy.len() - 2;
                let (parent_id, _) = graph_state.read().graph_info.hierarchy[parent_hierarchy_pos];

                if let Some(mut graph_store) = ws.get_graph_store(parent_id) {
                    graph_store
                        .write()
                        .update_ports_of_node(group_id, input_ports, output_ports);
                }
            }
        },
    )
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
    EventHandler::new(move |()| {
        workspace.write().drop_in_group.set(None);
    })
}
