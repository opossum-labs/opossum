use crate::components::scenery_editor::{
    EditorState, GraphStore, GraphsWorkspaceState,
    constants::{BORDER_WIDTH, PORT_HEIGHT, PORT_WIDTH},
    node::NodeElement,
    ports::{
        hooks::{use_on_context_menu, use_on_mouse_down, use_on_mouse_enter, use_on_mouse_leave},
        port_map_component::PortMapComponent,
    },
};
use dioxus::prelude::*;
use opossum_core::prelude::*;
#[derive(Clone, Eq, PartialEq, Default, Debug)]
pub struct Ports {
    input_ports: Vec<String>,
    output_ports: Vec<String>,
}
impl Ports {
    #[must_use]
    pub const fn new(input_ports: Vec<String>, output_ports: Vec<String>) -> Self {
        Self {
            input_ports,
            output_ports,
        }
    }
    pub fn set_input_ports(&mut self, ports: Vec<String>) {
        self.input_ports = ports;
    }
    pub fn set_output_ports(&mut self, ports: Vec<String>) {
        self.output_ports = ports;
    }
    pub fn remove_input_port(&mut self, remove: &String) {
        self.input_ports.retain(|p| p != remove);
    }
    pub fn remove_output_port(&mut self, remove: &String) {
        self.output_ports.retain(|p| p != remove);
    }
    #[must_use]
    pub const fn input_ports(&self) -> &Vec<String> {
        &self.input_ports
    }
    #[must_use]
    pub const fn output_ports(&self) -> &Vec<String> {
        &self.output_ports
    }
    pub fn invert_ports(&mut self) {
        let input_buffer = self.input_ports.clone();
        self.input_ports = self.output_ports.clone();
        self.output_ports = input_buffer;
    }
}

#[component]
pub fn NodePort(
    node: NodeElement,
    port_name: String,
    port_type: PortType,
    inverted_node: bool,
) -> Element {
    let editor_status = use_context::<Signal<EditorState>>();
    let graph_store = use_context::<Signal<GraphStore>>();
    let workspace = use_context::<Signal<GraphsWorkspaceState>>();

    let rel_port_position = node.rel_port_position(port_type, &port_name);
    let abs_port_position = node.abs_port_position(port_type, &port_name);
    let node_id = node.id();
    let port_class = get_port_class(inverted_node, port_type);

    let is_mapped_port = graph_store
        .read()
        .mapped_ports
        .read()
        .contains_port_of_node(node.id(), &port_name);

    let on_mouse_down_handler = use_on_mouse_down(
        workspace,
        node_id,
        port_name.clone(),
        port_type,
        abs_port_position,
    );
    let on_mouse_leave_handler = use_on_mouse_leave(editor_status);
    let on_mouse_enter_handler = use_on_mouse_enter(
        editor_status,
        &port_name,
        node_id,
        port_type,
        is_mapped_port,
    );
    let on_context_menu_handler = use_on_context_menu(
        workspace.into(),
        graph_store.into(),
        node_id,
        port_name.clone(),
        port_type,
    );

    rsx! {
        div {
            class: "port {port_class}",
            title: "{port_name}",
            style: format!(
                "left: {}px; top: {}px; width: {}px; height: {}px; border-width: {}px; transform: translateX(-50%) translateX({}px) translateY(-50%)",
                rel_port_position.x,
                rel_port_position.y,
                PORT_WIDTH,
                PORT_HEIGHT,
                BORDER_WIDTH,
                -BORDER_WIDTH / 2.,
            ),
            draggable: false,
            onmousedown: move |e| on_mouse_down_handler.call(e),
            onmouseenter: move |e| on_mouse_enter_handler.call(e),
            onmouseleave: move |e| on_mouse_leave_handler.call(e),
            oncontextmenu: move |e| on_context_menu_handler.call(e),
        }

        if is_mapped_port {
            PortMapComponent {
                on_context_menu_handler,
                rel_port_position,
                port_type,
            }
        }
    }
}

fn get_port_class(is_inverted: bool, port_type: PortType) -> &'static str {
    if is_inverted {
        if port_type == PortType::Input {
            "output-port"
        } else {
            "input-port"
        }
    } else if port_type == PortType::Input {
        "input-port"
    } else {
        "output-port"
    }
}

#[component]
pub fn NodePorts(node: NodeElement, inverted: bool) -> Element {
    let input_ports = node.input_ports();
    rsx! {
        for in_port in input_ports {
            NodePort {
                node: node.clone(),
                port_name: in_port,
                port_type: PortType::Input,
                inverted_node: inverted,
            }
        }
        for out_port in node.output_ports() {
            NodePort {
                node: node.clone(),
                port_name: out_port,
                port_type: PortType::Output,
                inverted_node: inverted,
            }
        }
    }
}
