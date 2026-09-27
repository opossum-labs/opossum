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
    RemovePortMap {
        group_id: Uuid,
        group_port_name: String,
        port_type: PortType,
    },
    JumpToMappedPort {
        mapped_node_id: Uuid,
        parent: (Uuid, String),
    },
    /// Marks or unmarks a node as an amplifier candidate - what the context menu's "As
    /// amplifier"/"As passive optic" entry sends. A hardware fact, independent of any pump
    /// scenario: offered for every volume node type regardless of whether one is active.
    ToggleAmplifierCandidate {
        node_id: Uuid,
        graph_id: Uuid,
        is_amplifier: bool,
    },
}

/// What a context menu entry does when clicked.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuAction {
    /// A command on the document, handed to the app, which routes it to whatever carries it out.
    Command(CxtCommand),
    /// A change to the view only, carried out on the spot by the component that opened the menu -
    /// how the editor's tabs are arranged, say, which is nothing the app routes anywhere.
    View(Callback),
}

#[derive(Clone, PartialEq, Debug)]
pub struct CxMenu {
    pub x: f64,
    pub y: f64,
    pub entries: Vec<(String, MenuAction)>,
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
    pub fn new(x: f64, y: f64, entries: Vec<(String, CxtCommand)>) -> Self {
        Self {
            x,
            y,
            entries: entries
                .into_iter()
                .map(|(label, command)| (label, MenuAction::Command(command)))
                .collect(),
        }
    }

    pub fn add_entry(&mut self, (label, command): (String, CxtCommand)) {
        self.entries.push((label, MenuAction::Command(command)));
    }

    /// Add an entry that changes only the view.
    ///
    /// # Arguments
    ///
    /// * `label` - the entry's text.
    /// * `action` - run when the entry is clicked, right where the menu was opened from.
    pub fn add_view_entry(&mut self, label: String, action: Callback) {
        self.entries.push((label, MenuAction::View(action)));
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

                for (index , (label , action)) in cx_menu.entries.into_iter().enumerate() {
                    MenuItem {
                        key: "{index}",
                        class: "context-menu-item",
                        onclick: move |_| {
                            match &action {
                                MenuAction::Command(cmd) => cxt_command_handler.call(Some(cmd.clone())),
                                MenuAction::View(run) => run.call(()),
                            }
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
