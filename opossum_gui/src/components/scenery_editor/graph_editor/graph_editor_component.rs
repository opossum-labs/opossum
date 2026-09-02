use crate::components::app::SIDEBAR_SWITCHER_WIDTH;
use crate::components::{
    node_editor::{NodeConfigEditor, PumpScenarioEditor},
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
use crate::{SIDEBAR_COLLAPSED, SIDEBAR_VIEW, SIDEBAR_WIDTH};
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
    sidebar_drag_handler: EventHandler<f64>,
) -> Element {
    info!("🔄 Render: GraphEditor");
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

            // Atomically clean backend leftovers and initialize root tab
            workspace_processor
                .send(GraphsWorkspaceAction::ResetAndInitializeRootScenery { name: scenery_name });
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
                //
                // `width: auto` is load-bearing in the collapsed case: this div is a child of a
                // Bootstrap `.row`, whose `.row > *` rule sets `width: 100%`. With a flex-basis of
                // `auto` that width becomes the basis, so the collapsed bar would claim the entire
                // row and wrap the graph editor out of sight.
                style: if SIDEBAR_COLLAPSED() { "flex: 0 0 auto; width: auto;".to_string() } else { format!("flex: 0 0 {}px; width: auto;", SIDEBAR_WIDTH()) },
                SidebarViewSwitcher {}
                if !SIDEBAR_COLLAPSED() {
                    div { class: "flex-grow-1 sidebar-view",
                        match SIDEBAR_VIEW() {
                            SidebarView::NodeProperties => rsx! {
                                NodeConfigEditor {
                                    selected_nodes_memo,
                                    model_modified_handler,
                                    workspace_processor,
                                    active_graph_id: active_tab,
                                }
                            },
                            SidebarView::PumpScenarios => rsx! {
                                PumpScenarioEditor {}
                            },
                        }
                    }
                }
                // Outside the collapsed check on purpose: a collapsed sidebar must still be
                // draggable back out, exactly as it can be dragged shut.
                div {
                    class: "resizer width_resizer",
                    onmousedown: move |e: MouseEvent| {
                        sidebar_drag_handler.call(e.client_coordinates().x);
                    },
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
#[allow(clippy::volatile_composites)]
const NODE_CONFIG_ICON: Asset = asset!("/assets/icons/node_config_icon.png");
#[allow(clippy::volatile_composites)]
const AMPLIFIER_ICON: Asset = asset!("/assets/icons/amplifier_menu_icon.png");

/// Which of the sidebar's two views is showing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SidebarView {
    /// The existing selection-bound node/analyzer configuration.
    NodeProperties,
    /// The document-wide editor for pump scenarios (operating points / amplifiers).
    PumpScenarios,
}
impl SidebarView {
    /// Icon and tooltip of this view's button in the switcher bar.
    const fn icon_and_title(self) -> (Asset, &'static str) {
        match self {
            Self::NodeProperties => (NODE_CONFIG_ICON, "Node properties"),
            Self::PumpScenarios => (AMPLIFIER_ICON, "Pump scenarios"),
        }
    }
}

/// Narrow vertical bar that switches the sidebar between its views, VS-Code style.
///
/// Clicking the view that is already showing collapses the sidebar to this bar; clicking any other
/// icon switches to it (and re-expands). The bar itself never disappears, so the panel can always be
/// brought back. The collapsed state is shared with the resize drag, which collapses the sidebar
/// once it is pulled past half the minimum width.
#[component]
fn SidebarViewSwitcher() -> Element {
    rsx! {
        div {
            class: "sidebar-view-switcher",
            // Width comes from Rust because the resize drag has to know the collapsed sidebar's
            // total width; see `COLLAPSED_SIDEBAR_WIDTH`.
            style: "width: {SIDEBAR_SWITCHER_WIDTH}px;",
            for entry in [SidebarView::NodeProperties, SidebarView::PumpScenarios] {
                {
                    let (icon, title) = entry.icon_and_title();
                    let is_open = SIDEBAR_VIEW() == entry && !SIDEBAR_COLLAPSED();
                    rsx! {
                        button {
                            key: "{title}",
                            r#type: "button",
                            title,
                            class: if is_open { "noselect sidebar-view-button active" } else { "noselect sidebar-view-button" },
                            onclick: move |_| {
                                if is_open {
                                    *SIDEBAR_COLLAPSED.write() = true;
                                } else {
                                    *SIDEBAR_VIEW.write() = entry;
                                    *SIDEBAR_COLLAPSED.write() = false;
                                }
                            },
                            img { src: icon, alt: title, draggable: false }
                        }
                    }
                }
            }
        }
    }
}
