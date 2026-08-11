#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::{
    node_editor::{AmpOverview, NodeConfigEditor},
    scenery_editor::{
        DragStatus, NodeEditorCommand, SelectedNode,
        graph_editor::{
            GraphViewEditor,
            hooks::{use_drag_end, use_on_key_down, use_on_key_up},
        },
        graph_workspace::{
            GraphStateStoreExt, GraphsWorkspaceAction, GraphsWorkspaceState,
            GraphsWorkspaceStateStoreExt, WorkSpaceSignalHandlers, use_workspace_processor,
            workspace_action::node_editor_command,
        },
    },
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use dioxus_primitives::tabs::{TabContent, TabList, TabTrigger, Tabs};
use std::path::PathBuf;
use uuid::Uuid;

#[component]
pub fn GraphEditor(
    command: ReadSignal<Option<NodeEditorCommand>>,
    node_editor_command_handler: EventHandler<Option<NodeEditorCommand>>,
    model_modified_sig: ReadSignal<bool>,
    model_modified_handler: EventHandler<bool>,
    model_file_path: ReadSignal<Option<PathBuf>>,
    model_file_path_handler: EventHandler<Option<PathBuf>>,
    root_tab_open_handler: EventHandler<bool>,
    sidebar_width: ReadSignal<f64>,
    sidebar_drag_handler: EventHandler<f64>,
) -> Element {
    let workspace = use_store(GraphsWorkspaceState::default);
    use_context_provider(|| ReadStore::from(workspace));
    let root_graph_id = use_memo(move || *workspace.root_scenery_id().read());

    let workspace_handlers = WorkSpaceSignalHandlers::new(workspace);

    let graph_editor_container_class = use_memo(move || match *workspace.drag_status().read() {
        DragStatus::Graph => "col px-0 graph-editor-container dragging".to_string(),
        _ => "col px-0 graph-editor-container".to_string(),
    });

    let workspace_processor = use_workspace_processor(
        workspace.into(),
        root_graph_id,
        workspace_handlers,
        model_file_path_handler,
    );

    let active_tab = use_memo(move || *workspace.active_tab().read());

    use_effect(move || {
        node_editor_command(
            node_editor_command_handler,
            active_tab.into(),
            workspace_processor,
            command,
        );
    });

    use_effect(move || {
        if let Some(path) = &*model_file_path.read()
            && let Some(os_fname) = path.file_stem()
            && let Some(fname) = os_fname.to_str()
        {
            let name = fname.to_string();
            let id = root_graph_id();
            workspace_processor.send(GraphsWorkspaceAction::SetNodeName {
                name,
                graph_id: id,
                node_id: id,
                needs_saving: false,
            });
        }
    });

    use_effect(move || {
        root_tab_open_handler.call(*root_graph_id.peek() == *workspace.active_tab().read());
    });

    use_effect(move || {
        if *root_graph_id.peek() == Uuid::nil() {
            let scenery_name = if let Some(path) = &*model_file_path.peek()
                && let Some(os_fname) = path.file_stem()
                && let Some(fname) = os_fname.to_str()
            {
                fname.to_string()
            } else {
                "unsaved".to_string()
            };
            workspace_processor.send(GraphsWorkspaceAction::DeleteRootScenery);
            workspace_processor
                .send(GraphsWorkspaceAction::AddRootSceneryTab { name: scenery_name });
        }
    });

    let current_mouse_in_editor_pos = use_signal(Point2D::<f64>::default);
    let ctrl_pressed = use_signal(|| false);
    let shift_pressed = use_signal(|| false);

    use_effect(move || {
        let is_unsaved = *workspace.needs_saving().read();
        if *model_modified_sig.peek() != is_unsaved {
            model_modified_handler.call(is_unsaved);
        }
    });

    let selected_nodes_memo = use_memo(move || {
        workspace
            .tabs()
            .get(active_tab())
            .map_or(Vec::<SelectedNode>::new(), |g| {
                g.graph_store().read().get_selected_nodes(active_tab())
            })
    });
    // `None` = collapsed to the icon bar alone, VS-Code style.
    let sidebar_view = use_signal(|| Some(SidebarView::NodeProperties));
    let onmouseleave_handler = use_drag_end(workspace.into(), None);
    let onkeydownhandler = use_on_key_down(
        current_mouse_in_editor_pos,
        workspace.into(),
        ctrl_pressed,
        shift_pressed,
    );
    let onkeyuphandler = use_on_key_up(ctrl_pressed, shift_pressed);

    rsx! {
        div { class: "row main-content-row",
            div {
                class: "sidebar d-flex",
                // Collapsed, the bar is only as wide as its icons; expanded, its width is whatever
                // the user dragged it to. Either way it never grows or shrinks with the window -
                // the graph editor next to it takes the remaining space.
                style: sidebar_view().map_or_else(
                    || "flex: 0 0 auto;".to_string(),
                    |_| format!("flex: 0 0 {}px;", sidebar_width()),
                ),
                SidebarViewSwitcher { view: sidebar_view }
                if let Some(view) = sidebar_view() {
                    div { class: "flex-grow-1 sidebar-view",
                        match view {
                            SidebarView::NodeProperties => rsx! {
                                NodeConfigEditor {
                                    selected_nodes_memo,
                                    model_modified_handler,
                                    workspace_processor,
                                    active_graph_id: active_tab,
                                }
                            },
                            SidebarView::AmpOverview => rsx! {
                                AmpOverview {}
                            },
                        }
                    }
                    div {
                        class: "width_resizer",
                        onmousedown: move |e: MouseEvent| {
                            sidebar_drag_handler.call(e.client_coordinates().x);
                        },
                    }
                }
            }
            div {
                class: graph_editor_container_class(),
                tabindex: 0,
                onkeydown: onkeydownhandler,
                onmouseleave: onmouseleave_handler,
                onkeyup: onkeyuphandler,

                Tabs {
                    class: "editor-tabs",
                    value: active_tab().as_simple().to_string(),
                    on_value_change: move |v: String| {
                        if let Ok(new_id) = Uuid::parse_str(&v) {
                            workspace_processor.send(GraphsWorkspaceAction::SetActiveTab(new_id));
                        }
                    },
                    {
                        let tab_order = workspace.tab_order().read().clone();
                        rsx! {
                            TabList { class: "editor-tab-list",
                                for (i , id) in tab_order.iter().enumerate() {
                                    if let Some(graph_state) = workspace.tabs().get(*id) {
                                        TabTrigger {
                                            key: "{id.as_simple().to_string()}",
                                            value: id.as_simple().to_string(),
                                            index: i,
                                            class: if active_tab() == *id { "editor-tab active-tab" } else { "editor-tab" },
                                            div { class: "tab-inner",
                                                span { {graph_state.graph_info().read().name.clone()} }
                                                if *id != root_graph_id() {
                                                    button {
                                                        class: "tab-close",
                                                        onclick: {
                                                            let id_copy = *id;
                                                            move |e: MouseEvent| {
                                                                e.stop_propagation();
                                                                workspace_processor.send(GraphsWorkspaceAction::RemoveTabs(vec![id_copy]));
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
                                id: "graphEditorContentContainer",
                                class: "graph-editor-tab-content",
                                onresize: move |_| workspace_processor.send(GraphsWorkspaceAction::GetEditorArea),
                                for (i , id) in tab_order.iter().enumerate() {
                                    if let Some(graph_state) = workspace.tabs().get(*id) {
                                        TabContent {
                                            key: "{id.as_simple().to_string()}",
                                            class: "tab-content",
                                            value: id.as_simple().to_string(),
                                            index: i,
                                            GraphViewEditor {
                                                model_modified_sig,
                                                model_modified_handler,
                                                model_file_path,
                                                model_file_path_handler,
                                                current_mouse_pos: current_mouse_in_editor_pos,
                                                graph_state,
                                                ctrl_pressed,
                                                shift_pressed,
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

/// Which of the sidebar's two views is showing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SidebarView {
    /// The existing selection-bound node/analyzer configuration.
    NodeProperties,
    /// The document-wide list of amplifying nodes.
    AmpOverview,
}
impl SidebarView {
    /// Icon and tooltip of this view's button in the switcher bar.
    const fn icon_and_title(self) -> (&'static str, &'static str) {
        match self {
            Self::NodeProperties => ("⚙", "Node properties"),
            Self::AmpOverview => ("⚡", "Amplifiers"),
        }
    }
}

/// Narrow vertical bar that switches the sidebar between its views, VS-Code style.
///
/// Clicking the view that is already showing collapses the sidebar to this bar; clicking any other
/// icon switches to it (and re-expands). The bar itself never disappears, so the panel can always be
/// brought back.
#[component]
fn SidebarViewSwitcher(view: Signal<Option<SidebarView>>) -> Element {
    rsx! {
        div { class: "sidebar-view-switcher",
            for entry in [SidebarView::NodeProperties, SidebarView::AmpOverview] {
                {
                    let (icon, title) = entry.icon_and_title();
                    let is_open = view() == Some(entry);
                    rsx! {
                        button {
                            key: "{title}",
                            r#type: "button",
                            title,
                            class: if is_open { "sidebar-view-button active" } else { "sidebar-view-button" },
                            onclick: move |_| view.set(if is_open { None } else { Some(entry) }),
                            "{icon}"
                        }
                    }
                }
            }
        }
    }
}
