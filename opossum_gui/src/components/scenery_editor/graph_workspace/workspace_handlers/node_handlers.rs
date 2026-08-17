use std::collections::{HashMap, HashSet};

use crate::components::scenery_editor::graph_workspace::{
    GraphStateStoreExt, GraphStore, GraphStoreStoreExt, GraphStoreStoreImplExt,
    GraphsWorkspaceState, GraphsWorkspaceStateStoreExt, GraphsWorkspaceStateStoreImplExt,
    workspace_handlers::helper_functions::{for_each_tab, with_graph_store, with_tab},
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_core::{
    gain::GainModel,
    prelude::PortType,
    types::api_types::{AnalyzerItemDto, NewAnalyzerInfo, NodeInfo},
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
    set_amp_model: EventHandler<(Uuid, Option<String>, Uuid)>,
    sync_amp_markers: EventHandler<HashMap<Uuid, GainModel>>,
    set_amplifier_candidate: EventHandler<(Uuid, bool, Uuid)>,
    sync_amplifier_candidates: EventHandler<HashSet<Uuid>>,
    set_node_name: EventHandler<(String, Uuid, Uuid, bool)>,
    add_group_nodes: EventHandler<(Uuid, Vec<NodeInfo>)>,
    add_group_analyzers: EventHandler<(Uuid, Vec<AnalyzerItemDto>)>,
    remove_droppable_group: EventHandler<()>,
    update_group_ports: EventHandler<(Vec<String>, Vec<String>, Uuid)>,
    remove_group_port: EventHandler<(String, Uuid, PortType)>,
    node_click: EventHandler<(Uuid, Uuid, bool, usize, bool)>,
    set_node_active: EventHandler<(Uuid, Uuid, bool, usize)>,
    remove_from_node_selection: EventHandler<(Uuid, Uuid)>,
    add_to_node_selection: EventHandler<(Uuid, Uuid, bool)>,
    clear_graph_store: EventHandler<Uuid>,
}

impl NodeHandlers {
    pub fn new(workspace: Store<GraphsWorkspaceState>) -> Self {
        Self {
            add_optical_node: add_optical_node_handler(workspace),
            add_reference_node: add_reference_node_handler(workspace),
            add_analyzer_node: add_analyzer_node_handler(workspace),
            remove_nodes: remove_nodes_handler(workspace),
            update_node_positions: update_node_positions_handler(workspace),
            invert_node: invert_node_handler(workspace),
            set_amp_model: set_amp_model_handler(workspace),
            sync_amp_markers: sync_amp_markers_handler(workspace),
            set_amplifier_candidate: set_amplifier_candidate_handler(workspace),
            sync_amplifier_candidates: sync_amplifier_candidates_handler(workspace),
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
            clear_graph_store: clear_graph_store_handler(workspace),
        }
    }

    /// Wipes a single tab's canvas mirror (nodes/edges/selection/port-maps) back to empty, so it can
    /// be refilled from scratch - used when undo/redo reports a structural change too coarse to patch
    /// incrementally (see `DocumentChange::GraphNeedsRefresh`).
    pub fn clear_graph_store(&self, graph_id: Uuid) {
        self.clear_graph_store.call(graph_id);
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

    /// Updates the amplification marker the canvas shows for a node. `None` removes it, which also
    /// shrinks the node back by the status line's height.
    pub fn set_amp_model(&self, node_id: Uuid, amp_model: Option<String>, graph_id: Uuid) {
        self.set_amp_model.call((node_id, amp_model, graph_id));
    }

    /// Brings every currently rendered node's amplifier marker (across every open tab) in line with
    /// `gain_models` in one pass - used after the active pump scenario changed, or after an undo/redo
    /// touched its contents, rather than one request per node.
    pub fn sync_amp_markers(&self, gain_models: HashMap<Uuid, GainModel>) {
        self.sync_amp_markers.call(gain_models);
    }

    /// Updates the amplifier-candidate flag the canvas shows for a node.
    pub fn set_amplifier_candidate(&self, node_id: Uuid, is_amplifier: bool, graph_id: Uuid) {
        self.set_amplifier_candidate
            .call((node_id, is_amplifier, graph_id));
    }

    /// Brings every currently rendered node's amplifier-candidate flag (across every open tab) in
    /// line with `candidates` in one pass - used after a candidacy toggle, or after an undo/redo
    /// touched the candidate set, rather than one request per node.
    pub fn sync_amplifier_candidates(&self, candidates: HashSet<Uuid>) {
        self.sync_amplifier_candidates.call(candidates);
    }

    pub fn set_node_name(&self, name: String, node_id: Uuid, graph_id: Uuid, needs_saving: bool) {
        self.set_node_name
            .call((name, node_id, graph_id, needs_saving));
    }

    pub fn add_group_nodes(&self, group_id: Uuid, nodes: Vec<NodeInfo>) {
        self.add_group_nodes.call((group_id, nodes));
    }

    pub fn add_group_analyzers(&self, group_id: Uuid, analyzers: Vec<AnalyzerItemDto>) {
        self.add_group_analyzers.call((group_id, analyzers));
    }

    pub fn remove_droppable_group(&self) {
        self.remove_droppable_group.call(());
    }
}

fn add_to_node_selection_handler(
    workspace: Store<GraphsWorkspaceState>,
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
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, Uuid)> {
    EventHandler::new(move |(graph_id, node_id): (Uuid, Uuid)| {
        with_graph_store(workspace, graph_id, false, |g| {
            g.remove_from_node_selection(node_id);
        });
    })
}

fn set_node_active_handler(
    workspace: Store<GraphsWorkspaceState>,
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
    workspace: Store<GraphsWorkspaceState>,
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
                let selected_nodes = g.read().selected_nodes();
                if ctrl_pressed {
                    if selected_nodes.contains_key(&node_id) {
                        g.remove_from_node_selection(node_id);
                    } else {
                        g.add_to_node_selection(node_id, is_optical_node);
                    }
                } else if !selected_nodes.contains_key(&node_id) {
                    g.set_node_active(node_id, z_index, is_optical_node);
                }
            });
        },
    )
}

