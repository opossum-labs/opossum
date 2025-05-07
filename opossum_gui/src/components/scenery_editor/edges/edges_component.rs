use std::collections::HashMap;

use crate::{
    api::{self},
    components::{
        context_menu::cx_menu::CxMenu,
        scenery_editor::{graph_editor::graph_editor_component::EditorState, EDGES},
    },
    CONTEXT_MENU, HTTP_API_CLIENT, OPOSSUM_UI_LOGS,
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_backend::{nodes::ConnectInfo, PortType};
use uuid::Uuid;

#[derive(Clone, PartialEq, Default)]
pub struct Edges {
    edges: HashMap<String, Edge>,
}
impl Edges {
    #[must_use]
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
        }
    }
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.insert(edge.start_port_id(), edge);
    }
    #[must_use]
    pub fn get_edge(&self, id: &str) -> Option<&Edge> {
        self.edges.get(id)
    }
    #[must_use]
    pub fn get_edge_mut(&mut self, id: &str) -> Option<&mut Edge> {
        self.edges.get_mut(id)
    }
    #[must_use]
    pub const fn edges(&self) -> &HashMap<String, Edge> {
        &self.edges
    }
    #[must_use]
    pub const fn edges_mut(&mut self) -> &mut HashMap<String, Edge> {
        &mut self.edges
    }

    pub fn shift_if_connected(&mut self, x_shift: f64, y_shift: f64, node_id: Uuid) {
        for edge in self.edges_mut().values_mut() {
            edge.shift_if_connected(x_shift, y_shift, node_id);
        }
    }
    pub fn remove_if_connected(&mut self, node_id: Uuid) {
        let keys_to_remove = self
            .edges()
            .iter()
            .filter(|(_, edge)| edge.is_connected(node_id))
            .map(|(k, _)| k.clone())
            .collect::<Vec<String>>();

        for key in &keys_to_remove {
            self.edges_mut().remove(key);
        }
    }
    pub fn remove_edge(&mut self, conn_info: &ConnectInfo) {
        self.edges.remove(&format!(
            "{}_{}",
            conn_info.src_uuid().as_simple(),
            conn_info.src_port()
        ));
    }
}

