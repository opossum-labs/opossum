use crate::components::scenery_editor::{
    NodeElement, NodeType, SelectedNode,
    constants::{
        EDGE_DISTANCE_FIELD_HEIGHT, LAYOUT_LINE_GAP, LAYOUT_NODE_GAP, MAX_ZOOM, NODE_WIDTH,
        SUGIYAMA_VERT_PATH_FACTOR, SUGIYAMA_VERTEX_SPACING, VIEW_PADDING_FACTOR,
    },
    edges::{edge_bounds, edge_element::EdgeElement},
    graph_workspace::{
        EditorState,
        test_registry::{EdgeKey, IndexRegistry},
    },
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

/// Builds the canvas element of a freshly fetched node, carrying the amplifier state the caches
/// hold for it.
///
/// `From<&NodeInfo> for NodeElement` deliberately does *not* do this: it stays free of any
/// `GlobalSignal` read so that it remains testable outside a mounted app. The price is that every
/// path materialising nodes has to seed the two fields itself — which is why they all go through
/// here rather than each remembering to. A path that forgot it showed up as amplifier markers
/// silently vanishing from a whole tab whenever it was refilled (opening a group, dropping a node
/// into one, loading a file), while the document still listed the nodes as amplifiers.
fn node_element_with_amp_state(node_info: &NodeInfo) -> NodeElement {
    let mut node_element = NodeElement::from(node_info);
    node_element.set_amp_model(active_scenario_amp_model(node_info.uuid()));
    node_element.set_amplifier_candidate(is_amplifier_candidate(node_info.uuid()));
    node_element
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
    /// Persistent sequential test indices for nodes across undo/redo actions.
    node_indices: IndexRegistry<Uuid>,
    /// Persistent sequential test indices for edges across undo/redo actions.
    edge_indices: IndexRegistry<EdgeKey>,
}

#[store(pub)]
impl<Lens> Store<GraphStore, Lens> {
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
        // Fetch or restore the persistent edge test index
        let edge_key = EdgeKey::from(&connect_info);
        let index = self.edge_indices().write().get_or_create(edge_key);
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

        // Avoid re-allocating and re-notifying selection signals if this node is already the sole active node
        let current_selected = self.node_selection().read().all_nodes.read().clone();
        if current_selected.len() == 1 && current_selected.contains_key(&id) {
            return;
        }
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
        for node in nodes {
            let mut node_element = node_element_with_amp_state(node);
            let index = self.node_indices().write().get_or_create(node.uuid());
            node_element.set_node_index(index);
            self.nodes().insert(node.uuid(), node_element);
        }
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
    fn set_amplifier_candidate_of_node(&mut self, node_id: Uuid, is_amplifier: bool) {
        if let Some(mut node) = self.nodes().get(node_id) {
            node.write().set_amplifier_candidate(is_amplifier);
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
    fn set_z_level_to_top(&mut self, node_id: Uuid, _z_level: usize) {
        let number_of_nodes = self.nodes().len();
        if let Some(mut elem) = self.nodes().get(node_id) {
            // Nur diese EINE Node mutieren, alle anderen in Ruhe lassen
            elem.write().set_z_index(number_of_nodes + 1);
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
        let mut node_element = node_element_with_amp_state(ref_node_info);
        let index = self
            .node_indices()
            .write()
            .get_or_create(ref_node_info.uuid());
        node_element.set_node_index(index);
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
        let mut node_element = node_element_with_amp_state(node_info);
        let index = self.node_indices().write().get_or_create(node_info.uuid());
        node_element.set_node_index(index);
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
        let node_index = self.node_indices().write().get_or_create(analyzer_id);
        let (x, y) = new_analyzer.gui_position;
        let mut node_element = NodeElement::new(
            format!("{}", new_analyzer.analyzer_type),
            NodeType::Analyzer(Box::new(new_analyzer.analyzer_type)),
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

    /// The box the graph covers on the canvas; see [`covered_box`].
    pub fn get_bounding_box(&self) -> Rect<f64> {
        covered_box(
            &self.nodes,
            self.edges.iter().map(EdgeElement::info),
            &self.mapped_ports,
        )
    }
}

/// The box that `nodes` and the connections `edges` between them cover on the canvas, including the
/// tags of the ports `mapped_ports` maps to the ports of the group they belong to.
///
/// Zoom to Fit fits this box, and the auto layout measures its candidates with it, so both agree.
///
/// # Arguments
///
/// * `nodes` - the nodes, at their positions.
/// * `edges` - the connections; one with an end outside `nodes` is left out.
/// * `mapped_ports` - the ports of the group, mapped to ports of `nodes`.
///
/// # Returns
///
/// The smallest box around all of them, or an empty box at the origin if there are no nodes.
fn covered_box<'a>(
    nodes: &HashMap<Uuid, NodeElement>,
    edges: impl IntoIterator<Item = &'a ConnectInfo>,
    mapped_ports: &PortMap,
) -> Rect<f64> {
    let node_boxes = nodes.values().map(|node| {
        let mapped = |ports: &Vec<String>| {
            ports
                .iter()
                .any(|port| mapped_ports.contains_port_of_node(node.id(), port))
        };
        node.get_bounding_box_with_mapped_ports(
            mapped(node.input_ports()),
            mapped(node.output_ports()),
        )
    });
    let edge_boxes = edges.into_iter().filter_map(|edge| {
        let output = nodes.get(&edge.src_uuid())?;
        let input = nodes.get(&edge.target_uuid())?;
        Some(edge_bounds(
            output.abs_port_position(PortType::Output, edge.src_port()),
            input.abs_port_position(PortType::Input, edge.target_port()),
        ))
    });
    // From the corners rather than by `union`, which drops boxes without height.
    Rect::from_points(
        node_boxes
            .chain(edge_boxes)
            .flat_map(|covered| [covered.min(), covered.max()]),
    )
}
/// Calculates optimized positions for all nodes in the graph.
///
/// Analyzers go to the top, unconnected optical nodes below them, and the connected parts
/// (Sugiyama layout) to the bottom. Rows too long for the view are broken into lines: of all line
/// lengths, the one is taken at which the whole layout fits `view` at the largest zoom, and on a tie
/// the longer one.
///
/// # Arguments
///
/// * `nodes` - the nodes to place.
/// * `edges` - the connections between them.
/// * `mapped_ports` - the ports of the group, mapped to ports of `nodes`; their tags take room.
/// * `view` - the size of the canvas in pixels. An empty size breaks no line.
///
/// # Returns
///
/// The new top-left position of every node.
#[must_use]
pub fn optimize_layout(
    nodes: &HashMap<Uuid, NodeElement>,
    edges: &[ConnectInfo],
    mapped_ports: &PortMap,
    view: Size2D<f64>,
) -> HashMap<Uuid, Point2D<f64>> {
    let blocks = layout_blocks(nodes, edges);
    let mut line_widths: Vec<f64> = blocks
        .iter()
        .flatten()
        .map(|(_, pos)| pos.x + NODE_WIDTH)
        .collect();
    line_widths.sort_by(f64::total_cmp);
    line_widths.dedup();

    let mut best: Option<(f64, HashMap<Uuid, Point2D<f64>>)> = None;
    for line_width in line_widths {
        let positions = stack_blocks(&blocks, nodes, edges, line_width);
        let zoom = fit_zoom(&positions, nodes, edges, mapped_ports, view);
        // The widths ascend, so `>=` hands a tie to the longer lines.
        if best
            .as_ref()
            .is_none_or(|(best_zoom, _)| zoom >= *best_zoom)
        {
            best = Some((zoom, positions));
        }
    }
    best.map_or_default(|(_, positions)| positions)
}

/// Places the nodes in blocks, before any line is broken: the analyzers in one row, the unconnected
/// optical nodes in one row, and each connected part by the Sugiyama layout.
///
/// # Arguments
///
/// * `nodes` - the nodes to place.
/// * `edges` - the connections between them.
///
/// # Returns
///
/// The blocks from top to bottom, each node with its position within its block.
fn layout_blocks(
    nodes: &HashMap<Uuid, NodeElement>,
    edges: &[ConnectInfo],
) -> Vec<Vec<(Uuid, Point2D<f64>)>> {
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

    let row = |ids: &[Uuid]| -> Vec<(Uuid, Point2D<f64>)> {
        ids.iter()
            .enumerate()
            .map(|(i, id)| {
                let x = to_f64(i) * (NODE_WIDTH + LAYOUT_NODE_GAP);
                (*id, Point2D::new(x, 0.0))
            })
            .collect()
    };
    let mut blocks = vec![row(&analyzers), row(&disconnected_nodes)];
    if !edges.is_empty() {
        blocks.extend(layout_sugiyama_connected(nodes, edges));
    }
    blocks
}

/// Breaks every block into lines no wider than `line_width` and stacks the blocks below each other.
///
/// # Arguments
///
/// * `blocks` - the blocks from [`layout_blocks`], top to bottom.
/// * `nodes` - the nodes, for their heights and ports.
/// * `edges` - the connections between them.
/// * `line_width` - the widest a line may get.
///
/// # Returns
///
/// The position of every node, with the top-left corner of the layout at the origin.
fn stack_blocks(
    blocks: &[Vec<(Uuid, Point2D<f64>)>],
    nodes: &HashMap<Uuid, NodeElement>,
    edges: &[ConnectInfo],
    line_width: f64,
) -> HashMap<Uuid, Point2D<f64>> {
    let mut positions = HashMap::new();
    let mut top = 0.0;
    for block in blocks {
        top = fold_into_lines(block, nodes, edges, line_width, top, &mut positions);
    }
    positions
}

/// Breaks one block into lines no wider than `line_width` and stacks them, starting at `top`.
///
/// The block's columns (the nodes sharing an x) are taken from left to right; a column that would
/// end beyond `line_width` from the start of its line opens the next line. Each line is moved to
/// x = 0, its top at least [`LAYOUT_LINE_GAP`] below the bottom of the line before. A connection
/// from one line to the next passes halfway between its ports' heights, and carries its distance
/// field there; the gap grows until that height keeps [`EDGE_DISTANCE_FIELD_HEIGHT`] from both
/// lines.
///
/// # Arguments
///
/// * `block` - the nodes with their positions within the block.
/// * `nodes` - the nodes, for their heights and ports.
/// * `edges` - the connections; those from one line to the next set the gap between the two.
/// * `line_width` - the widest a line may get; at least [`NODE_WIDTH`], so that every line holds a
///   column.
/// * `top` - where the first line starts.
/// * `positions` - receives the new position of every node of the block.
///
/// # Returns
///
/// Where the next block starts: [`LAYOUT_LINE_GAP`] below the last line, or `top` for an empty
/// block.
fn fold_into_lines(
    block: &[(Uuid, Point2D<f64>)],
    nodes: &HashMap<Uuid, NodeElement>,
    edges: &[ConnectInfo],
    line_width: f64,
    top: f64,
    positions: &mut HashMap<Uuid, Point2D<f64>>,
) -> f64 {
    let mut by_x = block.to_vec();
    by_x.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));

    let mut lines: Vec<Vec<(Uuid, Point2D<f64>)>> = Vec::new();
    let mut line_start = 0.0;
    for (id, pos) in by_x {
        if lines.is_empty() || pos.x - line_start + NODE_WIDTH > line_width {
            line_start = pos.x;
            lines.push(Vec::new());
        }
        if let Some(line) = lines.last_mut() {
            line.push((id, Point2D::new(pos.x - line_start, pos.y)));
        }
    }

    let extents: Vec<(f64, f64)> = lines
        .iter()
        .map(|line| {
            let line_top = line
                .iter()
                .map(|(_, pos)| pos.y)
                .fold(f64::INFINITY, f64::min);
            let line_bottom = line
                .iter()
                .map(|(id, pos)| pos.y + node_height(nodes, *id))
                .fold(f64::NEG_INFINITY, f64::max);
            (line_top, line_bottom)
        })
        .collect();
    let line_and_y: HashMap<Uuid, (usize, f64)> = lines
        .iter()
        .enumerate()
        .flat_map(|(i, line)| line.iter().map(move |(id, pos)| (*id, (i, pos.y))))
        .collect();
    let port_in_block = |id: Uuid, port_type: PortType, port: &str| {
        let (line, y) = *line_and_y.get(&id)?;
        let node = nodes.get(&id)?;
        Some((
            line,
            y + node.abs_port_position(port_type, port).y - node.pos().y,
        ))
    };
    let mut gaps_below = vec![LAYOUT_LINE_GAP; lines.len()];
    for edge in edges {
        if let Some((from, output_y)) =
            port_in_block(edge.src_uuid(), PortType::Output, edge.src_port())
            && let Some((to, input_y)) =
                port_in_block(edge.target_uuid(), PortType::Input, edge.target_port())
            && to == from + 1
        {
            let above_bottom = extents[from].1 - output_y;
            let below_top = input_y - extents[to].0;
            let needed =
                2.0f64.mul_add(EDGE_DISTANCE_FIELD_HEIGHT, (above_bottom - below_top).abs());
            gaps_below[from] = gaps_below[from].max(needed);
        }
    }

    let mut top = top;
    for ((line, (line_top, line_bottom)), gap_below) in
        lines.into_iter().zip(extents).zip(gaps_below)
    {
        for (id, pos) in line {
            positions.insert(id, Point2D::new(pos.x, pos.y - line_top + top));
        }
        top += line_bottom - line_top + gap_below;
    }
    top
}

/// The zoom, up to [`MAX_ZOOM`], that Zoom to Fit will choose for a layout in `view`.
///
/// # Arguments
///
/// * `positions` - the layout.
/// * `nodes` - the nodes it places.
/// * `edges` - the connections between them.
/// * `mapped_ports` - the ports of the group, mapped to ports of `nodes`.
/// * `view` - the size of the canvas in pixels.
///
/// # Returns
///
/// The zoom; zero for an empty view.
fn fit_zoom(
    positions: &HashMap<Uuid, Point2D<f64>>,
    nodes: &HashMap<Uuid, NodeElement>,
    edges: &[ConnectInfo],
    mapped_ports: &PortMap,
    view: Size2D<f64>,
) -> f64 {
    let placed: HashMap<Uuid, NodeElement> = positions
        .iter()
        .filter_map(|(id, pos)| {
            let mut node = nodes.get(id)?.clone();
            node.set_pos(*pos);
            Some((*id, node))
        })
        .collect();
    let covered = covered_box(&placed, edges, mapped_ports);
    (VIEW_PADDING_FACTOR * view.width / covered.width())
        .min(VIEW_PADDING_FACTOR * view.height / covered.height())
        .min(MAX_ZOOM)
}

/// The height of node `id` on the canvas, or zero if `nodes` does not hold it.
fn node_height(nodes: &HashMap<Uuid, NodeElement>, id: Uuid) -> f64 {
    nodes.get(&id).map_or(0.0, NodeElement::total_height)
}

/// Lays out the connected parts of the graph with Sugiyama and orders the nodes of each layer by the
/// height of the ports that feed them (barycenter).
///
/// # Arguments
///
/// * `nodes` - the nodes, for their port positions.
/// * `edges` - the connections; every node they name is placed.
///
/// # Returns
///
/// One block per connected part, each node with its position within its block.
fn layout_sugiyama_connected(
    nodes: &HashMap<Uuid, NodeElement>,
    edges: &[ConnectInfo],
) -> Vec<Vec<(Uuid, Point2D<f64>)>> {
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
    let mut blocks = Vec::new();

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
        let mut block = Vec::new();

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
                let final_y = assigned_y_val * SUGIYAMA_VERT_PATH_FACTOR;

                let final_pos = Point2D::new(final_x, final_y);
                subgraph_positions.insert(uuid, final_pos);
                block.push((uuid, final_pos));
            }
        }
        blocks.push(block);
    }
    blocks
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

#[cfg(test)]
mod tests {
    use super::*;
    use opossum_core::analyzers::{AnalyzerType, EnergyConfig};

    /// An optical node `id` with one input and one output port.
    fn optical(id: u128) -> (Uuid, NodeElement) {
        let uuid = Uuid::from_u128(id);
        let ports = Ports::new(vec!["input_1".to_string()], vec!["output_1".to_string()]);
        let node = NodeElement::new(
            format!("node {id}"),
            NodeType::Optical("dummy".to_string()),
            uuid,
            Point2D::zero(),
            ports,
            false,
            0,
        );
        (uuid, node)
    }

    /// An energy analyzer `id`.
    fn analyzer(id: u128) -> (Uuid, NodeElement) {
        let uuid = Uuid::from_u128(id);
        let analyzer_type = AnalyzerType::Energy(EnergyConfig::default());
        let node = NodeElement::new(
            String::new(),
            NodeType::Analyzer(Box::new(analyzer_type)),
            uuid,
            Point2D::zero(),
            Ports::default(),
            false,
            0,
        );
        (uuid, node)
    }

    /// The connection from the output of `src` to the input of `target`.
    fn connect(src: Uuid, target: Uuid) -> ConnectInfo {
        ConnectInfo::new(
            src,
            "output_1".to_string(),
            target,
            "input_1".to_string(),
            0.0,
            false,
        )
    }

    /// The box node `id` covers at the position the layout gave it.
    fn placed_box(
        nodes: &HashMap<Uuid, NodeElement>,
        positions: &HashMap<Uuid, Point2D<f64>>,
        id: Uuid,
    ) -> Rect<f64> {
        let mut node = nodes[&id].clone();
        node.set_pos(positions[&id]);
        node.get_bounding_box()
    }

    #[test]
    fn chain_lies_in_one_row_from_left_to_right() {
        let nodes: HashMap<Uuid, NodeElement> = (1..=3).map(optical).collect();
        let [a, b, c] = [1, 2, 3].map(Uuid::from_u128);

        let positions = optimize_layout(
            &nodes,
            &[connect(a, b), connect(b, c)],
            &PortMap::default(),
            Size2D::zero(),
        );

        let [pa, pb, pc] = [a, b, c].map(|id| positions[&id]);
        assert!((pb.x - pa.x - SUGIYAMA_VERTEX_SPACING).abs() < 1e-9);
        assert!((pc.x - pb.x - SUGIYAMA_VERTEX_SPACING).abs() < 1e-9);
        assert!((pb.y - pa.y).abs() < 1e-9);
        assert!((pc.y - pa.y).abs() < 1e-9);
    }

    #[test]
    fn analyzers_lie_above_unconnected_nodes_which_lie_above_connected_ones() {
        let nodes: HashMap<Uuid, NodeElement> = [analyzer(1), optical(2), optical(3), optical(4)]
            .into_iter()
            .collect();
        let [energy, lone, a, b] = [1, 2, 3, 4].map(Uuid::from_u128);

        let positions = optimize_layout(
            &nodes,
            &[connect(a, b)],
            &PortMap::default(),
            Size2D::zero(),
        );

        let placed = |id| placed_box(&nodes, &positions, id);
        assert!(placed(energy).max_y() <= placed(lone).min_y());
        assert!(placed(lone).max_y() <= placed(a).min_y());
        assert!(placed(lone).max_y() <= placed(b).min_y());
    }

    #[test]
    fn separate_chains_are_stacked_without_overlap() {
        let nodes: HashMap<Uuid, NodeElement> = (1..=4).map(optical).collect();
        let [a, b, c, d] = [1, 2, 3, 4].map(Uuid::from_u128);

        let positions = optimize_layout(
            &nodes,
            &[connect(a, b), connect(c, d)],
            &PortMap::default(),
            Size2D::zero(),
        );

        let (upper, lower) = if positions[&a].y < positions[&c].y {
            ([a, b], [c, d])
        } else {
            ([c, d], [a, b])
        };
        let placed = |id| placed_box(&nodes, &positions, id);
        for up in upper {
            for low in lower {
                assert!(placed(up).max_y() <= placed(low).min_y());
            }
        }
    }

    /// The chain 1 → 2 → … → `n` of optical nodes.
    fn chain(n: u128) -> (HashMap<Uuid, NodeElement>, Vec<ConnectInfo>) {
        let nodes = (1..=n).map(optical).collect();
        let edges = (1..n)
            .map(|i| connect(Uuid::from_u128(i), Uuid::from_u128(i + 1)))
            .collect();
        (nodes, edges)
    }

    /// With node height H ≈ 80.3, c columns per line and L lines, a chain is (c − 1)·250 + 130 wide,
    /// 71 more once a connection runs back to the next line, and L·H + (L − 1)·100 high. Six nodes
    /// in a 1200 × 500 view: 3 columns (701 × 261, zoom 1.63) beat 4 (951 × 261, zoom 1.20),
    /// 2 (451 × 441, zoom 1.08) and 6 (1380 × 80, zoom 0.83).
    #[test]
    fn long_chain_wraps_into_the_lines_that_fit_the_view_best() {
        let (nodes, edges) = chain(6);

        let positions = optimize_layout(
            &nodes,
            &edges,
            &PortMap::default(),
            Size2D::new(1200.0, 500.0),
        );

        let pos = |i| positions[&Uuid::from_u128(i)];
        let columns = [0.0, 1.0, 2.0, 0.0, 1.0, 2.0];
        for (i, column) in (1..=6).zip(columns) {
            assert!(f64::mul_add(column, -SUGIYAMA_VERTEX_SPACING, pos(i).x).abs() < 1e-9);
        }
        for (i, first_of_line) in [(2, 1), (3, 1), (5, 4), (6, 4)] {
            assert!((pos(i).y - pos(first_of_line).y).abs() < 1e-9);
        }
        let placed = |i| placed_box(&nodes, &positions, Uuid::from_u128(i));
        assert!(placed(1).max_y() <= placed(4).min_y());
    }

    /// The same six nodes in a 3000 × 200 view: one line (1380 × 80, zoom 2.07) beats two (261 high,
    /// zoom at most 0.73).
    #[test]
    fn long_chain_stays_in_one_line_in_a_wide_flat_view() {
        let (nodes, edges) = chain(6);

        let positions = optimize_layout(
            &nodes,
            &edges,
            &PortMap::default(),
            Size2D::new(3000.0, 200.0),
        );

        let pos = |i| positions[&Uuid::from_u128(i)];
        for (i, column) in (1..=6).zip([0.0, 1.0, 2.0, 3.0, 4.0, 5.0]) {
            assert!(f64::mul_add(column, -SUGIYAMA_VERTEX_SPACING, pos(i).x).abs() < 1e-9);
            assert!((pos(i).y - pos(1).y).abs() < 1e-9);
        }
    }

    /// Two nodes in a 4000 × 4000 view fit at the largest zoom both side by side and one below the
    /// other; on that tie the longer line wins.
    #[test]
    fn small_graph_stays_in_one_line_when_every_layout_fits_at_the_largest_zoom() {
        let (nodes, edges) = chain(2);

        let positions = optimize_layout(
            &nodes,
            &edges,
            &PortMap::default(),
            Size2D::new(4000.0, 4000.0),
        );

        let [first, second] = [1, 2].map(|i| positions[&Uuid::from_u128(i)]);
        assert!(second.x > first.x);
        assert!((second.y - first.y).abs() < 1e-9);
    }

    /// Four analyzers in a 700 × 600 view: with c per line their row is (c − 1)·180 + 130 wide, so
    /// two lines of two (310 × 261, zoom 2.15) beat one line (670 × 80, zoom 0.99), three columns
    /// (490 × 261, zoom 1.36) and one column (130 × 621, zoom 0.92).
    #[test]
    fn analyzer_row_wraps_like_a_chain() {
        let nodes: HashMap<Uuid, NodeElement> = (1..=4).map(analyzer).collect();

        let positions =
            optimize_layout(&nodes, &[], &PortMap::default(), Size2D::new(700.0, 600.0));

        let pos = |i| positions[&Uuid::from_u128(i)];
        let pitch = NODE_WIDTH + LAYOUT_NODE_GAP;
        for (i, column) in (1..=4).zip([0.0, 1.0, 0.0, 1.0]) {
            assert!(f64::mul_add(column, -pitch, pos(i).x).abs() < 1e-9);
        }
        assert!((pos(2).y - pos(1).y).abs() < 1e-9);
        assert!((pos(4).y - pos(3).y).abs() < 1e-9);
        let placed = |i| placed_box(&nodes, &positions, Uuid::from_u128(i));
        assert!(placed(1).max_y() <= placed(3).min_y());
    }

    /// A splitter feeding two nodes in a 200 × 1000 view: one column per line wins (201 wide with the
    /// connections running back, against 380). The two fed nodes share a column, so they move to the
    /// next line together.
    #[test]
    fn nodes_of_one_column_stay_in_one_line() {
        let nodes: HashMap<Uuid, NodeElement> = (1..=3).map(optical).collect();
        let [splitter, a, b] = [1, 2, 3].map(Uuid::from_u128);
        let edges = [connect(splitter, a), connect(splitter, b)];

        let positions = optimize_layout(
            &nodes,
            &edges,
            &PortMap::default(),
            Size2D::new(200.0, 1000.0),
        );

        assert!(positions[&a].x.abs() < 1e-9);
        assert!(positions[&b].x.abs() < 1e-9);
        assert!((positions[&a].y - positions[&b].y).abs() > 1.0);
        let placed = |id| placed_box(&nodes, &positions, id);
        assert!(placed(splitter).max_y() <= placed(a).min_y().min(placed(b).min_y()));
    }

    /// Optical node `id`, placed at (`x`, `y`).
    fn optical_at(id: u128, x: f64, y: f64) -> (Uuid, NodeElement) {
        let (uuid, mut node) = optical(id);
        node.set_pos(Point2D::new(x, y));
        (uuid, node)
    }

    #[test]
    fn covered_box_spans_all_nodes() {
        let nodes: HashMap<Uuid, NodeElement> =
            [optical_at(1, 0.0, 0.0), optical_at(2, 300.0, 100.0)]
                .into_iter()
                .collect();
        let height = nodes[&Uuid::from_u128(2)].total_height();

        let covered = covered_box(&nodes, &[], &PortMap::default());

        assert!(covered.min_x().abs() < 1e-9);
        assert!(covered.min_y().abs() < 1e-9);
        assert!((covered.max_x() - (300.0 + NODE_WIDTH)).abs() < 1e-9);
        assert!((covered.max_y() - (100.0 + height)).abs() < 1e-9);
    }

    #[test]
    fn covered_box_holds_the_tags_of_mapped_ports() {
        use crate::components::scenery_editor::constants::PORT_MAP_OVERHANG;
        let nodes: HashMap<Uuid, NodeElement> =
            [optical_at(1, 0.0, 0.0), optical_at(2, 300.0, 0.0)]
                .into_iter()
                .collect();
        let mut mapped_ports = PortMap::default();
        mapped_ports
            .add("input_1", Uuid::from_u128(1), "input_1")
            .unwrap();
        mapped_ports
            .add("output_1", Uuid::from_u128(2), "output_1")
            .unwrap();

        let covered = covered_box(&nodes, &[], &mapped_ports);

        assert!((covered.min_x() + PORT_MAP_OVERHANG).abs() < 1e-9);
        assert!((covered.max_x() - (300.0 + NODE_WIDTH + PORT_MAP_OVERHANG)).abs() < 1e-9);
    }

    #[test]
    fn covered_box_of_no_nodes_is_empty_at_the_origin() {
        let covered = covered_box(&HashMap::new(), &[], &PortMap::default());

        assert!(covered.is_empty());
        assert!(covered.origin.x.abs() < 1e-9 && covered.origin.y.abs() < 1e-9);
    }

    /// A chain wrapped after its second node: the connection from the end of the first line back to
    /// the start of the second reaches beyond both, and the box has to hold it.
    #[test]
    fn covered_box_holds_a_connection_running_back_to_the_next_line() {
        let nodes: HashMap<Uuid, NodeElement> = [
            optical_at(1, 0.0, 0.0),
            optical_at(2, 250.0, 0.0),
            optical_at(3, 0.0, 180.0),
        ]
        .into_iter()
        .collect();
        let [a, b, c] = [1, 2, 3].map(Uuid::from_u128);
        let edges = [connect(a, b), connect(b, c)];

        let covered = covered_box(&nodes, &edges, &PortMap::default());

        let wrap = edge_bounds(
            nodes[&b].abs_port_position(PortType::Output, "output_1"),
            nodes[&c].abs_port_position(PortType::Input, "input_1"),
        );
        assert!(covered.contains_rect(&wrap), "{covered:?} misses {wrap:?}");
    }

    /// A connection running back between two ports at one height has a box without height, which
    /// still counts.
    #[test]
    fn covered_box_holds_a_connection_back_at_one_height() {
        let nodes: HashMap<Uuid, NodeElement> =
            [optical_at(1, 300.0, 0.0), optical_at(2, 0.0, 0.0)]
                .into_iter()
                .collect();
        let [a, b] = [1, 2].map(Uuid::from_u128);

        let covered = covered_box(&nodes, &[connect(a, b)], &PortMap::default());

        let back = edge_bounds(
            nodes[&a].abs_port_position(PortType::Output, "output_1"),
            nodes[&b].abs_port_position(PortType::Input, "input_1"),
        );
        assert!(covered.min_x() <= back.min_x() && back.max_x() <= covered.max_x());
    }

    /// A beam splitter `id` with one input and two output ports.
    fn splitter(id: u128) -> (Uuid, NodeElement) {
        let (uuid, mut node) = optical(id);
        node.set_ports(
            vec!["input_1".to_string()],
            vec!["output_1".to_string(), "output_2".to_string()],
        );
        (uuid, node)
    }

    /// A splitter 1 with two parallel arms 2 → 3 → 4 and 5 → 6 → 7, in a tall narrow view that breaks
    /// the arms into lines of two rows each. With the gap fixed at 100, the lower arm would run back
    /// from one line to the next at about y = 333, through the upper arm's node at 305–385.
    #[test]
    fn connections_to_the_next_line_run_back_clear_of_every_node() {
        let mut nodes: HashMap<Uuid, NodeElement> = (2..=7).map(optical).collect();
        let (s, splitter_node) = splitter(1);
        nodes.insert(s, splitter_node);
        let [a1, a2, a3, b1, b2, b3] = [2, 3, 4, 5, 6, 7].map(Uuid::from_u128);
        let lower_arm = ConnectInfo::new(
            s,
            "output_2".to_string(),
            b1,
            "input_1".to_string(),
            0.0,
            false,
        );
        let edges = [
            connect(s, a1),
            lower_arm,
            connect(a1, a2),
            connect(a2, a3),
            connect(b1, b2),
            connect(b2, b3),
        ];

        let positions = optimize_layout(
            &nodes,
            &edges,
            &PortMap::default(),
            Size2D::new(400.0, 2000.0),
        );

        let placed: HashMap<Uuid, NodeElement> = nodes
            .iter()
            .map(|(id, node)| {
                let mut node = node.clone();
                node.set_pos(positions[id]);
                (*id, node)
            })
            .collect();
        let mut running_back = 0;
        for edge in &edges {
            let start =
                placed[&edge.src_uuid()].abs_port_position(PortType::Output, edge.src_port());
            let end =
                placed[&edge.target_uuid()].abs_port_position(PortType::Input, edge.target_port());
            if end.x >= start.x {
                continue;
            }
            running_back += 1;
            let reach = edge_bounds(start, end);
            let field = Rect::new(
                Point2D::new(
                    reach.min_x(),
                    f64::midpoint(start.y, end.y) - EDGE_DISTANCE_FIELD_HEIGHT / 2.0,
                ),
                Size2D::new(reach.width(), EDGE_DISTANCE_FIELD_HEIGHT),
            );
            for (id, node) in &placed {
                let node_box = node.get_bounding_box();
                assert!(
                    !node_box.intersects(&field),
                    "{} → {} runs back through node {id}",
                    edge.src_uuid(),
                    edge.target_uuid()
                );
            }
        }
        assert!(running_back > 0, "the view breaks no line");
    }
}
