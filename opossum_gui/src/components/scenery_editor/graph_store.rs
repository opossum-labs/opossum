use super::{
    node::{NodeElement, NodeType},
    ports::ports_component::Ports,
};
use crate::{
    OPOSSUM_UI_LOGS,
    api::{self, eval_action_run},
    components::scenery_editor::{
        constants::{
            HEADER_HEIGHT, NODE_WIDTH, SUGIYAMA_VERT_PATH_FACTOR, SUGIYAMA_VERTEX_SPACING,
        },
        graph_editor::graph_editor_component::EditorState,
    },
};
use dioxus::{
    html::geometry::euclid::{
        Size2D,
        default::{Point2D, Rect},
    },
    prelude::*,
};
use futures_util::StreamExt;
use opossum_backend::{
    AnalyzerInfo,
    nodes::{ConnectInfo, NewNode, NewRefNode, NodeInfo},
    scenery::NewAnalyzerInfo,
    to_f64,
};
use rust_sugiyama::{configure::RankingType, from_edges};
use std::{collections::HashMap, fs, path::PathBuf};
use uuid::Uuid;

#[derive(Clone, Copy, Eq, PartialEq, Default)]
pub struct GraphState {
    pub graph_store: Signal<GraphStore>,
    pub editor_state: Signal<EditorState>,
}

#[derive(Clone, Copy, Eq, PartialEq, Default)]
pub struct GraphStore {
    nodes: Signal<HashMap<Uuid, NodeElement>>,
    edges: Signal<Vec<ConnectInfo>>,
    active_node: Signal<Option<Uuid>>,
    scenery_id: Uuid,
}
pub enum GraphStoreAction {
    LoadFromFile(PathBuf),
    SaveToFile(PathBuf),
    AddOpticNode(String),
    AddOpticReference(NewRefNode),
    AddAnalyzer(NewAnalyzerInfo),
    SyncNodePosition(Uuid, Point2D<f64>),
    AddEdge(ConnectInfo),
    UpdateEdge(ConnectInfo),
    UpdateEdges(Vec<ConnectInfo>),
    DeleteEdge(ConnectInfo),
    DeleteNode(Uuid),
    CopyNode((NodeType, Uuid, Point2D<f64>)),
    GetSceneryId,
    DeleteScenery,
    OptimizeLayout,
    // TerminateBackend,
    UpdateActiveNode(Option<NodeElement>),
}
impl GraphStore {
    pub const fn set_scenery_id(&mut self, id: Uuid) {
        self.scenery_id = id;
    }
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

