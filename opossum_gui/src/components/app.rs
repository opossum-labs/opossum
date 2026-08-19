// --- Common imports ---
use crate::{
    APP_CONFIG,
    components::{
        context_menu::cx_menu::{ContextMenu, CxtCommand},
        logger::logger_component::Logger,
        material_catalog::MaterialCatalog,
        menu_bar::{
            menu_bar_component::{AppCommand, MenuBar},
            project_helper::{select_open_path, select_save_path},
        },
        primitives::alert_dialog::{
            AlertDialog, AlertDialogAction, AlertDialogActions, AlertDialogCancel,
            AlertDialogDescription, AlertDialogTitle,
        },
        scenery_editor::{GraphEditor, NodeEditorCommand},
        settings_dialog::SettingsDialog,
        short_cuts::{PendingAction, SHORTCUTS, Shortcut},
    },
};
use dioxus::prelude::*;
use dioxus_primitives::alert_dialog::AlertDialogContent;
use opossum_registry::AssetLoader;
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use crate::{ProcessHandle, components::simulation::simulation_window::SimulationWindow};
#[cfg(not(target_arch = "wasm32"))]
use dioxus::desktop::{tao::window::ResizeDirection, use_window};

/// Registers the app's keyboard shortcuts on the `document`, so they fire no matter where DOM focus
/// currently is, and routes each match through `process_command`.
///
/// Why not a plain element `onkeydown`: a DOM keydown only reaches an element handler while that
/// element (or a descendant) holds focus. A properties-panel re-render can remove the focused input
/// (e.g. undoing a dropdown variant change swaps out the sub-editor), which drops focus to `<body>` -
/// outside any app element - so element-scoped shortcuts silently die until the user clicks back in.
/// A `document`-level listener sidesteps that entirely.
///
/// The listener (see [`build_shortcut_listener_js`]) suppresses the browser default only for our own
/// combos and posts the matched shortcut's index back over the `dioxus.send` channel; here we map that
/// index to its [`ShortCutAction`] and dispatch it.
fn use_global_shortcuts(process_command: impl FnMut(AppCommand) + Clone + 'static) {
    let shortcuts: Vec<&'static Shortcut> = SHORTCUTS.values().collect();
    use_future(move || {
        let shortcuts = shortcuts.clone();
        let mut process = process_command.clone();
        async move {
            let mut eval = dioxus::document::eval(&build_shortcut_listener_js(&shortcuts));
            while let Ok(value) = eval.recv::<serde_json::Value>().await {
                if let Some(index) = value.as_u64()
                    && let Ok(idx) = usize::try_from(index)
                    && let Some(shortcut) = shortcuts.get(idx)
                {
                    process(AppCommand::from(shortcut.action));
                }
            }
        }
    });
}

