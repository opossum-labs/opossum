use std::{
    collections::{hash_map::Values, HashMap},
    rc::Rc,
    str::FromStr,
};

use crate::bindings::getNodeInfo;
use log::debug;
use serde_json::Value;
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;
use web_sys::{DragEvent, HtmlElement};
use yew::{function_component, html, Callback, Html, Properties, Reducible, TargetCast};

use super::callbacks::NodeCallbacks;

#[derive(Clone, PartialEq)]
pub struct HTMLNodeElement {
    id: Uuid,
    x: i32,
    y: i32,
    name: String,
    is_source: bool,
    offset: (i32, i32),
}

impl HTMLNodeElement {
    pub fn new(
        id: Uuid,
        x: i32,
        y: i32,
        name: String,
        is_source: bool,
        offset: (i32, i32),
    ) -> Self {
        HTMLNodeElement {
            id,
            x,
            y,
            name,
            is_source,
            offset,
        }
    }
    pub fn offset(&self) -> (i32, i32) {
        self.offset
    }
    pub fn set_offset(&mut self, offset: (i32, i32)) {
        self.offset = offset;
    }
    pub fn set_id(&mut self, id: Uuid) {
        self.id = id;
    }
    pub fn set_x(&mut self, x: i32) {
        self.x = x;
    }
    pub fn set_y(&mut self, y: i32) {
        self.y = y;
    }
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }
    pub fn set_is_source(&mut self, is_source: bool) {
        self.is_source = is_source;
    }
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn x(&self) -> i32 {
        self.x
    }
    pub fn y(&self) -> i32 {
        self.y
    }
    pub fn name(&self) -> String {
        self.name.clone()
    }
    pub fn is_source(&self) -> bool {
        self.is_source
    }
}

// Verbindung zwischen Ports
#[derive(Clone, PartialEq)]
pub struct Connection {
    pub from: Uuid,
    pub to: Uuid,
}

#[derive(Clone, PartialEq)]
pub struct Connections {
    pub connections: HashMap<Uuid, Connection>,
}
impl Connections {
    pub fn new() -> Self {
        Connections {
            connections: HashMap::<Uuid, Connection>::new(),
        }
    }
    pub fn values(&self) -> Values<Uuid, Connection> {
        self.connections.values()
    }
    pub fn check_connection_validity(&self, connect_to_id: Uuid) -> bool {
        self.connections
            .values()
            .into_iter()
            .fold(true, |arg0, c| (c.to != connect_to_id) & arg0)
    }
    pub fn insert_connection(&mut self, selected_type: String, selected_id: Uuid, node_id: Uuid) {
        self.connections.insert(
            if selected_type == "output_1" {
                selected_id
            } else {
                node_id
            },
            Connection {
                from: if selected_type == "output_1" {
                    selected_id
                } else {
                    node_id
                },
                to: if selected_type == "input_1" {
                    selected_id
                } else {
                    node_id
                },
            },
        );
    }
}

// Node-Komponente
#[derive(Properties, PartialEq)]
pub struct NodeProps {
    pub html_node: HTMLNodeElement,
    pub width: i32,                    // Breite der Node
    pub height: i32,                   // Höhe der Node
    pub node_callbacks: NodeCallbacks, // Callback für den Drag-Start
    pub is_active: bool,
}

#[derive(Clone, PartialEq)]
pub struct NodeStates {
    nodes: Vec<HTMLNodeElement>,
    active_node: Option<Value>,
    connections: Connections,
    selected_port: Option<(Uuid, String)>,
}
impl NodeStates {
    pub fn new(
        nodes: Vec<HTMLNodeElement>,
        active_node: Option<Value>,
        connections: Connections,
        selected_port: Option<(Uuid, String)>,
    ) -> Self {
        NodeStates {
            nodes,
            active_node,
            connections,
            selected_port,
        }
    }
    pub fn nodes(&self) -> &Vec<HTMLNodeElement> {
        &self.nodes
    }
    pub fn active_node(&self) -> &Option<Value> {
        &self.active_node
    }
    pub fn add_node(&mut self, node: HTMLNodeElement) {
        self.nodes.push(node);
    }
    pub fn set_active_node(&mut self, active_node: Value) {
        self.active_node = Some(active_node);
    }
    pub fn connections(&self) -> &Connections {
        &self.connections
    }
    pub fn selected_port(&self) -> &Option<(Uuid, String)> {
        &self.selected_port
    }
    pub fn active_node_uuid_str(&self) -> Option<String> {
        if let Some(active_node) = &self.active_node {
            active_node["uuid"]
                .as_str()
                .map(|uuid_str| uuid_str.to_string())
        } else {
            None
        }
    }
}

