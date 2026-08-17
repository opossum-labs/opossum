use crate::components::scenery_editor::{
    NodeElement, NodeType, SelectedNode,
    constants::{NODE_WIDTH, SUGIYAMA_VERT_PATH_FACTOR, SUGIYAMA_VERTEX_SPACING},
    edges::edge_element::EdgeElement,
    graph_workspace::EditorState,
    ports::ports_component::Ports,
};
use dioxus::{
    html::geometry::euclid::default::{Point2D, Rect, Size2D},
    prelude::*,
};
use opossum_core::{
    gain::GainModel,
    prelude::{PortMap, PortType},
    types::api_types::{AnalyzerItemDto, ConnectInfo, NewAnalyzerInfo, NodeInfo},
    utils::to_f64,
};
use rust_sugiyama::{configure::Config, from_edges};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

/// Looks up `node_id`'s gain model in the cached active pump scenario, for seeding a freshly
/// constructed node's canvas marker (see [`crate::ACTIVE_SCENARIO_GAIN_MODELS`]).
///
/// A plain, unsubscribed read of the global signal - fine here because the caller is a one-shot
/// node-construction path running inside the live app (not the plain-conversion `From<&NodeInfo>`,
/// which stays testable outside a mounted app precisely by not doing this itself).
fn active_scenario_amp_model(node_id: Uuid) -> Option<String> {
    crate::ACTIVE_SCENARIO_GAIN_MODELS
        .read()
        .get(&node_id)
        .and_then(opossum_core::gain::GainModel::active_name)
}

/// Looks up whether `node_id` is a member of the cached amplifier-candidate set, for seeding a
/// freshly constructed node's canvas flag (see [`crate::AMPLIFIER_CANDIDATES`]).
///
/// Same reasoning as [`active_scenario_amp_model`]: a plain, unsubscribed read, fine here because
/// the caller is a one-shot node-construction path running inside the live app.
fn is_amplifier_candidate(node_id: Uuid) -> bool {
    crate::AMPLIFIER_CANDIDATES.read().contains(&node_id)
}

#[derive(Clone, PartialEq, Store, Default)]
pub struct GraphState {
    graph_store: GraphStore,
    editor_state: EditorState,
    graph_info: GraphInfo,
}

impl GraphState {
    pub const fn new(
        graph_store: GraphStore,
        editor_state: EditorState,
        graph_info: GraphInfo,
    ) -> Self {
        Self {
            graph_store,
            editor_state,
            graph_info,
        }
    }
}

impl GraphInfo {
    pub fn get_parent(&self) -> Option<(Uuid, String)> {
        let hierarchy_len = self.hierarchy.len();
        if hierarchy_len > 2 {
            Some(self.hierarchy[hierarchy_len - 2].clone())
        } else {
            None
        }
    }
}

#[derive(Clone, Eq, PartialEq, Default)]
pub struct GraphInfo {
    pub name: String,
    pub id: Uuid,
    pub hierarchy: Vec<(Uuid, String)>,
}

#[derive(Clone, Eq, PartialEq, Default)]
pub struct NodeSelection {
    pub all_nodes: Signal<HashMap<Uuid, bool>>,
    pub analyzers: Signal<HashSet<Uuid>>,
}

#[derive(Clone, PartialEq, Store, Default)]
pub struct GraphStore {
    nodes: HashMap<Uuid, NodeElement>,
    edges: Vec<EdgeElement>,
    node_selection: NodeSelection,
    mapped_ports: PortMap,
    next_node_index: usize, // unique node ID for playwright tests
    next_edge_index: usize, // unique edge ID for playwright tests
}

#[store(pub)]
impl<Lens> Store<GraphStore, Lens> {
    /// Generates the next sequential node index and increments the internal counter.
    fn fetch_next_node_index(&mut self) -> usize {
        let current_index = *self.next_node_index().read();
        *self.next_node_index().write() += 1;
        current_index
    }

    /// Generates the next sequential edge index and increments the internal counter.
    fn fetch_next_edge_index(&mut self) -> usize {
        let current_index = *self.next_edge_index().read();
        *self.next_edge_index().write() += 1;
        current_index
    }

