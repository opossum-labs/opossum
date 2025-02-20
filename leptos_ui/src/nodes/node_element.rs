use std::{
    collections::{hash_map::Values, HashMap},
    rc::Rc,
    str::FromStr,
    sync::Arc,
};

use leptos::{
    component,
    prelude::{
        ClassAttribute, ElementChild, Get, GlobalAttributes, Read, ReadSignal, RwSignal, Set,
        StyleAttribute, Update,
    },
    view, IntoView,
};
use log::debug;
use reactive_stores::Store;
use uuid::Uuid;

#[derive(Clone, PartialEq, Store, Default)]
pub struct NodesStore {
    #[store(key: Uuid = |node| node.id.clone())]
    nodes: Vec<HTMLNodeElement>,
}

#[derive(Clone, PartialEq, Store, Default)]
pub struct HTMLNodeElement {
    x: i32,
    y: i32,
    id: Uuid,
    name: String,
    is_source: bool,
    offset: (i32, i32),
}

impl HTMLNodeElement {
    pub fn new(
        x: i32,
        y: i32,
        id: Uuid,
        name: String,
        is_source: bool,
        offset: (i32, i32),
    ) -> Self {
        HTMLNodeElement {
            x,
            y,
            id,
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
    pub fn set_x(&mut self, new_x: i32) {
        self.x = new_x;
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
    pub fn id(&self) -> &Uuid {
        &self.id
    }
    pub fn set_id(&mut self, id: Uuid) {
        self.id = id;
    }
}

#[component]
pub fn Node(node: HTMLNodeElement) -> impl IntoView {
    let width = 100;
    let height = 100;
    // Berechne die Position der Ports
    let port_w_h = 12;
    let border_radius = 2;
    let top_port_x = width / 2 - port_w_h / 2 - border_radius; // Horizontal zentriert
    let bottom_port_x = width / 2 - port_w_h / 2 - border_radius; // Horizontal zentriert

    // Position des oberen Ports
    let top_port_y = -port_w_h / 2 - 3 * border_radius / 2; // An der oberen Kante des Containers

    // Position des unteren Ports
    let bottom_port_y = height - port_w_h / 2 - border_radius / 2; // An der unteren Kante des Containers

    view! {
        <div
            class="node draggable prevent-select"
            style={format!("position: absolute; left: {}px; top: {}px;", node.x(), node.y())}
            >

            <div class="node-content" style={format!("width: {}px; height: {}px;", width, height)}>{
                node.name()
            }</div>

            <div
            draggable="true"
                class="drag-anchor"
                style={format!("positon:absolute;left: {}px; top: {}px; width: {}px; height: {}px;", -2*port_w_h/3, -2*port_w_h/3, port_w_h, port_w_h)}>
                <img src="static/images/grab.png" style={format!("position: absolute; margin: auto;top:2px; left:2px;width: {}px; height: {}px;", port_w_h-4, port_w_h-4)}/>
            </div>

            // Input-Port
            <div
                class="port input-port"
                style={format!("left: {}px; top: {}px; width: {}px; height: {}px;", top_port_x, top_port_y, port_w_h, port_w_h)}>
            </div>

            // Output-Port
            <div
                class="port output-port"
                style={format!("left: {}px; top: {}px; width: {}px; height: {}px;", bottom_port_x, bottom_port_y, port_w_h, port_w_h)}>
            </div>
        </div>
    }
}