/// Builds the JS that installs the `document` keydown listener for [`use_global_shortcuts`].
///
/// Each shortcut is emitted as a small object `{i, ctrl, shift, alt, key}` where `i` is its index in
/// `shortcuts` (so the JS never needs to know the actual action, only its number). On a keydown the
/// listener ignores anything without a Ctrl/Cmd or Alt modifier, then looks for a combo whose modifiers
/// and (case-insensitive) key all match; on a hit it calls `preventDefault` (so native handling like a
/// text field's own Ctrl+Z is suppressed for our combos, while Ctrl+C/V/A stay native) and sends the
/// index `i` back to Rust via `dioxus.send`.
fn build_shortcut_listener_js(shortcuts: &[&Shortcut]) -> String {
    let combos = shortcuts
        .iter()
        .enumerate()
        .map(|(i, s)| {
            format!(
                "{{i:{i},ctrl:{},shift:{},alt:{},key:\"{}\"}}",
                s.ctrl_or_meta,
                s.shift,
                s.alt,
                s.key.to_uppercase()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    r"
const combos=[__COMBOS__];
document.addEventListener('keydown', function(e){
    const ctrl = e.ctrlKey || e.metaKey;          // treat Ctrl and Cmd as the same modifier
    const key = (e.key || '').toUpperCase();
    if(!ctrl && !e.altKey && !key.startsWith('F')) return; // allow modifier combos and F-keys
    for(const c of combos){
        if(c.ctrl===ctrl && c.shift===e.shiftKey && c.alt===e.altKey && c.key===key){
            e.preventDefault();                    // suppress the browser default for our combos only
            dioxus.send(c.i);                      // forward the matched shortcut's index to Rust
            return;
        }
    }
});
"
    .replace("__COMBOS__", &combos)
}

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

    let loader = use_signal(|| {
        let registry_path = PathBuf::from("./target/test_registry");
        AssetLoader::new(registry_path)
    });
    let mut node_editor_command: Signal<Option<NodeEditorCommand>> = use_signal(|| None);
    let node_editor_command_handler =
        EventHandler::new(move |node_editor_command_opt: Option<NodeEditorCommand>| {
            node_editor_command.set(node_editor_command_opt);
        });
    let mut cxt_command = use_signal(|| None::<CxtCommand>);

    // Define global signals
    let mut model_file_path: Signal<Option<PathBuf>> = use_signal(|| None);
    let mut model_modified_sig: Signal<bool> = use_signal(|| false);

    // Status for "Unsaved Changes" dialog
    let mut pending_action = use_signal(|| Option::<PendingAction>::None);
    let mut show_alert = use_signal(|| false);
    let mut show_settings = use_signal(|| false);
    let mut show_material_catalog = use_signal(|| false);

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
            if let Some(path) = model_file_path.read().clone() {
                node_editor_command_handler.call(Some(NodeEditorCommand::SaveFile(path)));
            } else {
                #[cfg(not(target_arch = "wasm32"))]
                spawn(async move {
                    if let Some(path) = select_save_path().await {
                        node_editor_command_handler.call(Some(NodeEditorCommand::SaveFile(path)));
                    }
                });

                #[cfg(target_arch = "wasm32")]
                {
                    let default_path = PathBuf::from("project.opm");
                    node_editor_command_handler
                        .call(Some(NodeEditorCommand::SaveFile(default_path)));
                }
            }
        }
        AppCommand::SaveAs => {
            #[cfg(not(target_arch = "wasm32"))]
            spawn(async move {
                if let Some(path) = select_save_path().await {
                    node_editor_command_handler.call(Some(NodeEditorCommand::SaveFile(path)));
                }
            });

            #[cfg(target_arch = "wasm32")]
            {
                let current_filename = model_file_path
                    .read()
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| PathBuf::from("project.opm"));
                node_editor_command_handler
                    .call(Some(NodeEditorCommand::SaveFile(current_filename)));
            }
        }
        AppCommand::Refresh => {
            node_editor_command_handler.call(Some(NodeEditorCommand::Refresh));
        }
        AppCommand::Settings => {
            show_settings.set(true);
        }
        AppCommand::Quit => {
            // Save config file (even if not changed for automatic migration of file format)
            if let Err(e) = APP_CONFIG.read().to_file() {
                eprintln!("Error saving AppConfig on exit: {e}");
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                #[cfg(not(debug_assertions))]
                {
                    backend_handle.kill();
                    println!("Stopping app...");
                }
                window_for_quit.close();
            }
        }
        AppCommand::AutoLayout => {
            node_editor_command_handler.call(Some(NodeEditorCommand::AutoLayout));
        }
        AppCommand::Undo => {
            node_editor_command_handler.call(Some(NodeEditorCommand::Undo));
        }
        AppCommand::Redo => {
            node_editor_command_handler.call(Some(NodeEditorCommand::Redo));
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
                run_simulation.set(true);
            }
        }
        AppCommand::OpenMaterialCatalog => {
            show_material_catalog.set(true);
        }
    };

    let mut execute_immediate_for_alert = execute_immediate.clone();
    let process_command = move |cmd: AppCommand| match cmd {
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
            if let Some(path) = model_file_path.read().clone() {
                node_editor_command_handler.call(Some(NodeEditorCommand::SaveFile(path)));
            } else {
                execute_immediate(AppCommand::SaveAs);
            }
        }
        _ => execute_immediate(cmd),
    };
    let process_command_for_menu = process_command.clone();

    // Keyboard shortcuts are handled globally (on `document`, not a focusable element) so they keep
    // working after a panel re-render drops DOM focus - see `use_global_shortcuts`.
    use_global_shortcuts(process_command);

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
                CxtCommand::ConvertToGroup { nodes, graph_id } => {
                    node_editor_command_handler.call(Some(NodeEditorCommand::ConvertToGroup {
                        nodes: nodes.clone(),
                        graph_id: *graph_id,
                    }));
                }
                CxtCommand::MapNodePort {
                    port_type,
                    group_port_name,
                    mapped_node_port_name,
                    mapped_node_id,
                    group_id,
                } => node_editor_command_handler.call(Some(NodeEditorCommand::MapNodePort {
                    port_type: *port_type,
                    group_port_name: group_port_name.clone(),
                    mapped_node_port_name: mapped_node_port_name.clone(),
                    mapped_node_id: *mapped_node_id,
                    group_id: *group_id,
                })),
                CxtCommand::RemovePortMap {
                    group_id,
                    group_port_name,
                    port_type,
                } => {
                    node_editor_command_handler.call(Some(NodeEditorCommand::RemovePortMap {
                        group_id: *group_id,
                        group_port_name: group_port_name.clone(),
                        port_type: *port_type,
                    }));
                }
                CxtCommand::JumpToMappedPort {
                    mapped_node_id,
                    parent,
                } => {
                    node_editor_command_handler.call(Some(NodeEditorCommand::JumpToMappedPort {
                        mapped_node_id: *mapped_node_id,
                        parent: parent.clone(),
                    }));
                }
            }
        }
    });

    #[cfg(not(target_arch = "wasm32"))]
    rsx! {
        div { class: "app-container", tabindex: 0,
            // Keyboard shortcuts are handled by the document-level listener installed above, not here -
            // an element `onkeydown` only fires while focus is inside it, which breaks after a panel
            // re-render drops focus to `<body>`.
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
                model_file_path,
                model_file_path_handler: EventHandler::new(move |path_opt: Option<PathBuf>| {
                    model_file_path.set(path_opt);
                }),
                model_modified_sig,
                model_modified_handler: EventHandler::new(move |is_modified: bool| {
                    model_modified_sig.set(is_modified);
                }),
                node_editor_command,
                node_editor_command_handler,
                show_alert,
                on_alert_confirm,
                on_alert_cancel,
            }
        }
        SimulationWindow { show_simulation: run_simulation, model_file_path }
        SettingsDialog { show: show_settings }
        MaterialCatalog { open: show_material_catalog, loader }
    }

    #[cfg(target_arch = "wasm32")]
    rsx! {
        div { class: "app-container", tabindex: 0,
            CommonAppLayout {
                cxt_command_handler: EventHandler::new(move |cxt_cmd_opt: Option<CxtCommand>| {
                    cxt_command.set(cxt_cmd_opt);
                }),
                on_menu_action: process_command_for_menu,
                model_file_path,
                model_file_path_handler: EventHandler::new(move |path_opt: Option<PathBuf>| {
                    model_file_path.set(path_opt);
                }),
                model_modified_sig,
                model_modified_handler: EventHandler::new(move |is_modified: bool| {
                    model_modified_sig.set(is_modified);
                }),
                node_editor_command,
                node_editor_command_handler,
                show_alert,
                on_alert_confirm,
                on_alert_cancel,
            }
        }
        SettingsDialog { show: show_settings }
    }
}

