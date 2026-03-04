#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::scenery_editor::{
    GraphState,
    edges::edges_component::{
        EdgeCreation, EdgeCreationComponent, EdgesComponent, NewEdgeCreationStart,
    },
    graph_editor::{
        graph_workspace::{
            GraphsWorkspaceAction, GraphsWorkspaceState, WorkSpaceSignalHandlers,
            use_workspace_processor,
        },
        hooks::{
            use_drag, use_drag_end, use_on_key_down, use_on_mouse_down, use_on_resize, use_zoom,
        },
    },
    nodes::Nodes,
};
use dioxus::{html::geometry::euclid::default::{Point2D, Rect, Size2D}, prelude::*};
use dioxus_primitives::tabs::{TabContent, TabList, TabTrigger, Tabs};
use opossum_core::{prelude::*, types::api_types::NewRefNode};
use serde_json::Value;
use std::{path::PathBuf, time::Instant};
use uuid::Uuid;
#[derive(Debug, Clone, PartialEq)]
pub enum NodeEditorCommand {
    DeleteAll,
    AddNode(String),
    AddNodeRef(NewRefNode),
    AddAnalyzer(AnalyzerType),
    LoadFile(PathBuf),
    SaveFile(PathBuf),
    AutoLayout,
    CenterGraph,
    ZoomToFit,
}

