use std::collections::HashMap;

use crate::{
    components::{
        context_menu::cx_menu::CxMenu,
        node_components::node_drag_drop_container::drag_drop_container::ZoomShift,
    },
    CONTEXT_MENU, EDGES, HTTP_API_CLIENT, OPOSSUM_UI_LOGS, ZOOM,
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
    pub fn remove_edge(&mut self, conn_info: ConnectInfo) {
        self.edges.remove(&format!(
            "{}_{}",
            conn_info.src_uuid().as_simple(),
            conn_info.src_port()
        ));
    }
}

impl ZoomShift for Edges {
    fn zoom_shift(&mut self, zoom_factor: f64, shift: (f64, f64), zoom_center: (f64, f64)) {
        for edge in self.edges_mut().values_mut() {
            edge.zoom_shift(zoom_factor, shift, zoom_center);
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct EdgeCreation {
    src_node: Uuid,
    src_port: String,
    src_port_type: PortType,
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
    bezier_offset: f64,
}

impl EdgeCreation {
    #[must_use]
    pub fn new(
        src_node: Uuid,
        src_port: String,
        src_port_type: PortType,
        start: Point2D<f64>,
        end: Point2D<f64>,
        bezier_offset: f64,
    ) -> Self {
        let connection_factor = if src_port_type == PortType::Input {
            -1.
        } else {
            1.
        };
        Self {
            src_node,
            src_port,
            src_port_type,
            start_x: start.x,
            start_y: start.y,
            end_x: end.x,
            end_y: end.y,
            bezier_offset: bezier_offset * connection_factor,
        }
    }
    #[must_use]
    pub const fn port_type(&self) -> &PortType {
        &self.src_port_type
    }
    #[must_use]
    pub fn port_name(&self) -> String {
        self.src_port.clone()
    }
    #[must_use]
    pub const fn node_id(&self) -> Uuid {
        self.src_node
    }
    #[must_use]
    pub fn start_port_id(&self) -> String {
        format!("{}_{}", self.src_node.as_simple(), self.src_port)
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
    #[must_use]
    pub const fn bezier_offset(&self) -> f64 {
        self.bezier_offset
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
    pub const fn set_start_x(&mut self, start_x: f64) {
        self.start_x = start_x;
    }
    pub const fn set_start_y(&mut self, start_y: f64) {
        self.start_y = start_y;
    }
}

impl ZoomShift for EdgeCreation {
    fn zoom_shift(&mut self, zoom_factor: f64, shift: (f64, f64), zoom_center: (f64, f64)) {
        let new_start_x = shift.0 + (self.start_x() - zoom_center.0) * zoom_factor;
        let new_start_y = shift.1 + (self.start_y() - zoom_center.1) * zoom_factor;
        self.set_start_x(new_start_x);
        self.set_start_y(new_start_y);

        let new_end_x = shift.0 + (self.end_x() - zoom_center.0) * zoom_factor;
        let new_end_y = shift.1 + (self.end_y() - zoom_center.1) * zoom_factor;
        self.set_end_x(new_end_x);
        self.set_end_y(new_end_y);

        self.set_bezier_offset(self.bezier_offset() * zoom_factor);
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

impl ZoomShift for Edge {
    fn zoom_shift(&mut self, zoom_factor: f64, shift: (f64, f64), zoom_center: (f64, f64)) {
        let new_start_x = shift.0 + (self.start_x() - zoom_center.0) * zoom_factor;
        let new_start_y = shift.1 + (self.start_y() - zoom_center.1) * zoom_factor;
        self.set_start_x(new_start_x);
        self.set_start_y(new_start_y);

        let new_end_x = shift.0 + (self.end_x() - zoom_center.0) * zoom_factor;
        let new_end_y = shift.1 + (self.end_y() - zoom_center.1) * zoom_factor;
        self.set_end_x(new_end_x);
        self.set_end_y(new_end_y);

        self.set_bezier_offset(self.bezier_offset() * zoom_factor);
    }
}

fn define_bezier_path(
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
    bezier_offset: f64,
) -> String {
    format!(
        "M{},{} C{},{} {},{} {},{}",
        start_x,
        start_y,
        start_x + bezier_offset,
        start_y,
        end_x - bezier_offset,
        end_y,
        end_x,
        end_y,
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
    let edge_opt_sig = use_context::<Signal<Option<EdgeCreation>>>();
    if let Some(edge) = edge_opt_sig() {
        let new_path = define_bezier_path(
            edge.start_x(),
            edge.start_y(),
            edge.end_x(),
            edge.end_y(),
            edge.bezier_offset(),
        );
        rsx! {
            path {
                d: new_path,
                stroke: "black",
                fill: "transparent",
                stroke_width: format!("{}", 2. * ZOOM.read().current()),
            }
        }
    } else {
        rsx! {}
    }
}

#[component]
pub fn EdgeComponent(edge: Edge) -> Element {
    let mut distance_val = use_signal(|| format!("{}", edge.distance()));
    let new_path = define_bezier_path(
        edge.start_x(),
        edge.start_y(),
        edge.end_x(),
        edge.end_y(),
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
            stroke_width: format!("{}", 2. * ZOOM.read().current()),
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
            match HTTP_API_CLIENT().delete_connection(conn_info).await {
                Ok(conn_info) => {
                    EDGES.write().remove_edge(conn_info);
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log("Removed edge successfully!");
                }
                Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
            }
        });
    })
}