fn remove_group_port_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(String, Uuid, PortType)> {
    EventHandler::new(
        move |(removed_port, group_id, port_type): (String, Uuid, PortType)| {
            // A group's own box (with its port handles) is rendered by whichever tab holds it as a
            // member node - that's its true parent, and it's independent of whether the group's own tab
            // has ever been opened (looking up the parent via the group's own cached `hierarchy`, as
            // before, silently no-ops whenever it hasn't - the common case when a node is only ever
            // viewed/dragged from its parent's tab). Scanning every open tab for the one that actually
            // contains `group_id` as a node finds the right target regardless.
            for_each_tab(workspace, false, |tab| {
                let mut graph_store = tab.graph_store();
                if graph_store.nodes().get(group_id).is_some() {
                    graph_store.remove_port_of_node(group_id, &removed_port, port_type);
                }
            });
        },
    )
}

fn update_group_ports_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(Vec<String>, Vec<String>, Uuid)> {
    EventHandler::new(
        move |(input_ports, output_ports, group_id): (Vec<String>, Vec<String>, Uuid)| {
            // See `remove_group_port_handler`'s comment - same reasoning applies here.
            for_each_tab(workspace, false, |tab| {
                let mut graph_store = tab.graph_store();
                if graph_store.nodes().get(group_id).is_some() {
                    graph_store.update_ports_of_node(
                        group_id,
                        input_ports.clone(),
                        output_ports.clone(),
                    );
                }
            });
        },
    )
}