    /// Inserts a new edge, or - if one with the same `(src_uuid, src_port, target_uuid,
    /// target_port)` identity already exists - updates it in place instead of duplicating it.
    /// A full-tab refill (e.g. the GUI's `process_fill_graph_of_group`, which re-fetches a
    /// group's entire connection list) can legitimately re-report a connection that's already
    /// present locally; unlike the `HashMap`-backed node store, `edges` is a plain `Vec` with no
    /// dedup-by-construction, so a blind push there would leave two `EdgeElement`s sharing the
    /// same key `edges_component.rs` renders edges by - which Dioxus's keyed-list diff rejects
    /// as a duplicate key (panic: "keyed siblings must each have a unique key").
    fn add_edge(&mut self, connect_info: ConnectInfo) {
        let mut edges_store = self.edges();
        let mut edges = edges_store.write();
        if let Some(existing) = edges.iter_mut().find(|e| {
            e.src_uuid() == connect_info.src_uuid()
                && e.src_port() == connect_info.src_port()
                && e.target_uuid() == connect_info.target_uuid()
                && e.target_port() == connect_info.target_port()
        }) {
            let index = existing.edge_index();
            *existing = EdgeElement::new(connect_info, index);
            return;
        }
        drop(edges);
        let index = self.fetch_next_edge_index();
        self.edges()
            .write()
            .push(EdgeElement::new(connect_info, index));
    }

    /// Removes an edge matching source and target UUIDs and ports.
    fn remove_edge(&mut self, connect_info: &ConnectInfo) {
        self.edges().write().retain(|e| {
            !(e.src_uuid() == connect_info.src_uuid()
                && e.src_port() == connect_info.src_port()
                && e.target_uuid() == connect_info.target_uuid()
                && e.target_port() == connect_info.target_port())
        });
    }
    fn shift_node_position(&mut self, node_id: Uuid, shift: Point2D<f64>) {
        if let Some(mut node) = self.nodes().get(node_id) {
            node.write().shift_position(shift);
        }
    }
    fn clear_selected_nodes(&mut self) {
        self.node_selection().write().all_nodes.write().clear();
    }
    fn add_to_node_selection(&mut self, id: Uuid, is_optical: bool) {
        self.node_selection()
            .write()
            .all_nodes
            .write()
            .insert(id, is_optical);
    }
    fn remove_from_node_selection(&mut self, id: Uuid) {
        self.node_selection().write().all_nodes.write().remove(&id);
    }