#[derive(Clone, Debug)]
pub struct NewEdgeCreationStart {
    pub src_node: Uuid,
    pub src_port: String,
    pub src_port_type: PortType,
    pub start_pos: Point2D<f64>,
    pub bezier_offset: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeCreationPort {
    pub node_id: Uuid,
    pub port_name: String,
    pub port_type: PortType,
}
#[derive(Clone, PartialEq, Debug)]
pub struct EdgeCreation {
    start_port: EdgeCreationPort,
    end_port: Option<EdgeCreationPort>,
    start: Point2D<f64>,
    end: Point2D<f64>,
    bezier_offset: f64,
}

impl EdgeCreation {
    #[must_use]
    pub fn new(
        src_node: Uuid,
        src_port: String,
        src_port_type: PortType,
        start: Point2D<f64>,
    ) -> Self {
        let connection_factor = if src_port_type == PortType::Input {
            -1.
        } else {
            1.
        };
        Self {
            start_port: EdgeCreationPort {
                node_id: src_node,
                port_name: src_port.clone(),
                port_type: src_port_type,
            },
            end_port: None,
            start,
            end: start,
            bezier_offset: 50. * connection_factor,
        }
    }
    #[must_use]
    pub const fn start(&self) -> Point2D<f64> {
        self.start
    }
    #[must_use]
    pub const fn end(&self) -> Point2D<f64> {
        self.end
    }
    pub fn shift_end(&mut self, shift: Point2D<f64>) {
        self.end.x += shift.x;
        self.end.y += shift.y;
    }
    #[must_use]
    pub const fn bezier_offset(&self) -> f64 {
        self.bezier_offset
    }
    pub fn set_end_port(&mut self, end_port: Option<EdgeCreationPort>) {
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
            return true;
        } else {
            return false;
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Edge {
    conn_info: ConnectInfo,
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
    bezier_offset: f64,
}

impl Edge {
    #[must_use]
    pub const fn new(
        conn_info: ConnectInfo,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        bezier_offset: f64,
    ) -> Self {
        Self {
            conn_info,
            start_x,
            start_y,
            end_x,
            end_y,
            bezier_offset,
        }
    }
    #[must_use]
    pub fn start_port_id(&self) -> String {
        format!(
            "{}_{}",
            self.conn_info.src_uuid().as_simple(),
            self.conn_info.src_port()
        )
    }
    #[must_use]
    pub fn end_port_id(&self) -> String {
        format!(
            "{}_{}",
            self.conn_info.target_uuid().as_simple(),
            self.conn_info.target_port()
        )
    }
    #[must_use]
    pub const fn start_x(&self) -> f64 {
        self.start_x
    }
    #[must_use]
    pub const fn start_y(&self) -> f64 {
        self.start_y
    }
    #[must_use]
    pub const fn end_x(&self) -> f64 {
        self.end_x
    }
    #[must_use]
    pub const fn end_y(&self) -> f64 {
        self.end_y
    }
    pub const fn set_start_x(&mut self, start_x: f64) {
        self.start_x = start_x;
    }
    pub const fn set_start_y(&mut self, start_y: f64) {
        self.start_y = start_y;
    }
    pub const fn set_end_x(&mut self, end_x: f64) {
        self.end_x = end_x;
    }
    pub const fn set_end_y(&mut self, end_y: f64) {
        self.end_y = end_y;
    }
    pub const fn set_bezier_offset(&mut self, bezier_offset: f64) {
        self.bezier_offset = bezier_offset;
    }
    #[must_use]
    pub const fn bezier_offset(&self) -> f64 {
        self.bezier_offset
    }
    #[must_use]
    pub const fn distance(&self) -> f64 {
        self.conn_info.distance()
    }

    pub fn shift_if_connected(&mut self, x_shift: f64, y_shift: f64, node_id: Uuid) {
        if self.conn_info.src_uuid() == node_id {
            self.set_start_x(self.start_x() + x_shift);
            self.set_start_y(self.start_y() + y_shift);
        }
        if self.conn_info.target_uuid() == node_id {
            self.set_end_x(self.end_x() + x_shift);
            self.set_end_y(self.end_y() + y_shift);
        }
    }
    #[must_use]
    pub fn is_connected(&self, node_id: Uuid) -> bool {
        self.conn_info.src_uuid() == node_id || self.conn_info.target_uuid() == node_id
    }
}

fn define_bezier_path(start: Point2D<f64>, end: Point2D<f64>, bezier_offset: f64) -> String {
    format!(
        "M{},{} C{},{} {},{} {},{}",
        start.x,
        start.y,
        start.x + bezier_offset,
        start.y,
        end.x - bezier_offset,
        end.y,
        end.x,
        end.y,
    )
}

#[component]
pub fn EdgesComponent() -> Element {
    rsx! {
        for edge in EDGES.read().edges().values() {
            EdgeComponent { edge: edge.clone() }
        }
    }
}

#[component]
pub fn EdgeCreationComponent() -> Element {
    let editor_status = use_context::<EditorState>();
    let edge_in_creation = &*(editor_status.edge_in_creation.read());
    edge_in_creation.clone().map_or_else(
        || rsx! {},
        |edge| {
            let new_path = define_bezier_path(edge.start(), edge.end(), edge.bezier_offset());
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

#[component]
pub fn EdgeComponent(edge: Edge) -> Element {
    let mut distance_val = use_signal(|| format!("{}", edge.distance()));
    let new_path = define_bezier_path(
        Point2D::new(edge.start_x(), edge.start_y()),
        Point2D::new(edge.end_x(), edge.end_y()),
        edge.bezier_offset(),
    );

    let input_height: f64 = 20.;
    let input_width: f64 = 50.;
    let distance_field_x = (edge.end_x() + edge.start_x() - input_width) / 2.;
    let distance_field_y = (edge.end_y() + edge.start_y() - input_height) / 2.;
    rsx! {
        path {
            d: new_path,
            oncontextmenu: use_edge_context_menu(edge.conn_info),
            stroke: "black",
            fill: "transparent",
            stroke_width: format!("{}", 2.),
        }
        foreignObject {
            class: "distance-field",
            x: distance_field_x,
            y: distance_field_y,
            style: "background-color: white; width: {input_width}; height: {input_height}",
            input {
                r#type: "number",
                value: distance_val,
                oninput: move |e| distance_val.set(e.value()),
            }
        }
    }
}
#[must_use]
pub fn use_edge_context_menu(conn_info: ConnectInfo) -> Callback<Event<MouseData>> {
    use_callback(move |evt: Event<MouseData>| {
        evt.prevent_default();
        let mut cx_menu = CONTEXT_MENU.write();
        *cx_menu = CxMenu::new(
            evt.page_coordinates().x,
            evt.page_coordinates().y,
            vec![(
                "Delete connection".to_owned(),
                use_delete_edge(conn_info.clone()),
            )],
        );
    })
}
#[must_use]
pub fn use_delete_edge(conn_info: ConnectInfo) -> Callback<Event<MouseData>> {
    use_callback(move |_: Event<MouseData>| {
        let conn_info = conn_info.clone();
        spawn(async move {
            match api::delete_connection(&HTTP_API_CLIENT(), conn_info).await {
                Ok(conn_info) => {
                    EDGES.write().remove_edge(&conn_info);
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log("Removed edge successfully!");
                }
                Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
            }
        });
    })
}