    #[must_use]
    pub const fn nodes(&self) -> Signal<HashMap<Uuid, NodeElement>> {
        self.nodes
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
            .map(super::node::NodeElement::node_type)
            .cloned()
    }
    #[must_use]
    pub const fn edges_mut(&mut self) -> &mut Signal<Vec<ConnectInfo>> {
        &mut self.edges
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
    pub fn active_node(&self) -> Option<Uuid> {
        *self.active_node.read()
    }
    pub fn set_node_active(&mut self, id: Uuid, z_index: usize) {
        self.set_z_level_to_top(id, z_index);
        let mut active_node = self.active_node.write();
        *active_node = Some(id);
    }
    pub fn set_active_node_none(&mut self) {
        let mut active_node = self.active_node.write();
        *active_node = None;
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
        let first_node = optic_nodes.iter().next().unwrap().1;
        let mut min_x = first_node.pos().x;
        let mut min_y = first_node.pos().y;
        let mut max_x = first_node.pos().x + NODE_WIDTH;
        let mut max_y = first_node.pos().y + HEADER_HEIGHT + first_node.node_body_height();

        // Iterate over the rest of the nodes to expand the bounding box
        for node in optic_nodes.iter().skip(1) {
            let node_pos = node.1.pos();
            min_x = min_x.min(node_pos.x);
            min_y = min_y.min(node_pos.y);
            max_x = max_x.max(node_pos.x + NODE_WIDTH);
            max_y = max_y.max(node_pos.y + HEADER_HEIGHT + node.1.node_body_height());
        }
        Rect::new(
            Point2D::new(min_x, min_y),
            Size2D::new(max_x - min_x, max_y - min_y),
        )
    }
    pub fn clear(&mut self) {
        self.nodes().write().clear();
        self.edges().write().clear();
        let mut active_node = self.active_node.write();
        *active_node = None;
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
    pub fn set_z_level_to_top(&mut self, node_id: Uuid, z_level: usize){
        let number_of_nodes = self.nodes().read().len();
        let mut nodes = self.nodes.write();
        for (id, elem) in nodes.iter_mut() {
            let z_index = elem.z_index();
            if z_index > z_level && *id != node_id {
                elem.set_z_index(z_index-1);
            }
            else if *id == node_id{
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
        self.set_node_active(id, node_element.z_index());
        node_element
    }

    /// Removes nodes by their IDs from the graph store.
    /// This function iterates through the provided list of node IDs,
    /// removes each node from the store, and updates the edges accordingly.
    /// # Arguments:
    /// * `deleted_ids`: A vector of `Uuid` representing the IDs of the nodes to be removed.
    pub fn remove_nodes_by_id(&mut self, node_ids: Vec<Uuid>) {
        for node_id in node_ids {
            self.nodes_mut().write().remove(&node_id);
            self.renumber_z_levels();
            self.edges.with_mut(|edges| {
                edges.retain_mut(|e| e.src_uuid() != node_id && e.target_uuid() != node_id);
            });
        }
        self.set_active_node_none();
    }

    /// Adds a new optical node to the graph store.
    /// This function creates a new `NodeElement` for the optical node and inserts it into the store.
    /// It also sets the z-index based on the current number of nodes to ensure proper layering.
    /// # Arguments:
    /// * `node_info`: The `NodeInfo` containing the type and position of the new node.
    /// # Returns:
    /// A `NodeElement` representing the newly added optical node.
    pub fn add_new_optical_node(&mut self, node_info: &NodeInfo) -> NodeElement {
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
        self.set_node_active(node_info.uuid(),node_element.z_index());
        node_element
    }

    /// Adds a new analyzer to the graph store.
    /// This function creates a new `NodeElement` for the analyzer and inserts it into the store.
    /// It also sets the z-index based on the current number of nodes to ensure proper layering.
    /// # Arguments:
    /// * `new_analyzer`: The `NewAnalyzerInfo` containing the type and position of the new analyzer.
    /// * `analyzer_id`: The unique identifier for the new analyzer.
    /// # Returns:
    /// A `NodeElement` representing the newly added analyzer.
    #[must_use]
    pub fn add_new_analyzer(
        &mut self,
        new_analyzer: NewAnalyzerInfo,
        analyzer_id: Uuid,
    ) -> NodeElement {
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
        node_element
    }
}

pub async fn optimize_layout_and_sync(
    edges: Vec<ConnectInfo>,
) -> Result<HashMap<Uuid, Point2D<f64>>, String> {
    let mut reg = UuidRegistry::new();
    let edges_u32: Vec<(u32, u32)> = edges
        .iter()
        .map(|edge| {
            let src = reg.register(edge.src_uuid());
            let target = reg.register(edge.target_uuid());
            (src, target)
        })
        .collect();

    let layouts = from_edges(&edges_u32)
        .vertex_spacing(SUGIYAMA_VERTEX_SPACING)
        .layering_type(RankingType::Original)
        .build();

    let mut new_positions = HashMap::new();
    let mut height = 0f64;
    for (layout, group_height, _) in layouts {
        for l in layout {
            if let Some(uuid) = reg.get_uuid(u32::try_from(l.0).unwrap()) {
                let pos = Point2D::new(
                    -to_f64(l.1.1),
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
pub fn use_graph_processor(
    node_selected: Signal<Option<NodeElement>>,
    mut graph_state: Signal<GraphState>,
) -> Coroutine<GraphStoreAction> {
    use_coroutine(move |mut rx: UnboundedReceiver<GraphStoreAction>| {
        let mut graph_store = graph_state.write().graph_store;
        let editor_state = graph_state.write().editor_state;
        async move {
            // This loop runs forever in the background, waiting for actions.
            while let Some(action) = rx.next().await {
                match action {
                    GraphStoreAction::UpdateEdges(connect_infos) => {
                        graph_store.write().edges.set(connect_infos.clone());
                    }
                    GraphStoreAction::UpdateActiveNode(node) => {
                        process_update_active_node(node, graph_store);
                    }
                    GraphStoreAction::LoadFromFile(path) => {
                        process_load_from_file(path, graph_store).await;
                    }
                    GraphStoreAction::SaveToFile(path) => {
                        process_save_to_file(path).await;
                    }
                    GraphStoreAction::SyncNodePosition(node_id, pos) => {
                        eval_action_run(
                            api::update_gui_position(node_id, pos).await,
                            None::<fn(String)>,
                        );
                    }
                    GraphStoreAction::DeleteNode(node_id) => {
                        process_delete_node(node_id, graph_store, node_selected).await;
                    }
                    GraphStoreAction::AddOpticNode(new_node) => {
                        process_add_optic_node(&new_node, graph_store, editor_state, node_selected)
                            .await;
                    }
                    GraphStoreAction::AddOpticReference(new_ref_node) => {
                        process_add_reference_node(new_ref_node, graph_store, node_selected).await;
                    }
                    GraphStoreAction::AddAnalyzer(new_analyzer) => {
                        process_add_analyzer(new_analyzer, graph_store, node_selected).await;
                    }
                    GraphStoreAction::AddEdge(connect_info) => {
                        process_add_edge(connect_info, graph_store).await;
                    }
                    GraphStoreAction::UpdateEdge(connect_info) => {
                        process_update_edge(connect_info, graph_store).await;
                    }
                    GraphStoreAction::DeleteEdge(connect_info) => {
                        process_delete_edge(connect_info, graph_store).await;
                    }
                    GraphStoreAction::CopyNode((node_type, node_id, pos)) => {
                        process_copy_node(node_type, node_id, pos, graph_store).await;
                    }
                    GraphStoreAction::DeleteScenery => {
                        process_delete_scenery(graph_store).await;
                    }
                    GraphStoreAction::OptimizeLayout => {
                        process_optimize_layout(graph_store).await;
                    }
                    GraphStoreAction::GetSceneryId => eval_action_run(
                        api::get_scenery_uuid().await,
                        Some(move |id| graph_store.write().set_scenery_id(id)),
                    ),
                }
            }
        }
    })
}
// this flag is set because clippy expects Signal<GraphStore> to be Send.
// However graphstore is only used locally and not within another async thread
#[allow(clippy::future_not_send)]
async fn process_load_from_file(path: PathBuf, mut graph_store: Signal<GraphStore>) {
    let opm_string = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            OPOSSUM_UI_LOGS.write().add_log(&e.to_string());
            return;
        }
    };
    graph_store.write().clear();
    eval_action_run(api::post_opm_file(opm_string).await, None::<fn(String)>);
    eval_action_run(
        api::get_scenery_uuid().await,
        Some(move |id| graph_store.write().set_scenery_id(id)),
    );
    let scenery_id = graph_store.peek().scenery_id;

    eval_action_run(
        api::get_nodes(scenery_id).await,
        Some(move |nodes: Vec<NodeInfo>| graph_store.write().add_nodes(&nodes)),
    );
    eval_action_run(
        api::get_analyzers().await,
        Some(move |analyzers: Vec<AnalyzerInfo>| {
            graph_store.write().add_analyzers(&analyzers);
        }),
    );
    eval_action_run(
        api::get_connections(scenery_id).await,
        Some(move |connect_infos: Vec<ConnectInfo>| {
            graph_store.write().edges.set(connect_infos);
        }),
    );
}

fn process_update_active_node(node: Option<NodeElement>, mut graph_store: Signal<GraphStore>) {
    if let Some(node) = node {
        if let Some(active_node) = graph_store.write().nodes_mut().write().get_mut(&node.id()) {
            *active_node = node.clone();
        }
        graph_store.write().set_node_active(node.id(), node.z_index());
    } else {
        graph_store.write().set_active_node_none();
    }
}

async fn process_save_to_file(path: PathBuf) {
    eval_action_run(
        api::get_opm_file().await,
        Some(move |opm_string| {
            if let Err(err_str) = fs::write(path, opm_string) {
                OPOSSUM_UI_LOGS.write().add_log(&err_str.to_string());
            }
        }),
    );
}

#[allow(clippy::future_not_send)]
async fn process_delete_node(
    node_id: Uuid,
    graph_store: Signal<GraphStore>,
    node_selected: Signal<Option<NodeElement>>,
) {
    let node_type_opt = graph_store.read().get_node_type(node_id);
    if let Some(node_type) = node_type_opt {
        match node_type {
            NodeType::Optical(_) => {
                process_delete_optical_node(node_id, graph_store, node_selected).await;
            }
            NodeType::Analyzer(_) => {
                process_delete_analyzer_node(node_id, graph_store, node_selected).await;
            }
        }
    } else {
        OPOSSUM_UI_LOGS
            .write()
            .add_log("Node could not be deleted, as uuid was not found");
    }
}

#[allow(clippy::future_not_send)]
async fn process_delete_analyzer_node(
    analyzer_id: Uuid,
    mut graph_store: Signal<GraphStore>,
    mut node_selected: Signal<Option<NodeElement>>,
) {
    eval_action_run(
        api::delete_analyzer(analyzer_id).await,
        Some(move |deleted_id| {
            graph_store.write().remove_nodes_by_id(vec![deleted_id]);
            node_selected.set(None);
        }),
    );
}

#[allow(clippy::future_not_send)]
async fn process_delete_optical_node(
    node_id: Uuid,
    mut graph_store: Signal<GraphStore>,
    mut node_selected: Signal<Option<NodeElement>>,
) {
    eval_action_run(
        api::delete_node(node_id).await,
        Some(move |deleted_ids| {
            graph_store.write().remove_nodes_by_id(deleted_ids);
            node_selected.set(None);
        }),
    );
}

#[allow(clippy::future_not_send)]
async fn process_add_optic_node(
    new_node_type_string: &str,
    mut graph_store: Signal<GraphStore>,
    editor_state: Signal<EditorState>,
    mut node_selected: Signal<Option<NodeElement>>,
) {
    // calculate center of viewport (in graph coordinates)
    let zoom = *editor_state.peek().zoom.peek();
    let view_port_center = editor_state.peek().get_view_port_center();
    let shift = *editor_state.peek().shift.peek();

    let element_position = (
        (view_port_center.x - shift.x) / zoom,
        (view_port_center.y - shift.y) / zoom,
    );
    let new_node_info = NewNode::new(new_node_type_string.to_lowercase(), element_position);
    let scenery_id = graph_store.peek().scenery_id;
    eval_action_run(
        api::post_add_node(new_node_info, scenery_id).await,
        Some(move |node_info| {
            let node_element = graph_store.write().add_new_optical_node(&node_info);
            node_selected.set(Some(node_element));
        }),
    );
}

#[allow(clippy::future_not_send)]
async fn process_add_reference_node(
    new_ref_node: NewRefNode,
    mut graph_store: Signal<GraphStore>,
    mut node_selected: Signal<Option<NodeElement>>,
) {
    let scenery_id = graph_store.peek().scenery_id;
    eval_action_run(
        api::post_add_ref_node(new_ref_node, scenery_id).await,
        Some(move |node_info| {
            let node_element = graph_store.write().add_new_reference_node(&node_info);
            node_selected.set(Some(node_element));
        }),
    );
}

#[allow(clippy::future_not_send)]
async fn process_add_analyzer(
    new_analyzer: NewAnalyzerInfo,
    mut graph_store: Signal<GraphStore>,
    mut node_selected: Signal<Option<NodeElement>>,
) {
    eval_action_run(
        api::post_add_analyzer(new_analyzer.clone()).await,
        Some(move |analyzer_id| {
            let node_element = graph_store
                .write()
                .add_new_analyzer(new_analyzer, analyzer_id);
            node_selected.set(Some(node_element));
        }),
    );
}

#[allow(clippy::future_not_send)]
async fn process_update_edge(connect_info: ConnectInfo, mut graph_store: Signal<GraphStore>) {
    eval_action_run(
        api::update_distance(connect_info).await,
        Some(move |ci: ConnectInfo| {
            if let Some(e) = graph_store
                .write()
                .edges
                .write()
                .iter_mut()
                .find(|e| e.src_uuid() == ci.src_uuid() && e.target_uuid() == ci.target_uuid())
            {
                *e = ci;
            }
        }),
    );
}

#[allow(clippy::future_not_send)]
async fn process_delete_edge(connect_info: ConnectInfo, mut graph_store: Signal<GraphStore>) {
    eval_action_run(
        api::delete_connection(connect_info).await,
        Some(move |ci| graph_store.write().edges.write().retain(|e| e != &ci)),
    );
}

#[allow(clippy::future_not_send)]
async fn process_copy_node(
    node_type: NodeType,
    node_id: Uuid,
    pos: Point2D<f64>,
    mut graph_store: Signal<GraphStore>,
) {
    match node_type {
        NodeType::Optical(_) => eval_action_run(
            api::post_copy_optical_node(node_id, pos).await,
            Some(move |node_info| {
                let _ = graph_store.write().add_new_optical_node(&node_info);
            }),
        ),
        NodeType::Analyzer(_) => eval_action_run(
            api::post_copy_analyzer_node(node_id, pos).await,
            Some(move |analyzer_info: AnalyzerInfo| {
                let id = analyzer_info.id();
                let _ = graph_store
                    .write()
                    .add_new_analyzer(NewAnalyzerInfo::from(analyzer_info), id);
            }),
        ),
    }
}

#[allow(clippy::future_not_send)]
async fn process_add_edge(connect_info: ConnectInfo, mut graph_store: Signal<GraphStore>) {
    eval_action_run(
        api::post_add_connection(connect_info).await,
        Some(move |ci| {
            graph_store.write().edges_mut().write().push(ci);
        }),
    );
}

#[allow(clippy::future_not_send)]
async fn process_optimize_layout(mut graph_store: Signal<GraphStore>) {
    let edges = graph_store.read().edges().read().clone();
    eval_action_run(
        optimize_layout_and_sync(edges).await,
        Some(move |new_positions| {
            graph_store.write().update_node_positions(new_positions);
        }),
    );
}
#[allow(clippy::future_not_send)]
async fn process_delete_scenery(mut graph_store: Signal<GraphStore>) {
    eval_action_run(
        api::delete_scenery().await,
        Some(move |_| graph_store.write().clear()),
    );
}