    fn set_active_node_none(&mut self) {
        self.clear_selected_nodes();
    }
    fn set_node_active(&mut self, id: Uuid, z_index: usize, is_optical: bool) {
        self.set_z_level_to_top(id, z_index);
        self.clear_selected_nodes();
        self.node_selection()
            .write()
            .all_nodes
            .write()
            .insert(id, is_optical);
    }
    fn update_node_positions(&mut self, new_positions: HashMap<Uuid, Point2D<f64>>) {
        for (id, pos) in new_positions {
            if let Some(mut node) = self.nodes().get(id) {
                node.write().set_pos(pos);
            }
        }
    }
    fn add_nodes(&mut self, nodes: &[NodeInfo]) {
        self.nodes()
            .write()
            .extend(nodes.iter().map(|node| (node.uuid(), node.into())));
    }
    fn add_analyzers(&mut self, analyzers: &[AnalyzerItemDto]) {
        self.nodes()
            .write()
            .extend(analyzers.iter().map(|dto| (dto.id, dto.into())));
    }
    fn set_name_of_node(&mut self, node_id: Uuid, name: String) {
        if let Some(mut node) = self.nodes().get(node_id) {
            node.write().set_name(name);
        }
    }
    fn remove_port_of_node(&mut self, node_id: Uuid, remove_port: &str, port_type: PortType) {
        if let Some(mut node) = self.nodes().get(node_id) {
            node.write().remove_port(remove_port, port_type);
        }
    }
    fn update_ports_of_node(
        &mut self,
        node_id: Uuid,
        input_ports: Vec<String>,
        output_ports: Vec<String>,
    ) {
        if let Some(mut node) = self.nodes().get(node_id) {
            node.write().set_ports(input_ports, output_ports);
        }
    }
    fn set_node_inverted(&mut self, node_id: Uuid, inverted: bool) {
        if let Some(mut node) = self.nodes().get(node_id) {
            node.write().set_inverted(inverted);
        }
    }
    fn set_amp_model_of_node(&mut self, node_id: Uuid, amp_model: Option<String>) {
        if let Some(mut node) = self.nodes().get(node_id) {
            node.write().set_amp_model(amp_model);
        }
    }
    /// Sets every one of this tab's nodes' canvas amplifier marker from `gain_models`, in one pass.
    ///
    /// Used to bring a whole tab's markers in line with the active pump scenario at once - after
    /// switching which scenario is active, or after an undo/redo changed the active one's contents -
    /// rather than one request per node.
    fn sync_amp_markers(&mut self, gain_models: &HashMap<Uuid, GainModel>) {
        let node_ids: Vec<Uuid> = self.nodes().read().keys().copied().collect();
        for node_id in node_ids {
            if let Some(mut node) = self.nodes().get(node_id) {
                let amp_model = gain_models.get(&node_id).and_then(GainModel::active_name);
                node.write().set_amp_model(amp_model);
            }
        }
    }
    /// Sets every one of this tab's nodes' amplifier-candidate flag from `candidates`, in one pass.
    ///
    /// Mirrors [`Self::sync_amp_markers`] exactly - used to bring a whole tab's flags in line with
    /// the document-wide candidate set at once (after a candidacy toggle or an undo/redo touching
    /// one), rather than one request per node.
    fn sync_amplifier_candidates(&mut self, candidates: &HashSet<Uuid>) {
        let node_ids: Vec<Uuid> = self.nodes().read().keys().copied().collect();
        for node_id in node_ids {
            if let Some(mut node) = self.nodes().get(node_id) {
                node.write()
                    .set_amplifier_candidate(candidates.contains(&node_id));
            }
        }
    }
    fn renumber_z_levels(&mut self) {
        let mut node_elements: Vec<(Uuid, usize)> = self
            .nodes()
            .read()
            .iter()
            .map(|n| (n.1.id(), n.1.z_index()))
            .collect();
        node_elements.sort_by_key(|e_1| e_1.1);
        for element in node_elements.iter().enumerate() {
            if let Some(mut node) = self.nodes().get(element.1.0) {
                node.write().set_z_index(element.0);
            }
        }
    }
    fn set_z_level_to_top(&mut self, node_id: Uuid, z_level: usize) {
        let number_of_nodes = self.nodes().len();
        for (id, mut elem) in self.nodes().iter() {
            let z_index = elem.read().z_index();
            if z_index > z_level && id != node_id {
                elem.write().set_z_index(z_index - 1);
            } else if id == node_id {
                elem.write().set_z_index(number_of_nodes);
            }
        }
    }
    /// Adds a new reference node to the graph store.
    /// This function creates a new `NodeElement` for the reference node and inserts it into the store.
    /// It also sets the z-index based on the current number of nodes to ensure proper layering.
    /// # Arguments:
    /// * `ref_node_info`: The `NodeInfo` containing the type and position of the new reference node.
    /// # Returns:
    /// A `NodeElement` representing the newly added reference node.
    fn add_new_reference_node(&mut self, ref_node_info: &NodeInfo) -> NodeElement {
        let mut node_element = NodeElement::from(ref_node_info);
        node_element.set_amp_model(active_scenario_amp_model(ref_node_info.uuid()));
        node_element.set_amplifier_candidate(is_amplifier_candidate(ref_node_info.uuid()));
        node_element.set_node_index(self.fetch_next_node_index());
        let id = ref_node_info.uuid();
        let nr_of_nodes = self.nodes().len();
        node_element.set_z_index(nr_of_nodes + 1);
        self.nodes().insert(id, node_element.clone());
        self.set_node_active(id, node_element.z_index(), true);
        node_element
    }
    /// Removes nodes by their IDs from the graph store.
    /// This function iterates through the provided list of node IDs,
    /// removes each node from the store, and updates the edges accordingly.
    /// # Arguments:
    /// * `deleted_ids`: A vector of `Uuid` representing the IDs of the nodes to be removed.
    fn remove_nodes_by_id(&mut self, node_ids: &[Uuid]) {
        for node_id in node_ids {
            self.nodes().remove(node_id);
            self.renumber_z_levels();
            self.edges()
                .write()
                .retain_mut(|e| e.src_uuid() != *node_id && e.target_uuid() != *node_id);
        }
        self.set_active_node_none();
    }
    /// Adds a new optical node to the graph store.
    /// This function creates a new `NodeElement` for the optical node and inserts it into the store.
    /// It also sets the z-index based on the current number of nodes to ensure proper layering.
    /// # Arguments:
    /// * `node_info`: The `NodeInfo` containing the type and position of the new node.
    fn add_new_optical_node(&mut self, node_info: &NodeInfo) {
        let mut node_element = NodeElement::from(node_info);
        node_element.set_amp_model(active_scenario_amp_model(node_info.uuid()));
        node_element.set_amplifier_candidate(is_amplifier_candidate(node_info.uuid()));
        node_element.set_node_index(self.fetch_next_node_index());
        self.nodes().insert(node_info.uuid(), node_element.clone());
        self.set_node_active(node_info.uuid(), node_element.z_index(), true);
    }
    /// Adds a new analyzer to the graph store.
    /// This function creates a new `NodeElement` for the analyzer and inserts it into the store.
    /// It also sets the z-index based on the current number of nodes to ensure proper layering.
    /// # Arguments:
    /// * `new_analyzer`: The `NewAnalyzerInfo` containing the type and position of the new analyzer.
    /// * `analyzer_id`: The unique identifier for the new analyzer.
    fn add_new_analyzer(&mut self, new_analyzer: NewAnalyzerInfo, analyzer_id: Uuid) {
        let node_index = self.fetch_next_node_index();
        let (x, y) = new_analyzer.gui_position;
        let mut node_element = NodeElement::new(
            format!("{}", new_analyzer.analyzer_type),
            NodeType::Analyzer(new_analyzer.analyzer_type),
            analyzer_id,
            Point2D::new(x, y),
            Ports::default(),
            false,
            node_index,
        );
        let nr_of_nodes = self.nodes().len();
        node_element.set_z_index(nr_of_nodes + 1);
        self.nodes().insert(analyzer_id, node_element.clone());
        self.set_node_active(analyzer_id, node_element.z_index(), false);
    }
}

