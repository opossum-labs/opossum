#![allow(clippy::derive_partial_eq_without_eq)]
use crate::{CONTEXT_MENU, components::context_menu::sub_menu_item::MenuItem};
use dioxus::prelude::*;
use opossum_core::{prelude::PortType, types::api_types::NewRefNode};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum CxtCommand {
    AddRefNode(NewRefNode),
    ConvertToGroup {
        nodes: Vec<Uuid>,
        graph_id: Uuid,
    },
    MapNodePort {
        port_type: PortType,
        group_port_name: String,
        mapped_node_port_name: String,
        mapped_node_id: Uuid,
        group_id: Uuid,
    },
}

#[derive(Clone, PartialEq, Debug)]
pub struct CxMenu {
    pub x: f64,
    pub y: f64,
    pub entries: Vec<(String, CxtCommand)>,
}
impl CxMenu {
    #[must_use]
    pub const fn width() -> f64 {
        150. + 2. * Self::padding()
    }
    #[must_use]
    pub const fn padding() -> f64 {
        2.
    }
    #[must_use]
    pub const fn new(x: f64, y: f64, entries: Vec<(String, CxtCommand)>) -> Self {
        Self { x, y, entries }
    }
}

#[component]
pub fn ContextMenu(cxt_command_handler: EventHandler<Option<CxtCommand>>) -> Element {
    let cx_menu_opt = CONTEXT_MENU();

    if let Some(cx_menu) = cx_menu_opt {
        let (x, y) = (cx_menu.x, cx_menu.y);
        let width = CxMenu::width();
        let padding = CxMenu::padding();
        rsx!(
            div {
                id: "context-menu",
                style: "top: {y}px; left: {x}px; width: {width}px; padding: {padding}px;",

                for (index , (label , cmd)) in cx_menu.entries.into_iter().enumerate() {
                    MenuItem {
                        key: "{index}",
                        class: "context-menu-item",
                        onclick: move |_| {
                            cxt_command_handler.call(Some(cmd.clone()));
                            *CONTEXT_MENU.write() = None;
                        },
                        "{label}"
                    }
                }
            }
        )
    } else {
        rsx! {}
    }
}
