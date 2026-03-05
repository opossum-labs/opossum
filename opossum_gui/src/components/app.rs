// --- Common imports ---
use crate::{
    api::delete_scenery,
    components::{
        alert_dialog::{
            AlertDialogAction, AlertDialogActions, AlertDialogCancel, AlertDialogContent,
            AlertDialogDescription, AlertDialogRoot, AlertDialogTitle,
        },
        context_menu::cx_menu::{ContextMenu, CxtCommand},
        logger::logger_component::Logger,
        menu_bar::{
            menu_bar_component::{AppCommand, MenuBar},
            project_helper::{select_folder_path, select_open_path, select_save_path},
        },
        scenery_editor::{GraphEditor, NodeEditorCommand},
        short_cuts::{PendingAction, get_action_from_event},
    },
};
use dioxus::prelude::*;
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use crate::{ProcessHandle, components::simulation::simulation_window::SimulationWindow};
#[cfg(not(target_arch = "wasm32"))]
use dioxus::desktop::{tao::window::ResizeDirection, use_window};

#[component]
pub fn App() -> Element {
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(unused_variables)]
    let backend_handle = use_context::<ProcessHandle>();
    #[cfg(not(target_arch = "wasm32"))]
    let window = use_window();
    #[cfg(not(target_arch = "wasm32"))]
    let window_for_quit = window.clone();
    #[cfg(not(target_arch = "wasm32"))]
    let mut run_simulation = use_signal(|| false);

    let mut node_editor_command: Signal<Option<NodeEditorCommand>> = use_signal(|| None);
    let node_editor_command_memo = use_memo(move || node_editor_command.read().clone());
    let node_editor_command_handler =
        EventHandler::new(move |node_editor_command_opt: Option<NodeEditorCommand>| {
            node_editor_command.set(node_editor_command_opt);
        });
    let mut cxt_command = use_signal(|| None::<CxtCommand>);

    // global signals
    let mut project_directory: Signal<Option<PathBuf>> = use_signal(|| None);
    let mut model_file_path_sig: Signal<Option<PathBuf>> = use_signal(|| None);
    let mut model_modified_sig: Signal<bool> = use_signal(|| false);

    // status for "Unsaved Changes" dialog
    let mut pending_action = use_signal(|| Option::<PendingAction>::None);
    let mut show_alert = use_signal(|| false);

    use_effect(|| {
        spawn(async move {
            let _ = delete_scenery().await;
        });
    });

    let mut execute_immediate = move |cmd: AppCommand| match cmd {
        AppCommand::NewProject => {
            node_editor_command_handler.call(Some(NodeEditorCommand::DeleteAll));
        }
        AppCommand::OpenTrigger => {
            spawn(async move {
                if let Some(path) = select_open_path().await {
                    node_editor_command_handler.call(Some(NodeEditorCommand::LoadFile(path)));
                }
            });
        }
        AppCommand::Save => {
            if let Some(path) = model_file_path_sig.read().clone() {
                node_editor_command_handler.call(Some(NodeEditorCommand::SaveFile(path)));
            } else {
                spawn(async move {
                    if let Some(path) = select_save_path().await {
                        node_editor_command_handler.call(Some(NodeEditorCommand::SaveFile(path)));
                    }
                });
            }
        }
        AppCommand::SaveAs => {
            spawn(async move {
                if let Some(path) = select_save_path().await {
                    node_editor_command_handler.call(Some(NodeEditorCommand::SaveFile(path)));
                }
            });
        }
        AppCommand::SetReportDir(path) => {
            if path.as_os_str().is_empty() {
                spawn(async move {
                    if let Some(folder) = select_folder_path().await {
                        project_directory.set(Some(folder));
                    }
                });
            } else {
                project_directory.set(Some(path));
            }
        }
        AppCommand::Quit => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                #[cfg(not(debug_assertions))]
                {
                    backend_handle.kill();
                    println!("Stopping app...")
                }
                window_for_quit.close();
            }
        }
        AppCommand::AutoLayout => {
            node_editor_command_handler.call(Some(NodeEditorCommand::AutoLayout));
        }
        AppCommand::CenterGraph => {
            node_editor_command_handler.call(Some(NodeEditorCommand::CenterGraph));
        }
        AppCommand::ZoomToFit => {
            node_editor_command_handler.call(Some(NodeEditorCommand::ZoomToFit));
        }
        AppCommand::AddNode(name) => {
            node_editor_command_handler.call(Some(NodeEditorCommand::AddNode(name)));
        }
        AppCommand::AddAnalyzer(atype) => {
            node_editor_command_handler.call(Some(NodeEditorCommand::AddAnalyzer(atype)));
        }
        AppCommand::Simulate => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                if project_directory.read().is_some() {
                    run_simulation.set(true);
                } else {
                    spawn(async move {
                        if let Some(folder) = select_folder_path().await {
                            project_directory.set(Some(folder));
                            run_simulation.set(true);
                        }
                    });
                }
            }
        }
    };
    let mut execute_immediate_for_alert = execute_immediate.clone();
    let mut process_command = move |cmd: AppCommand| match cmd {
        AppCommand::NewProject => {
            if *model_modified_sig.read() {
                pending_action.set(Some(PendingAction::NewProject));
                show_alert.set(true);
            } else {
                execute_immediate(AppCommand::NewProject);
            }
        }
        AppCommand::Quit => {
            if *model_modified_sig.read() {
                pending_action.set(Some(PendingAction::Quit));
                show_alert.set(true);
            } else {
                execute_immediate(AppCommand::Quit);
            }
        }
        AppCommand::OpenTrigger => {
            if *model_modified_sig.read() {
                pending_action.set(Some(PendingAction::OpenProject));
                show_alert.set(true);
            } else {
                execute_immediate(AppCommand::OpenTrigger);
            }
        }
        AppCommand::Save => {
            if let Some(path) = model_file_path_sig.read().clone() {
                node_editor_command_handler.call(Some(NodeEditorCommand::SaveFile(path)));
            } else {
                execute_immediate(AppCommand::SaveAs);
            }
        }
        _ => execute_immediate(cmd),
    };
    let process_command_for_menu = process_command.clone();

    let on_alert_confirm = move |_| {
        if let Some(action) = *pending_action.read() {
            match action {
                PendingAction::NewProject => execute_immediate_for_alert(AppCommand::NewProject),
                PendingAction::Quit => execute_immediate_for_alert(AppCommand::Quit),
                PendingAction::OpenProject => execute_immediate_for_alert(AppCommand::OpenTrigger),
            }
        }
        pending_action.set(None);
        show_alert.set(false);
    };

    let on_alert_cancel = move |_| {
        pending_action.set(None);
        show_alert.set(false);
    };

    use_effect(move || {
        let cxt_command_val = cxt_command.read();
        if let Some(cmd) = &*(cxt_command_val) {
            match cmd {
                CxtCommand::AddRefNode(new_ref_node) => {
                    node_editor_command_handler
                        .call(Some(NodeEditorCommand::AddNodeRef(*new_ref_node)));
                }
            }
        }
    });

    #[cfg(not(target_arch = "wasm32"))]
    rsx! {
        div {
            class: "app-container",
            tabindex: 0,
            onkeydown: move |e| {
                if let Some(action) = get_action_from_event(&e) {
                    process_command(AppCommand::from(action));
                }
            },
            div {
                class: "resize-handle-top",
                onmousedown: {
                    let window = window.clone();
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::North);
                    }
                },
            }
            div {
                class: "resize-handle-bottom",
                onmousedown: {
                    let window = window.clone();
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::South);
                    }
                },
            }
            div {
                class: "resize-handle-left",
                onmousedown: {
                    let window = window.clone();
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::West);
                    }
                },
            }
            div {
                class: "resize-handle-right",
                onmousedown: {
                    let window = window.clone();
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::East);
                    }
                },
            }
            div {
                class: "resize-handle-top-left",
                onmousedown: {
                    let window = window.clone();
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::NorthWest);
                    }
                },
            }
            div {
                class: "resize-handle-top-right",
                onmousedown: {
                    let window = window.clone();
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::NorthEast);
                    }
                },
            }
            div {
                class: "resize-handle-bottom-left",
                onmousedown: {
                    let window = window.clone();
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::SouthWest);
                    }
                },
            }
            div {
                class: "resize-handle-bottom-right",
                onmousedown: {
                    move |_| {
                        let _ = window.drag_resize_window(ResizeDirection::SouthEast);
                    }
                },
            }
            CommonAppLayout {
                cxt_command_handler: EventHandler::new(move |cxt_cmd_opt: Option<CxtCommand>| {
                    cxt_command.set(cxt_cmd_opt);
                }),
                on_menu_action: process_command_for_menu,
                model_file_path_sig,
                model_file_path_handler: EventHandler::new(move |path_opt: Option<PathBuf>| {
                    model_file_path_sig.set(path_opt);
                }),
                model_modified_sig,
                model_modified_handler: EventHandler::new(move |is_modified: bool| {
                    model_modified_sig.set(is_modified);
                }),
                node_editor_command: node_editor_command_memo,
                node_editor_command_handler,
                show_alert,
                on_alert_confirm,
                on_alert_cancel,
            }
        }
        SimulationWindow { show_simulation: run_simulation, project_directory }
    }

    // #[cfg(target_arch = "wasm32")]
    // rsx! {
    //     div {
    //         class: "app-container",
    //         tabindex: 0,
    //         onkeydown: move |e| {
    //             if let Some(action) = get_action_from_event(&e) {
    //                 process_command(AppCommand::from(action));
    //             }
    //         },
    //         CommonAppLayout {
    //             cxt_command,
    //             on_menu_action: process_command,
    //             project_directory,
    //             model_file_path,
    //             model_modified,
    //             node_editor_command,
    //             show_alert,
    //             on_alert_confirm,
    //             on_alert_cancel,
    //         }
    //     }
    // }
}