#[component]
fn CommonAppLayout(
    cxt_command_handler: EventHandler<Option<CxtCommand>>,
    on_menu_action: EventHandler<AppCommand>,
    model_file_path: ReadSignal<Option<PathBuf>>,
    model_file_path_handler: EventHandler<Option<PathBuf>>,
    model_modified_handler: EventHandler<bool>,
    model_modified_sig: ReadSignal<bool>,
    node_editor_command: ReadSignal<Option<NodeEditorCommand>>,
    node_editor_command_handler: EventHandler<Option<NodeEditorCommand>>,
    show_alert: Signal<bool>,
    on_alert_confirm: EventHandler<MouseEvent>,
    on_alert_cancel: EventHandler<MouseEvent>,
) -> Element {
    info!("🔄 Render: App::CommonAppLayout");

    // 5. Handle catalog actions (Edit existing vs Create new)
    // let on_catalog_action = use_callback(move |event: MaterialCatalogEvent| {
    //     match event {
    //         MaterialCatalogEvent::MaterialAdded => {},
    //         MaterialCatalogEvent::MaterialDeleted => {}
    //     }
    // });

    // --- GUI Layout Drag Logic ---
    let mut root_tab_open = use_signal(|| true);
    let root_tab_open_handler = EventHandler::<bool>::new(move |b| root_tab_open.set(b));
    let mut height = use_signal(|| 100.0);
    let mut dragging = use_signal(|| false);
    let mut last_y = use_signal(|| 0.0);

    let on_mousemove = move |evt: MouseEvent| {
        if *dragging.read() {
            let height_val = *height.read();
            let dy = evt.client_coordinates().y - *last_y.read();
            height.set((height_val - dy).max(100.0));
            last_y.set(evt.client_coordinates().y);
        }
    };
    let on_mouseup = move |_| dragging.set(false);
    let on_mousedown = move |evt: f64| {
        dragging.set(true);
        last_y.set(evt);
    };

    rsx! {
        document::Title { "OPOSSUM" }
        ContextMenu { cxt_command_handler }
        div {
            class: "container-fluid text-bg-dark",
            onmousemove: on_mousemove,
            onmouseup: on_mouseup,
            div { class: "row",
                div { class: "col",
                    MenuBar {
                        model_file_path,
                        model_modified_sig,
                        on_menu_action,
                        root_tab_open,
                    }
                }
            }
            GraphEditor {
                command: node_editor_command,
                node_editor_command_handler,
                model_modified_sig,
                model_modified_handler,
                model_file_path,
                model_file_path_handler,
                root_tab_open_handler,
            }
            Logger { drag_handler: on_mousedown, height }
        }
        AlertDialog {
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
