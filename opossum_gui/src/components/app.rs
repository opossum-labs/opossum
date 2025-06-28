use std::path::{Path, PathBuf};

use crate::{
    api,
    components::{
        context_menu::cx_menu::{ContextMenu, CxtCommand},
        logger::logger_component::Logger,
        menu_bar::menu_bar_component::{MenuBar, MenuSelection},
        node_editor::NodeEditor,
        scenery_editor::{graph_editor::NodeEditorCommand, node::NodeElement, GraphEditor},
    },
    HTTP_API_CLIENT, OPOSSUM_UI_LOGS,
};
use dioxus::prelude::*;
use opossum_backend::{create_data_dir, create_report_and_data_files};

pub async fn analyze_setup(path: PathBuf) {
    match api::analyze(&HTTP_API_CLIENT()).await {
        Ok(reports) => {
            if create_data_dir(&path).is_err() {
                OPOSSUM_UI_LOGS
                    .write()
                    .add_log("Error while creating report-data directory");
            };
            // create_dot_file(&opossum_args.report_directory, document.scenery())?;
            for report in reports.iter().enumerate() {
                if create_report_and_data_files(&path, report.1, report.0).is_err() {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log("Error while creating report and data files");
                };
            }
        }
        Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
    }
}

#[component]
pub fn App() -> Element {
    let menu_item_selected = use_signal(|| None::<MenuSelection>);
    let mut node_editor_command = use_signal(|| None::<NodeEditorCommand>);
    let cxt_command = use_signal(|| None::<CxtCommand>);
    let selected_node = use_signal(|| None::<NodeElement>);
    let project_directory = use_signal(|| Path::new("./").to_path_buf());

    use_effect(move || {
        let cxt_command = cxt_command.read();
        if let Some(cxt_command) = &*(cxt_command) {
            match cxt_command {
                CxtCommand::AddRefNode(new_ref_node) => node_editor_command
                    .set(Some(NodeEditorCommand::AddNodeRef(new_ref_node.clone()))),
            }
        }
    });
    use_effect(move || {
        let menu_item = menu_item_selected.read();
        if let Some(menu_item) = &*(menu_item) {
            match menu_item {
                MenuSelection::AddNode(node_selected) => {
                    node_editor_command
                        .set(Some(NodeEditorCommand::AddNode(node_selected.clone())));
                }
                MenuSelection::AddAnalyzer(analyzer_selected) => {
                    node_editor_command.set(Some(NodeEditorCommand::AddAnalyzer(
                        analyzer_selected.clone(),
                    )));
                }
                MenuSelection::AutoLayout => {
                    node_editor_command.set(Some(NodeEditorCommand::AutoLayout));
                }
                MenuSelection::NewProject => {
                    node_editor_command.set(Some(NodeEditorCommand::DeleteAll));
                }
                MenuSelection::OpenProject(path) => {
                    let path = path.to_owned();
                    node_editor_command.set(Some(NodeEditorCommand::LoadFile(path)));
                }
                MenuSelection::SaveProject(path) => {
                    let path = path.to_owned();
                    node_editor_command.set(Some(NodeEditorCommand::SaveFile(path)));
                }
                MenuSelection::WinMaximize => {
                    println!("App::Window maximize selected");
                }
                MenuSelection::WinMinimize => {
                    println!("App::Window minimize selected");
                }
                MenuSelection::WinClose => {
                    println!("App::Window close selected");
                }
                MenuSelection::RunProject => {
                    spawn(async move { analyze_setup(project_directory()).await });
                }
            }
        }
    });
    rsx! {
        ContextMenu { command: cxt_command }
        div { class: "container-fluid text-bg-dark",
            div { class: "row",
                div { class: "col",
                    MenuBar { menu_item_selected, project_directory }
                }
            }
            div { class: "row main-content-row",
                div { class: "col-2 sidebar",
                    NodeEditor { node: selected_node }
                }
                div { class: "col px-0 graph-editor-container",
                    GraphEditor {
                        command: node_editor_command,
                        node_selected: selected_node,
                    }
                }
            }
            div { class: "row footer",
                div { class: "col", Logger {} }
            }
        }
    }
}
