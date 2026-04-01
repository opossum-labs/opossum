use dioxus::html::geometry::euclid::default::{Point2D, Rect};
use opossum_core::{
    prelude::{AnalyzerType, PortType},
    types::api_types::{ConnectInfo, NewRefNode},
};
use std::{collections::HashSet, path::PathBuf};
use uuid::Uuid;

use crate::components::scenery_editor::graph_editor::DragStatus;

pub enum GraphsWorkspaceAction {
    // Group into which other nodes could be dropped
    // UUid of that group
    // z-index of that group to select group directly underneath
    SetDropInGroup(Option<(Uuid,usize)>),
    SetSelectionBox(Option<Rect<f64>>),
    SetDragStatus(DragStatus),
    LoadFromFile(PathBuf),
    SaveToFile(PathBuf),
    ConvertToGroup {
        nodes: Vec<Uuid>,
        graph_id: Uuid,
    },
    AddRootSceneryTab {
        name: String,
    },
    MapNodePort {
        port_type: PortType,
        group_port_name: String,
        mapped_node_port_name: String,
        mapped_node_id: Uuid,
        group_id: Uuid,
    },
    RemovePortMap {
        group_id: Uuid,
        group_port_name: String,
        port_type: PortType,
    },
    DeleteRootScenery,
    OpenGroupTab {
        group_id: Uuid,
        group_name: String,
    },
    AddOpticNode {
        node_type: String,
        graph_id: Uuid,
    },
    AddOpticReference {
        new_ref_node: NewRefNode,
        graph_id: Uuid,
    },
    AddAnalyzer {
        analyzer_type: AnalyzerType,
        graph_id: Uuid,
    },
    OptimizeLayout {
        graph_id: Uuid,
    },
    CenterGraph {
        graph_id: Uuid,
        save_changes: bool,
    },
    ZoomToFit {
        graph_id: Uuid,
        save_changes: bool,
    },
    UpdateEdges {
        connections: Vec<ConnectInfo>,
        graph_id: Uuid,
    },
    UpdateEdge {
        connection: ConnectInfo,
        graph_id: Uuid,
    },
    DeleteEdge {
        connection: ConnectInfo,
        graph_id: Uuid,
    },
    AddEdge {
        new_edge: ConnectInfo,
        graph_id: Uuid,
    },
    InvertNode {
        inverted: bool,
        graph_id: Uuid,
        node_id: Uuid,
    },
    SetNodeName {
        name: String,
        graph_id: Uuid,
        node_id: Uuid,
        needs_saving: bool,
    },
    CopyNodes {
        nodes: HashSet<Uuid>,
    },
    PasteNode {
        pos: Point2D<f64>,
        graph_id: Uuid,
    },
    SyncNodePosition {
        pos: Point2D<f64>,
        node_id: Uuid,
    },
    DeleteNode {
        node_id: Uuid,
        graph_id: Uuid,
    },
    DropNodesIntoGroup {
        nodes: Vec<Uuid>,
        from_graph_id: Uuid,
        to_graph_id: Uuid,
    },
}