impl GraphStore {
    #[must_use]
    pub fn selected_nodes(&self) -> HashMap<Uuid, bool> {
        self.node_selection.all_nodes.read().clone()
    }
    #[must_use]
    pub fn selected_optical_nodes(&self) -> HashSet<Uuid> {
        self.node_selection
            .all_nodes
            .read()
            .iter()
            .filter(|(_, optical)| **optical)
            .map(|(id, _)| id)
            .copied()
            .collect()
    }
    #[must_use]
    pub fn selected_node_ids(&self) -> HashSet<Uuid> {
        self.node_selection
            .all_nodes
            .read()
            .keys()
            .copied()
            .collect()
    }

    pub fn get_selected_nodes(&self, graph_id: Uuid) -> Vec<SelectedNode> {
        let mut selected_nodes = Vec::<SelectedNode>::new();
        for n_id in &self.selected_node_ids() {
            if let Some(n) = self.nodes.get(n_id) {
                let selected_node = SelectedNode {
                    node_id: n.id(),
                    graph_id,
                    node_type: n.node_type().clone(),
                };
                selected_nodes.push(selected_node);
            }
        }
        selected_nodes
    }

    pub fn get_bounding_box(&self) -> Rect<f64> {
        if self.nodes.is_empty() {
            return Rect::new(Point2D::zero(), Size2D::zero());
        }
        // Use the first node to initialize the bounding box
        let mut rect = self.nodes.iter().next().unwrap().1.get_bounding_box();

        // Iterate over the rest of the nodes to expand the bounding box
        for node in self.nodes.values().skip(1) {
            rect = rect.union(&node.get_bounding_box());
        }
        rect
    }
}
/// Calculates optimized positions for all nodes in the graph.
/// Places Analyzers at the top, disconnected nodes below them,
/// and the connected Sugiyama layout at the bottom.
#[must_use]
pub fn optimize_layout(
    nodes: &HashMap<Uuid, NodeElement>,
    edges: &[ConnectInfo],
) -> HashMap<Uuid, Point2D<f64>> {
    let mut new_positions = HashMap::new();
    let mut connected_node_ids = HashSet::new();

    // Determine which nodes are part of a connection
    for edge in edges {
        connected_node_ids.insert(edge.src_uuid());
        connected_node_ids.insert(edge.target_uuid());
    }

    let mut analyzers = Vec::new();
    let mut disconnected_nodes = Vec::new();

    // Categorize nodes
    for (id, node) in nodes {
        match node.node_type() {
            NodeType::Analyzer(_) => analyzers.push(*id),
            NodeType::Optical(_) => {
                if !connected_node_ids.contains(id) {
                    disconnected_nodes.push(*id);
                }
            }
        }
    }

    // Sort to ensure a deterministic layout
    analyzers.sort();
    disconnected_nodes.sort();

    // Define layout spacing constants
    let h_gap = 50.0;
    let v_gap = 100.0;

    let mut current_y = 0.0;

    // --- Row 1: Analyzers ---
    if !analyzers.is_empty() {
        current_y = layout_horizontal_row(
            &analyzers,
            nodes,
            current_y,
            h_gap,
            v_gap,
            &mut new_positions,
        );
    }

    // --- Row 2: Disconnected Optical Nodes ---
    if !disconnected_nodes.is_empty() {
        current_y = layout_horizontal_row(
            &disconnected_nodes,
            nodes,
            current_y,
            h_gap,
            v_gap,
            &mut new_positions,
        );
    }

    // --- Row 3: Connected Nodes (Sugiyama Layout with Post-Processing) ---
    if !connected_node_ids.is_empty() && !edges.is_empty() {
        layout_sugiyama_connected(nodes, edges, current_y, v_gap, &mut new_positions);
    }

    new_positions
}

