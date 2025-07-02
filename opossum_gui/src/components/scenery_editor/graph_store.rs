use super::{
    node::{NodeElement, NodeType, HEADER_HEIGHT, NODE_WIDTH},
    ports::ports_component::Ports,
};
use crate::{
    api::{self},
    HTTP_API_CLIENT, OPOSSUM_UI_LOGS,
};
use dioxus::{
    html::geometry::euclid::{
        default::{Point2D, Rect},
        Size2D,
    },
    prelude::*,
};
use futures_util::StreamExt;
use opossum_backend::{
    isize_to_f64,
    nodes::{ConnectInfo, NewNode, NewRefNode},
    scenery::NewAnalyzerInfo,
    usize_to_f64, PortType,
};
use rust_sugiyama::{configure::RankingType, from_edges};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Copy, Eq, PartialEq, Default)]
pub struct GraphStore {
    nodes: Signal<HashMap<Uuid, NodeElement>>,
    edges: Signal<Vec<ConnectInfo>>,
    active_node: Signal<Option<Uuid>>,
}

pub enum GraphStoreAction {
    LoadFromFile(PathBuf),
    SaveToFile(PathBuf),
    AddOpticNode(NewNode),
    AddOpticReference(NewRefNode),
    AddAnalyzer(NewAnalyzerInfo),
    SyncNodePosition(Uuid),
    AddEdge(ConnectInfo),
    UpdateEdge(ConnectInfo),
    DeleteEdge(ConnectInfo),
    DeleteNode(Uuid),
    DeleteScenery,
    OptimizeLayout,
    UpdateActiveNode(Option<NodeElement>),
}
impl GraphStore {
    #[must_use]
    pub const fn nodes(&self) -> Signal<HashMap<Uuid, NodeElement>> {
        self.nodes
    }
    #[must_use]
    pub const fn edges(&self) -> Signal<Vec<ConnectInfo>> {
        self.edges
    }
    #[must_use]
    pub const fn edges_mut(&mut self) -> &mut Signal<Vec<ConnectInfo>> {
        &mut self.edges
    }
    pub const fn nodes_mut(&mut self) -> &mut Signal<HashMap<Uuid, NodeElement>> {
        &mut self.nodes
    }
    pub fn shift_node_position(&mut self, node_id: &Uuid, shift: Point2D<f64>) {
        if let Some(node) = self.nodes_mut().write().get_mut(node_id) {
            node.shift_position(shift);
        }
    }
    #[must_use]
    pub fn active_node(&self) -> Option<Uuid> {
        *self.active_node.read()
    }
    pub fn set_node_active(&mut self, id: Uuid) {
        let mut active_node = self.active_node.write();
        *active_node = Some(id);
    }
    pub fn set_active_node_none(&mut self) {
        let mut active_node = self.active_node.write();
        *active_node = None;
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
    // #[must_use]
    // pub fn find_position(&self) -> Point2D<f64> {
    //     let size = Point2D::new(130., 130. / 1.618_033_988_7);
    //     let mut new_pos = Point2D::new(size.x, size.x);
    //     let phi = 1. / 1.618_033_988_7;
    //     loop {
    //         let mut position_found = true;
    //         for node in self.optic_nodes().read().iter() {
    //             if (node.1.pos().x - new_pos.x).abs() < 10.
    //                 && (node.1.pos().y - new_pos.y).abs() < 10.
    //             {
    //                 position_found = false;
    //                 new_pos.x += size.x * f64::powi(phi, 3);
    //                 new_pos.y += size.y * f64::powi(phi, 3);
    //                 break;
    //             }
    //         }
    //         if position_found {
    //             break;
    //         }
    //     }
    //     new_pos
    // }
}

pub async fn save_to_opm_file(path: &Path) {
    match api::get_opm_file(&HTTP_API_CLIENT()).await {
        Ok(opm_string) => {
            if let Err(err_str) = fs::write(path, opm_string) {
                OPOSSUM_UI_LOGS.write().add_log(&err_str.to_string());
            }
        }
        Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
    }
}
async fn get_ports(node_id: Uuid) -> Ports {
    match api::get_node_properties(&HTTP_API_CLIENT(), node_id).await {
        Ok(node_attr) => {
            let input_ports = node_attr
                .ports()
                .ports(&PortType::Input)
                .keys()
                .cloned()
                .collect::<Vec<String>>();
            let output_ports = node_attr
                .ports()
                .ports(&PortType::Output)
                .keys()
                .cloned()
                .collect::<Vec<String>>();
            Ports::new(input_ports, output_ports)
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
            Ports::default()
        }
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
        .vertex_spacing(250)
        .layering_type(RankingType::Original)
        .build();

    let mut new_positions = HashMap::new();
    let mut height = 0f64;
    for (layout, group_height, _) in layouts {
        for l in layout {
            if let Some(uuid) = reg.get_uuid(u32::try_from(l.0).unwrap()) {
                let pos = Point2D::new(
                    -1.0 * isize_to_f64(l.1 .1),
                    0.7f64.mul_add(isize_to_f64(l.1 .0), height),
                );
                new_positions.insert(uuid, pos);
            }
        }
        height += usize_to_f64(group_height) * 250.0;
    }
    for (id, pos) in &new_positions {
        if let Err(err_str) = api::update_gui_position(&HTTP_API_CLIENT(), *id, *pos).await {
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
#[allow(clippy::too_many_lines)]
pub fn use_graph_processor(
    graph_store: &Signal<GraphStore>,
    mut node_selected: Signal<Option<NodeElement>>,
) -> Coroutine<GraphStoreAction> {
    let mut graph_store = *graph_store;
    use_coroutine(move |mut rx: UnboundedReceiver<GraphStoreAction>| {
        async move {
            // This loop runs forever in the background, waiting for actions.
            while let Some(action) = rx.next().await {
                match action {
                    GraphStoreAction::UpdateActiveNode(node) => {
                        if let Some(node) = node {
                            graph_store.write().set_node_active(node.id());
                            if let Some(active_node) =
                                graph_store.write().nodes_mut().write().get_mut(&node.id())
                            {
                                *active_node = node;
                            }
                        } else {
                            graph_store.write().set_active_node_none();
                        }
                    }
                    GraphStoreAction::LoadFromFile(path) => {
                        let opm_string = match fs::read_to_string(path) {
                            Ok(s) => s,
                            Err(e) => {
                                OPOSSUM_UI_LOGS.write().add_log(&e.to_string());
                                continue;
                            }
                        };
                        if api::post_opm_file(&HTTP_API_CLIENT(), opm_string)
                            .await
                            .is_err()
                        {
                            continue;
                        }
                        // Clear existing state
                        graph_store.write().nodes.set(HashMap::new());
                        graph_store.write().edges.set(Vec::new());
                        graph_store.write().active_node.set(None);
                        // Load new state
                        if let Ok(nodes) = api::get_nodes(&HTTP_API_CLIENT(), Uuid::nil()).await {
                            let node_elements = nodes.into_iter().map(|node| {
                                let position = node
                                    .gui_position()
                                    .map_or_else(Point2D::zero, |(x, y)| Point2D::new(x, y));
                                NodeElement::new(
                                    node.name().to_string(),
                                    NodeType::Optical(node.node_type().to_string()),
                                    node.uuid(),
                                    position,
                                    Ports::new(node.input_ports(), node.output_ports()),
                                )
                            });
                            graph_store
                                .write()
                                .nodes
                                .write()
                                .extend(node_elements.map(|ne| (ne.id(), ne)));
                        }
                        if let Ok(analyzers) = api::get_analyzers(&HTTP_API_CLIENT()).await {
                            let analyzer_elements = analyzers.into_iter().map(|analyzer| {
                                let position = analyzer
                                    .gui_position()
                                    .map_or_else(Point2D::zero, |p| Point2D::new(p.x, p.y));
                                NodeElement::new(
                                    format!("{}", analyzer.analyzer_type()),
                                    NodeType::Analyzer(analyzer.analyzer_type().clone()),
                                    analyzer.id(),
                                    position,
                                    Ports::default(),
                                )
                            });
                            graph_store
                                .write()
                                .nodes
                                .write()
                                .extend(analyzer_elements.map(|ne| (ne.id(), ne)));
                        }
                        if let Ok(connections) =
                            api::get_connections(&HTTP_API_CLIENT(), Uuid::nil()).await
                        {
                            graph_store.write().edges.set(connections);
                        }
                    }
                    GraphStoreAction::SaveToFile(path) => save_to_opm_file(&path).await,
                    GraphStoreAction::SyncNodePosition(node_id) => {
                        if let Some(pos) = graph_store
                            .read()
                            .nodes()
                            .read()
                            .get(&node_id)
                            .map(NodeElement::pos)
                        {
                            if let Err(e) =
                                api::update_gui_position(&HTTP_API_CLIENT(), node_id, pos).await
                            {
                                OPOSSUM_UI_LOGS.write().add_log(&e);
                            }
                        }
                    }
                    GraphStoreAction::DeleteNode(node_id) => {
                        let nodes = graph_store().nodes()();
                        if let Some(node_element) = nodes.get(&node_id) {
                            match node_element.node_type() {
                                NodeType::Optical(_) => {
                                    match api::delete_node(&HTTP_API_CLIENT(), node_id).await {
                                        Ok(deleted_ids) => {
                                            for node_id in deleted_ids {
                                                graph_store().nodes_mut().write().remove(&node_id);
                                                graph_store().edges.with_mut(|edges| {
                                                    edges.retain_mut(|e| {
                                                        e.src_uuid() != node_id
                                                            && e.target_uuid() != node_id
                                                    });
                                                });
                                            }
                                            node_selected.set(None);
                                        }
                                        Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
                                    }
                                }
                                NodeType::Analyzer(_) => {
                                    match api::delete_analyzer(&HTTP_API_CLIENT(), node_id).await {
                                        Ok(_) => {
                                            graph_store().nodes_mut().write().remove(&node_id);
                                            node_selected.set(None);
                                        }
                                        Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
                                    }
                                }
                            }
                        }
                    }
                    GraphStoreAction::AddOpticNode(new_node) => {
                        match api::post_add_node(&HTTP_API_CLIENT(), new_node, Uuid::nil()).await {
                            Ok(node_info) => {
                                let ports = get_ports(node_info.uuid()).await;
                                let node_element = NodeElement::new(
                                    node_info.name().to_string(),
                                    NodeType::Optical(node_info.node_type().to_string()),
                                    node_info.uuid(),
                                    Point2D::new(100.0, 100.0),
                                    ports,
                                );
                                let id = node_element.id();
                                graph_store
                                    .write()
                                    .nodes
                                    .write()
                                    .insert(id, node_element.clone());
                                graph_store.write().set_node_active(id);
                                node_selected.set(Some(node_element));
                            }
                            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
                        }
                    }
                    GraphStoreAction::AddOpticReference(new_ref_node) => {
                        match api::post_add_ref_node(&HTTP_API_CLIENT(), new_ref_node, Uuid::nil())
                            .await
                        {
                            Ok(node_info) => {
                                let ports =
                                    Ports::new(node_info.input_ports(), node_info.output_ports());
                                let node_element = NodeElement::new(
                                    node_info.name().to_string(),
                                    NodeType::Optical(node_info.node_type().to_string()),
                                    node_info.uuid(),
                                    Point2D::new(100.0, 100.0),
                                    ports,
                                );
                                let id = node_element.id();
                                graph_store.write().nodes.write().insert(id, node_element);
                                graph_store.write().set_node_active(id);
                            }
                            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
                        }
                    }
                    GraphStoreAction::AddAnalyzer(new_analyzer) => {
                        match api::post_add_analyzer(&HTTP_API_CLIENT(), new_analyzer.clone()).await
                        {
                            Ok(analyzer_id) => {
                                let (x, y) = new_analyzer.gui_position;
                                let node_element = NodeElement::new(
                                    format!("{}", new_analyzer.analyzer_type),
                                    NodeType::Analyzer(new_analyzer.analyzer_type),
                                    analyzer_id,
                                    Point2D::new(x, y),
                                    Ports::default(),
                                );
                                graph_store
                                    .write()
                                    .nodes
                                    .write()
                                    .insert(analyzer_id, node_element.clone());
                                node_selected.set(Some(node_element));
                            }
                            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
                        }
                    }
                    GraphStoreAction::AddEdge(edge) => {
                        match api::post_add_connection(&HTTP_API_CLIENT(), edge.clone()).await {
                            Ok(_) => {
                                graph_store().edges_mut().write().push(edge);
                            }
                            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
                        }
                    }
                    GraphStoreAction::UpdateEdge(edge) => {
                        if api::update_distance(&HTTP_API_CLIENT(), edge.clone())
                            .await
                            .is_ok()
                        {
                            if let Some(e) =
                                graph_store.write().edges.write().iter_mut().find(|e| {
                                    e.src_uuid() == edge.src_uuid()
                                        && e.target_uuid() == edge.target_uuid()
                                })
                            {
                                *e = edge;
                            }
                        }
                    }
                    GraphStoreAction::DeleteEdge(edge) => {
                        if api::delete_connection(&HTTP_API_CLIENT(), edge.clone())
                            .await
                            .is_ok()
                        {
                            graph_store.write().edges.write().retain(|e| e != &edge);
                        }
                    }
                    GraphStoreAction::DeleteScenery => {
                        match api::delete_scenery(&HTTP_API_CLIENT()).await {
                            Ok(_) => {
                                let mut store = graph_store.write();
                                store.nodes.write().clear();
                                store.edges.write().clear();
                            }
                            Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
                        }
                    }
                    GraphStoreAction::OptimizeLayout => {
                        let edges = graph_store.read().edges().read().clone();
                        if let Ok(new_positions) = optimize_layout_and_sync(edges).await {
                            let mut store = graph_store.write();
                            let mut nodes = store.nodes.write();
                            for (id, pos) in new_positions {
                                if let Some(node) = nodes.get_mut(&id) {
                                    node.set_pos(pos);
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}