impl Reducible for NodeStates {
    type Action = NodeAction;

    fn reduce(self: std::rc::Rc<Self>, action: Self::Action) -> std::rc::Rc<Self> {
        match action {
            NodeAction::AddNode(node) => {
                let mut nodestates = self.as_ref().clone();
                nodestates.add_node(node);
                Rc::new(nodestates)
            }
            NodeAction::UpdateNode(updated_node) => {
                let mut nodes = self.nodes.clone();
                let index = nodes
                    .iter()
                    .position(|node| node.id() == updated_node.id())
                    .unwrap();
                nodes[index] = updated_node;

                Rc::new(NodeStates {
                    nodes,
                    ..self.as_ref().clone()
                })
            }
            NodeAction::NodeDoubleClick(js_val) => Rc::new(NodeStates {
                active_node: Some(js_val),
                ..self.as_ref().clone()
            }),
            NodeAction::UpdateNodeName(uuid_str, new_name, active_node_js) => {
                let mut nodes = self.nodes.clone();
                let index = nodes
                    .iter()
                    .position(|node| node.id() == Uuid::from_str(uuid_str.as_str()).unwrap())
                    .unwrap();
                nodes[index].set_name(new_name);
                Rc::new(NodeStates {
                    nodes,
                    active_node: Some(active_node_js),
                    ..self.as_ref().clone()
                })
            }
            NodeAction::UpdateConnections(connections) => Rc::new(NodeStates {
                connections,
                ..self.as_ref().clone()
            }),
            NodeAction::SelectPort(selected_port) => Rc::new(NodeStates {
                selected_port,
                ..self.as_ref().clone()
            }),
            NodeAction::SetDragStartOffset(offset, uuid_str) => {
                let mut nodes = self.nodes.clone();
                let index = nodes
                    .iter()
                    .position(|node| node.id() == Uuid::from_str(uuid_str.as_str()).unwrap())
                    .unwrap();
                nodes[index].set_offset(offset);
                Rc::new(NodeStates {
                    nodes,
                    ..self.as_ref().clone()
                })
            }
            NodeAction::UpdateNodeLIDT(active_node) => {
                let mut nodes = self.nodes.clone();
                Rc::new(NodeStates {
                    active_node: Some(active_node),
                    ..self.as_ref().clone()
                })
            }
        }
    }
}

pub enum NodeAction {
    AddNode(HTMLNodeElement),
    UpdateNode(HTMLNodeElement),
    UpdateNodeName(String, String, Value),
    UpdateNodeLIDT(Value),
    NodeDoubleClick(Value),
    UpdateConnections(Connections),
    SelectPort(Option<(Uuid, String)>),
    SetDragStartOffset((i32, i32), String),
}

