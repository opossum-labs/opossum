use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_core::prelude::PortType;

use crate::components::scenery_editor::constants::{BORDER_WIDTH, PORT_MAP_DIST, PORT_WIDTH};

#[component]
pub fn PortMapComponent(
    on_context_menu_handler: Callback<Event<MouseData>>,
    rel_port_position: Point2D<f64>,
    port_type: PortType,
    external_port: String,
) -> Element {
    match port_type {
        PortType::Input => rsx! {
            InputPortMapComponent { on_context_menu_handler, rel_port_position, external_port}
        },
        PortType::Output => rsx! {
            OutputPortMapComponent { on_context_menu_handler, rel_port_position, external_port }
        },
    }
}

#[component]
pub fn InputPortMapComponent(
    on_context_menu_handler: EventHandler<Event<MouseData>>,
    rel_port_position: Point2D<f64>,
    external_port: String,
) -> Element {
    rsx! {
        div {
            title: "mapped to port: {external_port}",
            class: "port-map-wrapper",
            style: format!(
                "left: {}px; top: {}px; transform: translate(-50%, -50%)",
                2.0f64.mul_add(-PORT_WIDTH, rel_port_position.x) - PORT_MAP_DIST - BORDER_WIDTH,
                rel_port_position.y,
            ),
            oncontextmenu: on_context_menu_handler,
            div { class: "graph-port-node-input" }

            div {
                class: "port-map-line",
                style: format!("right: {}px; width: {}px;", -1.5 * PORT_WIDTH, PORT_MAP_DIST),
            }
        }
    }
}

#[component]
pub fn OutputPortMapComponent(
    on_context_menu_handler: EventHandler<Event<MouseData>>,
    rel_port_position: Point2D<f64>,
    external_port: String,
) -> Element {
    rsx! {
        div {
            title: "mapped to port: {external_port}",
            class: "port-map-wrapper",
            style: format!(
                "right: -{}px; top: {}px; transform: translate(-50%, -50%)",
                2.0f64.mul_add(-(PORT_WIDTH - BORDER_WIDTH), rel_port_position.x)
                    - PORT_MAP_DIST,
                rel_port_position.y,
            ),
            oncontextmenu: on_context_menu_handler,

            div { class: "graph-port-node-output" }

            div {
                class: "port-map-line",
                style: format!("left: {}px; width: {}px;", -1.5 * PORT_WIDTH, PORT_MAP_DIST),
            }
        }
    }
}
