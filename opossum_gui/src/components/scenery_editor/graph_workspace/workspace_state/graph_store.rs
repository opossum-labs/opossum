use crate::{
    OPOSSUM_UI_LOGS, api,
    components::scenery_editor::{
        NodeElement, NodeType, SelectedNode,
        constants::{SUGIYAMA_VERT_PATH_FACTOR, SUGIYAMA_VERTEX_SPACING},
        graph_workspace::EditorState,
        ports::ports_component::Ports,
    },
};
use dioxus::{
    html::geometry::euclid::default::{Point2D, Rect, Size2D},
    prelude::*,
};
use opossum_core::{
    opm_document::AnalyzerInfo,
    prelude::{PortMap, PortType},
    types::api_types::{ConnectInfo, NewAnalyzerInfo, NodeInfo},
    utils::to_f64,
};
use rust_sugiyama::{configure::Config, from_edges};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Eq, PartialEq, Default)]
pub struct GraphState {
    pub graph_store: Signal<GraphStore>,
    pub editor_state: Signal<EditorState>,
    pub graph_info: GraphInfo,
}

impl GraphInfo {
    pub fn get_parent_id(&self) -> Option<Uuid> {
        let parent_hierarchy_pos = self.hierarchy.len() - 2;
        if parent_hierarchy_pos > 0 {
            Some(self.hierarchy[parent_hierarchy_pos].0)
        } else {
            None
        }
    }
    pub fn get_parent(&self) -> Option<(Uuid, String)> {
        let parent_hierarchy_pos = self.hierarchy.len() - 2;
        if parent_hierarchy_pos > 0 {
            Some(self.hierarchy[parent_hierarchy_pos].clone())
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

#[derive(Clone, Copy, Eq, PartialEq, Default)]
pub struct GraphStore {
    nodes: Signal<HashMap<Uuid, NodeElement>>,
    pub edges: Signal<Vec<ConnectInfo>>,
    pub node_selection: Signal<NodeSelection>,
    pub mapped_ports: Signal<PortMap>,
}

impl GraphStore {
    pub fn add_nodes(&mut self, nodes: &[NodeInfo]) {
        self.nodes
            .write()
            .extend(nodes.iter().map(|node| (node.uuid(), node.into())));
    }
    pub fn add_analyzers(&mut self, analyzers: &[AnalyzerInfo]) {
        self.nodes
            .write()
            .extend(analyzers.iter().map(|node| (node.id(), node.into())));
    }
    pub fn set_name_of_node(&mut self, node_id: Uuid, name: String) {
        if let Some(node) = self.nodes_mut().write().get_mut(&node_id) {
            node.set_name(name);
        }
    }
    pub fn remove_port_of_node(
        &mut self,
        node_id: Uuid,
        remove_port: &String,
        port_type: PortType,
    ) {
        if let Some(node) = self.nodes_mut().write().get_mut(&node_id) {
            node.remove_port(remove_port, port_type);
        }
    }
    pub fn update_ports_of_node(
        &mut self,
        node_id: Uuid,
        input_ports: Vec<String>,
        output_ports: Vec<String>,
    ) {
        if let Some(node) = self.nodes_mut().write().get_mut(&node_id) {
            node.set_ports(input_ports, output_ports);
        }
    }
    pub fn set_node_inverted(&mut self, node_id: Uuid, inverted: bool) {
        if let Some(node) = self.nodes_mut().write().get_mut(&node_id) {
            node.set_inverted(inverted);
        }
    }
    #[must_use]
    pub fn nodes(&self) -> ReadSignal<HashMap<Uuid, NodeElement>> {
        self.nodes.into()
    }
    #[must_use]
    pub const fn edges(&self) -> Signal<Vec<ConnectInfo>> {
        self.edges
    }
    #[must_use]
    pub fn get_node_type(&self, node_id: Uuid) -> Option<NodeType> {
        self.nodes
            .read()
            .get(&node_id)
            .map(NodeElement::node_type)
            .cloned()
    }
    pub const fn nodes_mut(&mut self) -> &mut Signal<HashMap<Uuid, NodeElement>> {
        &mut self.nodes
    }
    pub fn shift_node_position(&mut self, node_id: Uuid, shift: Point2D<f64>) {
        if let Some(node) = self.nodes_mut().write().get_mut(&node_id) {
            node.shift_position(shift);
        }
    }
    #[must_use]
    pub fn selected_nodes(&self) -> HashMap<Uuid, bool> {
        self.node_selection.read().all_nodes.read().clone()
    }
    #[must_use]
    pub fn selected_optical_nodes(&self) -> HashSet<Uuid> {
        self.node_selection
            .read()
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
            .read()
            .all_nodes
            .read()
            .keys()
            .copied()
            .collect()
    }
    pub fn clear_selected_nodes(&mut self) {
        self.node_selection.write().all_nodes.write().clear();
    }
    pub fn get_selected_nodes(&self, graph_id: Uuid) -> Vec<SelectedNode> {
        let mut selected_nodes = Vec::<SelectedNode>::new();
        for n_id in &self.selected_node_ids() {
            if let Some(n) = self.nodes().read().get(n_id) {
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
    pub fn set_node_active(&mut self, id: Uuid, z_index: usize, is_optical: bool) {
        self.set_z_level_to_top(id, z_index);
        self.clear_selected_nodes();
        self.node_selection
            .write()
            .all_nodes
            .write()
            .insert(id, is_optical);
    }
    pub fn add_to_node_selection(&mut self, id: Uuid, is_optical: bool) {
        self.node_selection
            .write()
            .all_nodes
            .write()
            .insert(id, is_optical);
    }
    pub fn remove_from_node_selection(&mut self, id: Uuid) {
        self.node_selection.write().all_nodes.write().remove(&id);
    }

    pub fn set_active_node_none(&mut self) {
        self.clear_selected_nodes();
    }
    pub fn update_node_positions(&mut self, new_positions: HashMap<Uuid, Point2D<f64>>) {
        let mut nodes = self.nodes.write();
        for (id, pos) in new_positions {
            if let Some(node) = nodes.get_mut(&id) {
                node.set_pos(pos);
            }
        }
    }
    pub fn get_bounding_box(&self) -> Rect<f64> {
        let optic_nodes = self.nodes()();
        if optic_nodes.is_empty() {
            return Rect::new(Point2D::zero(), Size2D::zero());
        }
        // Use the first node to initialize the bounding box
        let mut rect = optic_nodes.iter().next().unwrap().1.get_bounding_box();

        // Iterate over the rest of the nodes to expand the bounding box
        for node in optic_nodes.iter().skip(1) {
            rect = rect.union(&node.1.get_bounding_box());
        }
        rect
    }
    pub fn renumber_z_levels(&mut self) {
        let mut node_elements: Vec<(Uuid, usize)> = self
            .nodes
            .read()
            .iter()
            .map(|n| (n.1.id(), n.1.z_index()))
            .collect();
        node_elements.sort_by(|e_1, e_2| e_1.1.cmp(&e_2.1));
        let mut nodes = self.nodes.write();
        for element in node_elements.iter().enumerate() {
            if let Some(node) = nodes.get_mut(&element.1.0) {
                node.set_z_index(element.0);
            }
        }
    }
    pub fn set_z_level_to_top(&mut self, node_id: Uuid, z_level: usize) {
        let number_of_nodes = self.nodes().read().len();
        let mut nodes = self.nodes.write();
        for (id, elem) in nodes.iter_mut() {
            let z_index = elem.z_index();
            if z_index > z_level && *id != node_id {
                elem.set_z_index(z_index - 1);
            } else if *id == node_id {
                elem.set_z_index(number_of_nodes);
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
    pub fn add_new_reference_node(&mut self, ref_node_info: &NodeInfo) -> NodeElement {
        let gui_position = ref_node_info.gui_position().unwrap_or((100.0, 100.0));
        let ports = Ports::new(ref_node_info.input_ports(), ref_node_info.output_ports());
        let mut node_element = NodeElement::new(
            ref_node_info.name().to_string(),
            NodeType::Optical(ref_node_info.node_type().to_string()),
            ref_node_info.uuid(),
            Point2D::new(gui_position.0, gui_position.1),
            ports,
            ref_node_info.inverted(),
        );
        let id = ref_node_info.uuid();
        let nr_of_nodes = self.nodes().read().len();
        node_element.set_z_index(nr_of_nodes + 1);
        self.nodes.write().insert(id, node_element.clone());
        self.set_node_active(id, node_element.z_index(), true);
        node_element
    }
    /// Removes nodes by their IDs from the graph store.
    /// This function iterates through the provided list of node IDs,
    /// removes each node from the store, and updates the edges accordingly.
    /// # Arguments:
    /// * `deleted_ids`: A vector of `Uuid` representing the IDs of the nodes to be removed.
    pub fn remove_nodes_by_id(&mut self, node_ids: &Vec<Uuid>) {
        for node_id in node_ids {
            self.nodes_mut().write().remove(node_id);
            self.renumber_z_levels();
            self.edges.with_mut(|edges| {
                edges.retain_mut(|e| e.src_uuid() != *node_id && e.target_uuid() != *node_id);
            });
        }
        self.set_active_node_none();
    }
    /// Adds a new optical node to the graph store.
    /// This function creates a new `NodeElement` for the optical node and inserts it into the store.
    /// It also sets the z-index based on the current number of nodes to ensure proper layering.
    /// # Arguments:
    /// * `node_info`: The `NodeInfo` containing the type and position of the new node.
    pub fn add_new_optical_node(&mut self, node_info: &NodeInfo) {
        let gui_position = node_info.gui_position().unwrap_or((100.0, 100.0));
        let node_element = NodeElement::new(
            node_info.name().to_string(),
            NodeType::Optical(node_info.node_type().to_string()),
            node_info.uuid(),
            Point2D::new(gui_position.0, gui_position.1),
            Ports::new(node_info.input_ports(), node_info.output_ports()),
            node_info.inverted(),
        );
        self.nodes
            .write()
            .insert(node_info.uuid(), node_element.clone());
        self.set_node_active(node_info.uuid(), node_element.z_index(), true);
    }
    /// Adds a new analyzer to the graph store.
    /// This function creates a new `NodeElement` for the analyzer and inserts it into the store.
    /// It also sets the z-index based on the current number of nodes to ensure proper layering.
    /// # Arguments:
    /// * `new_analyzer`: The `NewAnalyzerInfo` containing the type and position of the new analyzer.
    /// * `analyzer_id`: The unique identifier for the new analyzer.
    pub fn add_new_analyzer(&mut self, new_analyzer: NewAnalyzerInfo, analyzer_id: Uuid) {
        let (x, y) = new_analyzer.gui_position;
        let mut node_element = NodeElement::new(
            format!("{}", new_analyzer.analyzer_type),
            NodeType::Analyzer(new_analyzer.analyzer_type),
            analyzer_id,
            Point2D::new(x, y),
            Ports::default(),
            false,
        );
        let nr_of_nodes = self.nodes().read().len();
        node_element.set_z_index(nr_of_nodes + 1);
        self.nodes.write().insert(analyzer_id, node_element.clone());
        self.set_node_active(analyzer_id, node_element.z_index(), false);
    }
}

pub async fn optimize_layout_and_sync(
    edges: Vec<ConnectInfo>,
) -> Result<HashMap<Uuid, Point2D<f64>>, String> {
    let sugiyama_config = Config {
        vertex_spacing: SUGIYAMA_VERTEX_SPACING,
        ..Default::default()
    };
    let mut reg = UuidRegistry::new();
    let edges_u32: Vec<(u32, u32)> = edges
        .iter()
        .map(|edge| {
            let src = reg.register(edge.src_uuid());
            let target = reg.register(edge.target_uuid());
            (src, target)
        })
        .collect();

    let layouts = from_edges(&edges_u32, &sugiyama_config);
    let mut new_positions = HashMap::new();
    let mut height = 0f64;
    for (layout, group_height, _) in layouts {
        for l in layout {
            if let Some(uuid) = reg.get_uuid(u32::try_from(l.0).unwrap()) {
                let pos = Point2D::new(
                    to_f64(l.1.1),
                    SUGIYAMA_VERT_PATH_FACTOR.mul_add(to_f64(l.1.0), height),
                );
                new_positions.insert(uuid, pos);
            }
        }
        height += to_f64(group_height * SUGIYAMA_VERTEX_SPACING);
    }
    for (id, pos) in &new_positions {
        if let Err(err_str) = api::update_gui_position(*id, *pos).await {
            // If any API call fails, log it and return an error for the whole operation.
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
            return Err(format!("Failed to sync position for node {id}"));
        }
    }
    Ok(new_positions)
}

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