#[function_component(Node)]
pub fn node(props: &NodeProps) -> Html {
    let NodeProps {
        html_node,
        width,
        height,
        node_callbacks,
        is_active,
    } = props;

    // Berechne die Position der Ports
    let port_w_h = 12;
    let border_radius = 2;
    let top_port_x = width / 2 - port_w_h / 2 - border_radius; // Horizontal zentriert
    let bottom_port_x = width / 2 - port_w_h / 2 - border_radius; // Horizontal zentriert

    // Position des oberen Ports
    let top_port_y = -port_w_h / 2 - 3 * border_radius / 2; // An der oberen Kante des Containers

    // Position des unteren Ports
    let bottom_port_y = height - port_w_h / 2 - border_radius / 2; // An der unteren Kante des Containers

    // Klick-Handler für Ports
    let on_input_port_click = {
        let on_port_click = node_callbacks.on_port_click.clone();
        let id = html_node.id();
        Callback::from(move |_| on_port_click.emit((id, "input_1".to_string())))
    };

    let on_output_port_click = {
        let on_port_click = node_callbacks.on_port_click.clone();
        let id = html_node.id();
        Callback::from(move |_| on_port_click.emit((id, "output_1".to_string())))
    };

    // Drag-Ende-Handler
    let on_drag_end = {
        let on_drag_end = node_callbacks.on_drag_end.clone();
        let id = html_node.id();
        move |event: DragEvent| {
            if let Some(target) = event.target_dyn_into::<HtmlElement>() {
                let target = target.parent_element().unwrap();
                if let Some(container) = target.closest(".drop-container").unwrap() {
                    let rect = container.get_bounding_client_rect();
                    let new_x = event.page_x() as i32 - rect.left() as i32;
                    let new_y = event.page_y() as i32 - rect.top() as i32;

                    on_drag_end.emit((id, new_x, new_y)); // ID und neue Position übergeben
                }
            }
        }
    };

    let on_dblclick = {
        let on_double_click = node_callbacks.on_node_double_click.clone();
        let id = html_node.id().clone();
        let name = html_node.name().clone();
        let add_log_handler = node_callbacks.on_add_log.clone();

        Callback::from(move |_| {
            let id = id.clone();
            let name = name.clone();
            let add_log_handler = add_log_handler.clone();
            let on_double_click = on_double_click.clone();

            spawn_local(async move {
                let result = unsafe { getNodeInfo(id.clone().as_simple().to_string()) };
                let result = wasm_bindgen_futures::JsFuture::from(result).await;

                match result {
                    Ok(value) => {
                        if let Ok(to_json) = serde_json::from_str(&value.as_string().unwrap()) {
                            on_double_click.emit(to_json);
                        } else {
                            add_log_handler.emit(format!(
                                "Error while getting info of node \"{}\". Error: {}",
                                name.clone(),
                                "Error while deserializing json"
                            ));
                        };
                    }
                    Err(e) => {
                        add_log_handler.emit(format!(
                            "Error while getting info of node \"{}\". Error: {}",
                            name.clone(),
                            e.as_string().unwrap()
                        ));
                    }
                    _ => {
                        add_log_handler.emit(format!("Error: unknown"));
                    }
                }
            });
        })
    };

    let style = if *is_active { " active-node" } else { "" };

    html! {
        <div
            ondblclick={on_dblclick}
            class={format!("node{style} draggable prevent-select")}
            style={format!("position: absolute; left: {}px; top: {}px;", html_node.x(), html_node.y())}>

            <div id={html_node.id().to_string()} class="node-content" style={format!("width: {}px; height: {}px;", width, height)}>{html_node.name()}</div>

            <div
            draggable="true"

            ondragstart={node_callbacks.on_drag_start.clone()}
            ondragend={on_drag_end}
                class="drag-anchor"
                style={format!("positon:absolute;left: {}px; top: {}px; width: {}px; height: {}px;", -2*port_w_h/3, -2*port_w_h/3, port_w_h, port_w_h)}>
                <img src="static/images/grab.png" style={format!("position: absolute; margin: auto;top:2px; left:2px;width: {}px; height: {}px;", port_w_h-4, port_w_h-4)}/>
            </div>

            // Input-Port
            <div
                class="port input-port"
                onclick={on_input_port_click}
                style={format!("left: {}px; top: {}px; width: {}px; height: {}px;", top_port_x, top_port_y, port_w_h, port_w_h)}>
            </div>
            {
            if !html_node.is_source(){
                html!{
            // Output-Port
            <div
                class="port output-port"
                onclick={on_output_port_click}
                style={format!("left: {}px; top: {}px; width: {}px; height: {}px;", bottom_port_x, bottom_port_y, port_w_h, port_w_h)}>
            </div>
            }
        }
        else{
            html!{}
        }
        }
        </div>
    }
}
