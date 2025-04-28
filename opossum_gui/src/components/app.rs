use dioxus::prelude::*;
use uuid::Uuid;

use crate::components::{
    logger::logger_component::Logger,
    menu_bar::menu_bar_component::{MenuBar, MenuSelection},
    node_components::{node_drag_drop_container::NodeEditorCommand, NodeEditor},
    node_property_config::node_config_menu::NodePropertyConfigMenu,
};

#[component]
pub fn App() -> Element {
    use_context_provider(|| Signal::new(Uuid::nil()));
    let menu_item_selected = use_signal(|| None::<MenuSelection>);
    let mut node_editor_command = use_signal(|| None::<NodeEditorCommand>);
    // let mut main_window = use_signal(|| None::<Rc<MountedData>>);

    use_effect(move || {
        let menu_item = menu_item_selected.read();
        if let Some(menu_item) = &*(menu_item) {
            match menu_item {
                MenuSelection::AddNode(node_selected) => {
                    println!("Node selected: {:?}", node_selected);
                    node_editor_command
                        .set(Some(NodeEditorCommand::AddNode(node_selected.clone())));
                }
                MenuSelection::AddAnalyzer(analyzer_selected) => {
                    println!("Analyzer selected: {:?}", analyzer_selected);
                    node_editor_command.set(Some(NodeEditorCommand::AddAnalyzer(
                        analyzer_selected.clone(),
                    )));
                }
                MenuSelection::NewProject => {
                    println!("New project selected");
                    node_editor_command.set(Some(NodeEditorCommand::DeleteAll));
                }
                MenuSelection::OpenProject => {
                    println!("Open project selected");
                }
                MenuSelection::SaveProject => {
                    println!("Save project selected");
                }
                MenuSelection::WinMaximize => {
                    println!("Window maximize selected");
                }
                MenuSelection::WinMinimize => {
                    println!("Window minimize selected");
                }
                MenuSelection::WinClose => {
                    println!("Window close selected");
                }
            }
        }
    });

    rsx! {
        div { class: "d-flex flex-column text-bg-dark vh-100",
            div { MenuBar { menu_item_selected } }
            div { class: "d-flex flex-row",
            div { class: "p-1", NodePropertyConfigMenu {} }
            div { class: "p-1 flex-grow-1", NodeEditor {command: node_editor_command} }
            }
            footer {
                class: "footer p-1",
                style: "background-color:rgb(119, 119, 119);",
                Logger {}
            }
        } 
    }
}