/// Helper function to lay out a single flat row of nodes horizontally.
/// Returns the updated next available Y position.
fn layout_horizontal_row(
    node_ids: &[Uuid],
    nodes: &HashMap<Uuid, NodeElement>,
    current_y: f64,
    h_gap: f64,
    v_gap: f64,
    new_positions: &mut HashMap<Uuid, Point2D<f64>>,
) -> f64 {
    let mut current_x = 0.0;
    let mut max_row_height = 0.0;

    for id in node_ids {
        new_positions.insert(*id, Point2D::new(current_x, current_y));
        current_x += NODE_WIDTH + h_gap;

        if let Some(node) = nodes.get(id) {
            let height = node.get_bounding_box().height();
            if height > max_row_height {
                max_row_height = height;
            }
        }
    }

    current_y + max_row_height + v_gap
}

/// Helper function to process the Sugiyama layout and apply custom post-processing barycenter logic.
fn layout_sugiyama_connected(
    nodes: &HashMap<Uuid, NodeElement>,
    edges: &[ConnectInfo],
    start_y: f64,
    v_gap: f64,
    new_positions: &mut HashMap<Uuid, Point2D<f64>>,
) {
    let sugiyama_config = Config {
        vertex_spacing: SUGIYAMA_VERTEX_SPACING,
        ..Default::default()
    };

    let mut reg = UuidRegistry::new();

    // The rust_sugiyama crate strictly requires u32 for node IDs on input.
    let edges_u32: Vec<(u32, u32)> = edges
        .iter()
        .map(|edge| {
            let src = reg.register(edge.src_uuid());
            let target = reg.register(edge.target_uuid());
            (src, target)
        })
        .collect();

    let layouts = from_edges(&edges_u32, &sugiyama_config);
    let mut current_sugiyama_y = start_y;

    for (layout, _group_height, _) in layouts {
        // Group nodes by their X-coordinate.
        let mut layers: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
        for &(node_id, (_y_pos, x_layer)) in &layout {
            // Safely handle potential negative coordinates from layout engine before casting
            let positive_x = x_layer.round().max(0.0);
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let discrete_layer = positive_x as usize;

            // Safely convert usize back to u32 since IDs originated from our u32 registry
            let node_id_u32 = u32::try_from(node_id).expect(
                "Sugiyama node ID exceeded u32 limits, which is impossible based on Registry",
            );

            layers.entry(discrete_layer).or_default().push(node_id_u32);
        }

        let mut subgraph_positions: HashMap<Uuid, Point2D<f64>> = HashMap::new();
        let mut max_y_in_group = current_sugiyama_y;

        // Process each layer from left to right
        for (&orig_x_layer, node_ids_in_layer) in &layers {
            let mut node_ideal_ys: Vec<(u32, f64)> = Vec::new();

            for &node_id_u32 in node_ids_in_layer {
                let target_uuid = reg.get_uuid(node_id_u32).unwrap();
                let mut sum_y = 0.0;
                let mut count = 0.0;

                // Find incoming edges to calculate the ideal vertical position
                for edge in edges {
                    if edge.target_uuid() == target_uuid {
                        let src_uuid = edge.src_uuid();

                        if let Some(src_pos) = subgraph_positions.get(&src_uuid)
                            && let Some(src_node) = nodes.get(&src_uuid)
                        {
                            let port_rel_pos =
                                src_node.rel_port_position(PortType::Output, edge.src_port());
                            let port_abs_y = src_pos.y + port_rel_pos.y;
                            sum_y += port_abs_y;
                            count += 1.0;
                        }
                    }
                }

                // Average the Y positions (Barycenter logic)
                let ideal_y = if count > 0.0 {
                    sum_y / count
                } else {
                    // Fallback: extract the original f64 Y order from Sugiyama.
                    let orig_y_pos = layout
                        .iter()
                        .find(|l| l.0 == usize::try_from(node_id_u32).unwrap())
                        .unwrap()
                        .1
                        .0;
                    orig_y_pos * SUGIYAMA_VERT_PATH_FACTOR
                };

                node_ideal_ys.push((node_id_u32, ideal_y));
            }

            // Sort nodes safely
            node_ideal_ys
                .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            // Extract original f64 vertical slots.
            let mut original_ys: Vec<f64> = node_ids_in_layer
                .iter()
                .map(|&id| {
                    layout
                        .iter()
                        .find(|l| l.0 == usize::try_from(id).unwrap())
                        .unwrap()
                        .1
                        .0
                })
                .collect();

            original_ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            // Assign final positions
            for (i, &(node_id_u32, _)) in node_ideal_ys.iter().enumerate() {
                let uuid = reg.get_uuid(node_id_u32).unwrap();
                let final_x = to_f64(orig_x_layer);

                let assigned_y_val = original_ys[i];
                let final_y = assigned_y_val.mul_add(SUGIYAMA_VERT_PATH_FACTOR, current_sugiyama_y);

                let final_pos = Point2D::new(final_x, final_y);
                subgraph_positions.insert(uuid, final_pos);
                new_positions.insert(uuid, final_pos);

                if final_y > max_y_in_group {
                    max_y_in_group = final_y;
                }
            }
        }
        // Move down for the next disconnected subgraph
        current_sugiyama_y = max_y_in_group + v_gap;
    }
}

/// Registry to map UUIDs to u32 IDs.
struct UuidRegistry {
    forward: HashMap<Uuid, u32>,
    backward: HashMap<u32, Uuid>,
    next_id: u32,
}

impl UuidRegistry {
    fn new() -> Self {
        Self {
            forward: HashMap::new(),
            backward: HashMap::new(),
            next_id: 0,
        }
    }

    fn register(&mut self, uuid: Uuid) -> u32 {
        if let Some(&id) = self.forward.get(&uuid) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.forward.insert(uuid, id);
        self.backward.insert(id, uuid);
        id
    }

    fn get_uuid(&self, id: u32) -> Option<Uuid> {
        self.backward.get(&id).copied()
    }
}
