#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::{
    node_editor::NodeConfigEditor,
    scenery_editor::{
        ActiveNode, NodeEditorCommand,
        graph_editor::{
            GraphViewEditor,
            graph_workspace::{
                GraphsWorkspaceAction, GraphsWorkspaceState, WorkSpaceSignalHandlers,
                use_workspace_processor, workspace_action::use_node_editor_command,
            },
            hooks::{use_drag_end, use_on_key_down, use_on_resize},
        },
    },
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use dioxus_primitives::tabs::{TabContent, TabList, TabTrigger, Tabs};
use std::path::PathBuf;
use uuid::Uuid;

#[component]
pub fn GraphEditor(
    command: Memo<Option<NodeEditorCommand>>,
    node_editor_command_handler: EventHandler<Option<NodeEditorCommand>>,
    model_modified_sig: ReadSignal<bool>,
    model_modified_handler: EventHandler<bool>,
    model_file_path_sig: ReadSignal<Option<PathBuf>>,
    model_file_path_handler: EventHandler<Option<PathBuf>>,
) -> Element {
    let workspace = use_signal(GraphsWorkspaceState::default);
    use_context_provider(|| workspace);
    let root_graph_id = use_memo(move || *workspace.read().root_scenery_id.read());

    let workspace_handlers = WorkSpaceSignalHandlers::new(workspace);
    use_context_provider(|| workspace_handlers);

    let workspace_processor = use_workspace_processor(
        workspace.into(),
        root_graph_id,
        workspace_handlers,
        model_file_path_handler,
    );

    let active_tab = use_memo(move || *workspace.read().active_tab.read());

    use_effect(move || {
        use_node_editor_command(
            node_editor_command_handler,
            active_tab.into(),
            workspace_processor,
            command,
        );
    });

    use_effect(move || {
        if let Some(path) = &*model_file_path_sig.read() && let Some(os_fname)  =path.file_stem() && let Some(fname) = os_fname.to_str(){
            let name = fname.to_string();
            let id = root_graph_id();
            workspace_processor.send(GraphsWorkspaceAction::SetNodeName { name, graph_id: id, node_id: id, needs_saving: false});
        }
    });

    use_effect(move || {
        if *root_graph_id.peek() == Uuid::nil(){
            let scenery_name = if let Some(path) = &*model_file_path_sig.peek()&& let Some(os_fname)  =path.file_stem() && let Some(fname) = os_fname.to_str(){
                fname.to_string()
            }
            else{
                "unsaved".to_string()
            };
            workspace_processor.send(GraphsWorkspaceAction::AddRootSceneryTab{name: scenery_name});
        }
    });

    let current_mouse_pos = use_signal(Point2D::<f64>::default);


    use_effect(move || {
        let is_unsaved = *workspace.read().needs_saving.read();
        if *model_modified_sig.peek() != is_unsaved {
            model_modified_handler.call(is_unsaved);
        }
    });

    let active_node_opt = use_memo(move || {
        let read_workspace = workspace.read();
        let active_tab = *read_workspace.active_tab.read();

        read_workspace
            .get_graph_store_read(active_tab)
            .and_then(|g| {
                g.read().get_active_node().map(|n| ActiveNode {
                    node_id: n.id(),
                    graph_id: active_tab,
                    node_type: n.node_type().clone(),
                })
            })
    });
    let onmouseleave_handler = use_drag_end(workspace);
    let onkeydownhandler = use_on_key_down(current_mouse_pos, workspace);
    let graph_editor_content_container_id = "graphEditorContentContainer";
    let onresizehandler = use_on_resize(workspace, graph_editor_content_container_id.to_string());

    rsx! {
        div { class: "row main-content-row",
            div { style: "min-width:256px;", class: "col-2 sidebar",
                NodeConfigEditor { active_node_opt, model_modified_handler }
            }
            div {
                class: "col px-0 graph-editor-container",
                tabindex: 0,
                onkeydown: onkeydownhandler,
                onmouseleave: onmouseleave_handler,

                Tabs {
                    class: "editor-tabs",
                    value: (*workspace.read().active_tab.read()).as_simple().to_string(),
                    on_value_change: move |v: String| {
                        if let Ok(new_id) = Uuid::parse_str(&v) {
                            workspace_handlers.workspace.set_active_tab(new_id);
                        }
                    },
                    {
                        let tabs = workspace.read().tabs.read().clone();
                        let tab_order = workspace.read().tab_order.read().clone();
                        rsx! {
                            TabList { class: "editor-tab-list",
                                for (i , id) in tab_order.iter().enumerate() {
                                    if let Some(graph_state) = tabs.get(id) {
                                        TabTrigger {
                                            key: "{id.as_simple().to_string()}",
                                            value: id.as_simple().to_string(),
                                            index: i,
                                            class: if active_tab() == *id { "editor-tab active-tab" } else { "editor-tab" },
                                            div { class: "tab-inner",
                                                span { {graph_state.read().graph_info.name.clone()} }
                                                if *id != root_graph_id() {
                                                    button {
                                                        class: "tab-close",
                                                        onclick: {
                                                            let id_copy = *id;
                                                            move |e: MouseEvent| {
                                                                e.stop_propagation();
                                                                workspace_handlers.workspace.remove_tabs(vec![id_copy]);
                                                            }
                                                        },

                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                div { class: "editor-tab-filler" }
                            }
                            div {
                                id: graph_editor_content_container_id,
                                class: "graph-editor-tab-content",
                                onresize: move |_| onresizehandler.call(()),
                                for (i , id) in tab_order.iter().enumerate() {
                                    if let Some(graph_state) = tabs.get(id) {
                                        TabContent {
                                            key: "{id.as_simple().to_string()}",
                                            class: "tab-content",
                                            value: id.as_simple().to_string(),
                                            index: i,
                                            GraphViewEditor {
                                                onmouseup_handler: EventHandler::new(use_drag_end(workspace)),
                                                model_modified_sig,
                                                model_modified_handler,
                                                model_file_path_sig,
                                                model_file_path_handler,
                                                current_mouse_pos,
                                                graph_state: *graph_state,
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