fn add_optical_node_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(NodeInfo, Uuid)> {
    EventHandler::new(move |(node_info, graph_id)| {
        with_graph_store(workspace, graph_id, true, |store| {
            store.add_new_optical_node(&node_info);
        });
    })
}
fn add_reference_node_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(NodeInfo, Uuid)> {
    EventHandler::new(move |(node_info, graph_id)| {
        with_graph_store(workspace, graph_id, true, |store| {
            store.add_new_reference_node(&node_info);
        });
    })
}
fn add_analyzer_node_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(NewAnalyzerInfo, Uuid, Uuid)> {
    EventHandler::new(move |(info, analyzer_id, graph_id)| {
        with_graph_store(workspace, graph_id, true, |store| {
            store.add_new_analyzer(info, analyzer_id);
        });
    })
}
fn invert_node_handler(workspace: Store<GraphsWorkspaceState>) -> EventHandler<(Uuid, bool, Uuid)> {
    EventHandler::new(move |(node_id, inverted, graph_id)| {
        with_graph_store(workspace, graph_id, true, |store| {
            store.set_node_inverted(node_id, inverted);
        });
    })
}
/// Mirrors a node's gain model in the active pump scenario into the canvas. Not marked dirty here:
/// the scenario patch that caused it already marks the document unsaved (and on undo/redo the saved
/// state is restored, so mirroring must not re-dirty it).
fn set_amp_model_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, Option<String>, Uuid)> {
    EventHandler::new(
        move |(node_id, amp_model, graph_id): (Uuid, Option<String>, Uuid)| {
            with_graph_store(workspace, graph_id, false, |store| {
                store.set_amp_model_of_node(node_id, amp_model.clone());
            });
        },
    )
}
/// Bulk-mirrors a gain-model map into every currently open tab's canvas, via
/// [`GraphStore::sync_amp_markers`](super::super::workspace_state::GraphStore).
fn sync_amp_markers_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<HashMap<Uuid, GainModel>> {
    EventHandler::new(move |gain_models: HashMap<Uuid, GainModel>| {
        for_each_tab(workspace, false, |tab| {
            tab.graph_store().sync_amp_markers(&gain_models);
        });
    })
}
/// Mirrors a node's amplifier candidacy into the canvas. Not marked dirty here: the PUT that caused
/// it already marks the document unsaved (same reasoning as `set_amp_model_handler`).
fn set_amplifier_candidate_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, bool, Uuid)> {
    EventHandler::new(
        move |(node_id, is_amplifier, graph_id): (Uuid, bool, Uuid)| {
            with_graph_store(workspace, graph_id, false, |store| {
                store.set_amplifier_candidate_of_node(node_id, is_amplifier);
            });
        },
    )
}
/// Bulk-mirrors the amplifier-candidate set into every currently open tab's canvas, via
/// [`GraphStore::sync_amplifier_candidates`](super::super::workspace_state::GraphStore).
fn sync_amplifier_candidates_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<HashSet<Uuid>> {
    EventHandler::new(move |candidates: HashSet<Uuid>| {
        for_each_tab(workspace, false, |tab| {
            tab.graph_store().sync_amplifier_candidates(&candidates);
        });
    })
}
fn remove_nodes_handler(
    mut workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(Vec<Uuid>, Uuid)> {
    EventHandler::new(move |(node_ids, graph_id): (Vec<Uuid>, Uuid)| {
        if let Some(mut graph_store) = workspace.tabs().get(graph_id).map(|g| g.graph_store()) {
            graph_store.remove_nodes_by_id(&node_ids);
        }

        workspace.remove_tabs(&node_ids);
        workspace.needs_saving().set(true);
    })
}
fn update_node_positions_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(HashMap<Uuid, Point2D<f64>>, Uuid)> {
    EventHandler::new(move |(positions, graph_id)| {
        with_graph_store(workspace, graph_id, true, |store| {
            store.update_node_positions(positions);
        });
    })
}
fn set_node_name_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(String, Uuid, Uuid, bool)> {
    EventHandler::new(
        move |(name, node_id, graph_id, needs_saving): (String, Uuid, Uuid, bool)| {
            with_graph_store(workspace, graph_id, needs_saving, |store| {
                store.set_name_of_node(node_id, name.clone());
            });
            with_tab(workspace, node_id, needs_saving, |tab| {
                tab.graph_info().write().name.clone_from(&name);
            });
            for_each_tab(workspace, needs_saving, |tab| {
                if let Some((_, h_name)) = tab
                    .graph_info()
                    .write()
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
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, Vec<NodeInfo>)> {
    EventHandler::new(move |(group_id, nodes): (Uuid, Vec<NodeInfo>)| {
        with_graph_store(workspace, group_id, false, |store| {
            store.add_nodes(&nodes);
        });
    })
}
fn add_group_analyzers_handler(
    workspace: Store<GraphsWorkspaceState>,
) -> EventHandler<(Uuid, Vec<AnalyzerItemDto>)> {
    EventHandler::new(move |(group_id, analyzers): (Uuid, Vec<AnalyzerItemDto>)| {
        with_graph_store(workspace, group_id, false, |store| {
            store.add_analyzers(&analyzers);
        });
    })
}

fn remove_droppable_group_handler(workspace: Store<GraphsWorkspaceState>) -> EventHandler<()> {
    EventHandler::new(move |()| {
        workspace.drop_in_group().set(None);
    })
}

fn clear_graph_store_handler(workspace: Store<GraphsWorkspaceState>) -> EventHandler<Uuid> {
    EventHandler::new(move |graph_id: Uuid| {
        if let Some(graph_state) = workspace.tabs().get(graph_id) {
            graph_state.graph_store().set(GraphStore::default());
        }
    })
}
