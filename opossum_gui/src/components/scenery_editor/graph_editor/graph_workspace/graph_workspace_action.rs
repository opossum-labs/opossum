use dioxus::html::geometry::euclid::default::Point2D;
use opossum_core::{
    prelude::AnalyzerType,
    types::api_types::{ConnectInfo, NewRefNode},
};
use std::path::PathBuf;
use uuid::Uuid;

use crate::components::scenery_editor::NodeType;

pub enum GraphsWorkspaceAction {
    LoadFromFile(PathBuf),
    SaveToFile(PathBuf),
    GetRootSceneryId,
    DeleteRootScenery,
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
    },
    CopyNode {
        node_type: NodeType,
        node_id: Uuid,
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
}