#[derive(Clone, Copy)]
pub struct EditorState {
    pub drag_status: Signal<DragStatus>,
    pub edge_in_creation: Signal<Option<EdgeCreation>>,
    pub zoom: Signal<f64>,
    pub shift: Signal<Point2D<f64>>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            drag_status: Signal::<DragStatus>::default(),
            edge_in_creation: Signal::<Option<EdgeCreation>>::default(),
            zoom: Signal::new(1.),
            shift: Signal::<Point2D<f64>>::default(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum DragStatus {
    #[default]
    None,
    Graph,
    Node(Uuid, Point2D<f64>), // stores also old position before drag.
    Edge(NewEdgeCreationStart),
}

#[component]
pub fn GraphEditor(
    command: Memo<Option<NodeEditorCommand>>,
    node_editor_command_handler: EventHandler<Option<NodeEditorCommand>>,
    model_modified_sig: ReadSignal<bool>,
    model_modified_handler: EventHandler<bool>,
    model_file_path_sig: ReadSignal<Option<PathBuf>>,
    model_file_path_handler: EventHandler<Option<PathBuf>>,
) -> Element {
    let mut workspace = use_signal(|| GraphsWorkspaceState::default());
    use_context_provider(|| workspace);
    let root_graph_id = use_memo(move || *workspace.read().root_scenery_id.read());

    let workspace_handlers = WorkSpaceSignalHandlers::new(workspace);
    use_context_provider(|| workspace_handlers);

    let workspace_processor = use_workspace_processor(
        workspace.into(),
        root_graph_id.into(),
        workspace_handlers,
        model_file_path_handler,
    );

    let active_tab = use_memo(move || {
        workspace
            .read()
            .active_tab
            .read()
            .map_or_else(|| Uuid::nil(), |t| t)
    });

    use_effect(move || {
        let cmd = command.read().clone();
        if let Some(command) = cmd {
            match command {
                NodeEditorCommand::DeleteAll => {
                    workspace_processor.send(GraphsWorkspaceAction::DeleteRootScenery);
                    workspace_processor.send(GraphsWorkspaceAction::GetRootSceneryId);
                }
                NodeEditorCommand::AddNode(node_type) => {
                    workspace_processor.send(GraphsWorkspaceAction::AddOpticNode {
                        node_type: node_type.clone(),
                        graph_id: active_tab(),
                    });
                }
                NodeEditorCommand::AddNodeRef(new_ref_node) => {
                    workspace_processor.send(GraphsWorkspaceAction::AddOpticReference {
                        new_ref_node: new_ref_node.clone(),
                        graph_id: active_tab(),
                    });
                }
                NodeEditorCommand::AddAnalyzer(analyzer_type) => {
                    workspace_processor.send(GraphsWorkspaceAction::AddAnalyzer {
                        analyzer_type: analyzer_type.clone(),
                        graph_id: active_tab(),
                    });
                }
                NodeEditorCommand::AutoLayout => {
                    workspace_processor.send(GraphsWorkspaceAction::OptimizeLayout {
                        graph_id: active_tab(),
                    });
                    workspace_processor.send(GraphsWorkspaceAction::ZoomToFit {
                        graph_id: active_tab(),
                        save_changes: true,
                    });
                }
                NodeEditorCommand::CenterGraph => {
                    workspace_processor.send(GraphsWorkspaceAction::CenterGraph {
                        graph_id: active_tab(),
                        save_changes: true,
                    });
                }
                NodeEditorCommand::ZoomToFit => {
                    workspace_processor.send(GraphsWorkspaceAction::ZoomToFit {
                        graph_id: active_tab(),
                        save_changes: true,
                    });
                }
                NodeEditorCommand::LoadFile(path) => {
                    workspace_processor.send(GraphsWorkspaceAction::LoadFromFile(path.to_owned()));
                }
                NodeEditorCommand::SaveFile(path) => {
                    workspace_processor.send(GraphsWorkspaceAction::SaveToFile(path.to_owned()));
                }
            }
            node_editor_command_handler.call(None);
        }
    });

    let current_mouse_pos = use_signal(Point2D::<f64>::default);

    use_effect(move || {
        workspace_processor.send(GraphsWorkspaceAction::GetRootSceneryId);
    });

    use_effect(move || {
        let is_unsaved = *workspace.read().needs_saving.read();
        if *model_modified_sig.peek() != is_unsaved {
            model_modified_handler.call(is_unsaved);
        }
    });

    // let active_node_opt = use_memo(move || {
    //     graph_state
    //         .read()
    //         .graph_store
    //         .read()
    //         .get_active_node()
    //         .map(|n| (n.node_type().clone(), n.id()))
    // });
    let onmouseleave_handler = use_drag_end(workspace);
    let onkeydownhandler = use_on_key_down(current_mouse_pos, workspace);
    let graph_editor_content_container_id = "graphEditorContentContainer";
    let onresizehandler = use_on_resize(workspace, graph_editor_content_container_id.to_string());


    rsx! {
        div { class: "row main-content-row",
            div { style: "min-width:256px;", class: "col-2 sidebar",
                {"nothing"}
                        // //NodeConfigEditor { active_node_opt, model_modified_handler }
            }
            div {
                class: "col px-0 graph-editor-container",
                tabindex: 0,
                onkeydown: onkeydownhandler,
                onmouseleave: onmouseleave_handler,
                Tabs {
                    class: "editor-tabs",
                    value: active_tab.read().as_simple().to_string(),
                    on_value_change: move |v: String| {
                        if let Ok(new_id) = Uuid::parse_str(&v) {
                            workspace_handlers.set_active_tab.call(Some(new_id));
                        }
                    },
                    {
                        let tabs = workspace.read().tabs.read().clone();
                        rsx! {
                            TabList { class: "editor-tab-list",
                                for (i , (id , graph_state)) in tabs.iter().enumerate() {
                                    TabTrigger {
                                        key: "{id.as_simple().to_string()}",
                                        value: id.as_simple().to_string(),
                                        index: i,
                                        class: if active_tab() == *id { "editor-tab active-tab" } else { "editor-tab" },
                                        div { class: "tab-inner",
                                            span { {graph_state.read().name.clone()} }
                                            if *id != root_graph_id() {
                                                button {
                                                    class: "tab-close",
                                                    onclick: {
                                                        let id_copy = *id;
                                                        move |_| workspace_handlers.remove_tab.call(id_copy)
                                                    },

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
                                for (i , (id , graph_state)) in tabs.iter().enumerate() {
                                    TabContent {
                                        class: "tab-content",
                                        key: "{id.as_simple().to_string()}",
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

#[component]
pub fn GraphViewEditor(
    onmouseup_handler: EventHandler<Event<MouseData>>,
    model_modified_sig: ReadSignal<bool>,
    model_modified_handler: EventHandler<bool>,
    model_file_path_sig: ReadSignal<Option<PathBuf>>,
    model_file_path_handler: EventHandler<Option<PathBuf>>,
    current_mouse_pos: Signal<Point2D<f64>>,
    graph_state: Signal<GraphState>,
) -> Element {
    let last_auxiliary_click = use_signal(|| Option::<Instant>::None);
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let editor_state = graph_state.read().editor_state;
    let graph_store = graph_state.read().graph_store;
    let graph_id = graph_state.read().id;

    use_context_provider(|| graph_state);
    use_context_provider(|| editor_state);
    use_context_provider(|| graph_store);
    let onwheel_handler = use_zoom();
    let onmousemove_handler = use_drag(current_mouse_pos);
    let onmousedown_handler = use_on_mouse_down(current_mouse_pos, last_auxiliary_click);

    let shift = use_memo(move || *editor_state.read().shift.read());
    let zoom = use_memo(move || *editor_state.read().zoom.read());

    use_effect(move || {
        workspace_processor.send(GraphsWorkspaceAction::CenterGraph {
            graph_id,
            save_changes: false,
        });
    });

    rsx! {
        div {
            class: "graph-editor",
            id: format!("editor_{}", graph_id.as_simple()),
            draggable: false,

            onwheel: onwheel_handler,
            onmousedown: onmousedown_handler,
            onmouseup: move |e| onmouseup_handler.call(e),
            onmousemove: onmousemove_handler,
            div {
                draggable: false,
                style: format!(
                    "transform-origin: 0 0; transform: translate({}px, {}px) scale({});",
                    shift().x,
                    shift().y,
                    zoom(),
                ),
                Nodes { graph_store, graph_id }
                svg {
                    width: "100%",
                    height: "100%",
                    overflow: "visible",
                    tabindex: 0,
                    {
                        rsx! {
                            EdgesComponent {}
                            EdgeCreationComponent {}
                        }
                    }
                }
            }
        }
    }
}