#[component]
fn CommonAppLayout(
    cxt_command_handler: EventHandler<Option<CxtCommand>>,
    on_menu_action: EventHandler<AppCommand>,
    model_file_path_sig: ReadSignal<Option<PathBuf>>,
    model_file_path_handler: EventHandler<Option<PathBuf>>,
    model_modified_handler: EventHandler<bool>,
    model_modified_sig: ReadSignal<bool>,
    node_editor_command: Memo<Option<NodeEditorCommand>>,
    node_editor_command_handler: EventHandler<Option<NodeEditorCommand>>,
    show_alert: Signal<bool>,
    on_alert_confirm: EventHandler<MouseEvent>,
    on_alert_cancel: EventHandler<MouseEvent>,
) -> Element {
    let mut height = use_signal(|| 100.0);
    let mut dragging = use_signal(|| false);
    let mut last_y = use_signal(|| 0.0);

    let on_mousemove = {
        move |evt: MouseEvent| {
            if *dragging.read() {
                let height_val = *height.read();
                let dy = evt.client_coordinates().y - *last_y.read();
                height.set((height_val - dy).max(100.0));
                last_y.set(evt.client_coordinates().y);
            }
        }
    };
    let on_mouseup = { move |_| dragging.set(false) };
    let on_mousedown = {
        move |evt: f64| {
            dragging.set(true);
            last_y.set(evt);
        }
    };

    rsx! {
        ContextMenu { cxt_command_handler }
        div {
            class: "container-fluid text-bg-dark",
            onmousemove: on_mousemove,
            onmouseup: on_mouseup,
            div { class: "row",
                div { class: "col",
                    MenuBar {
                        model_file_path_sig,
                        model_modified_sig,
                        on_menu_action,
                    }
                }
            }
            GraphEditor {
                command: node_editor_command,
                node_editor_command_handler,
                model_modified_handler,
                model_modified_sig,
                model_file_path_sig,
                model_file_path_handler,
            }
            Logger { drag_handler: on_mousedown, height }
        }
        AlertDialogRoot {
            open: show_alert(),
            on_open_change: move |v: bool| show_alert.set(v),
            AlertDialogContent {
                AlertDialogTitle { "Unsaved Changes" }
                AlertDialogDescription { "You have unsaved changes. Do you really want to proceed and discard them?" }
                AlertDialogActions {
                    AlertDialogCancel { on_click: on_alert_cancel, "No" }
                    AlertDialogAction { on_click: on_alert_confirm, "Yes" }
                }
            }
        }
    }
}
