use crate::{
    CONNECTIONS_PLACING_BY_DISTANCE,
    components::scenery_editor::{
        EditorStateStoreExt, GraphState,
        edges::{edge_component::EdgeComponent, edge_path},
        graph_workspace::{GraphStateStoreExt, GraphStoreStoreExt},
    },
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_core::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct NewEdgeCreationStart {
    pub src_node: Uuid,
    pub src_port: String,
    pub src_port_type: PortType,
    pub start_pos: Point2D<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdgePort {
    pub node_id: Uuid,
    pub port_name: String,
    pub port_type: PortType,
}

#[derive(Clone, PartialEq, Debug)]
pub struct EdgeCreation {
    start_port: EdgePort,
    end_port: Option<EdgePort>,
    start: Point2D<f64>,
    end: Point2D<f64>,
}

impl EdgeCreation {
    #[must_use]
    pub fn new(
        src_node: Uuid,
        src_port: String,
        src_port_type: PortType,
        start: Point2D<f64>,
    ) -> Self {
        Self {
            start_port: EdgePort {
                node_id: src_node,
                port_name: src_port,
                port_type: src_port_type,
            },
            end_port: None,
            start,
            end: start,
        }
    }
    pub fn shift_end(&mut self, shift: Point2D<f64>) {
        self.end.x += shift.x;
        self.end.y += shift.y;
    }
    pub fn set_end_port(&mut self, end_port: Option<EdgePort>) {
        self.end_port = end_port;
    }
    pub fn is_valid(&self) -> bool {
        if let Some(end_port) = &self.end_port {
            if end_port.node_id == self.start_port.node_id {
                return false;
            }
            if end_port.port_type == self.start_port.port_type {
                return false;
            }
            true
        } else {
            false
        }
    }
    pub const fn start_port(&self) -> &EdgePort {
        &self.start_port
    }
    pub const fn end_port(&self) -> Option<&EdgePort> {
        self.end_port.as_ref()
    }
}
/// Draw the connections of the graph.
///
/// Which of them place a node by their distance is what the 3D view last found when it positioned
/// the model (see [`CONNECTIONS_PLACING_BY_DISTANCE`]): whether a distance places a node depends on
/// the whole model, not on the connection alone. Until a view has positioned the model, every
/// connection counts as placing one.
#[component]
pub fn EdgesComponent() -> Element {
    let graph_state = use_context::<ReadStore<GraphState>>();
    let graph_store = graph_state.graph_store();
    rsx! {
        for edge in graph_store.edges().iter() {
            EdgeComponent {
                key: "{edge.read().src_uuid()}-{edge.read().src_port()}-{edge.read().target_uuid()}-{edge.read().target_port()}",
                distance_places: CONNECTIONS_PLACING_BY_DISTANCE
                    .read()
                    .as_ref()
                    .is_none_or(|placing| {
                        placing.contains(&(edge.read().src_uuid(), edge.read().src_port().to_owned()))
                    }),
                edge,
            }
        }
    }
}
#[component]
pub fn EdgeCreationComponent() -> Element {
    let graph_state = use_context::<ReadStore<GraphState>>();
    let editor_status = graph_state.editor_state();
    editor_status.edge_in_creation().read().clone().map_or_else(
        || rsx! {},
        |edge| {
            // A drag from an input port ends at the output the connection will come from.
            let new_path = if edge.start_port.port_type == PortType::Input {
                edge_path(edge.end, edge.start)
            } else {
                edge_path(edge.start, edge.end)
            };
            rsx! {
                path {
                    d: new_path,
                    stroke: "black",
                    fill: "transparent",
                    stroke_width: format!("{}", 2.),
                }
            }
        },
    )
}
