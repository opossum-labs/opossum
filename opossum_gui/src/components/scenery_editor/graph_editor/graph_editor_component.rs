use crate::components::app::SIDEBAR_SWITCHER_WIDTH;
use crate::components::scene_view::SceneView;
use crate::components::{
    node_editor::{NodeConfigEditor, PumpScenarioEditor},
    scenery_editor::{
        DragStatus, NodeEditorCommand, SelectedNode,
        graph_editor::{
            GraphViewEditor,
            hooks::{use_drag_end, use_on_key_down, use_on_key_up},
            tab_layout::{Side, TabKey, TabLayout},
        },
        graph_workspace::{
            GraphStateStoreExt, GraphsWorkspaceAction, GraphsWorkspaceState,
            GraphsWorkspaceStateStoreExt, WorkSpaceSignalHandlers, use_workspace_processor,
            workspace_action::node_editor_command,
        },
    },
};
use crate::{KEEP_SCENE_IN_FRONT, SCENE_VIEW_OPEN, SIDEBAR_COLLAPSED, SIDEBAR_VIEW, SIDEBAR_WIDTH};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use dioxus_free_icons::{Icon, icons::fa_solid_icons::FaArrowRightArrowLeft};
use dioxus_primitives::tabs::{TabList, TabTrigger, Tabs};
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
    debug!("🔄 Render: GraphEditor");
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

    // Which pane each tab sits in and which tab each pane shows is this editor's own business,
    // unlike whether the 3D view exists at all - that is driven from the menu bar, so it lives in
    // [`SCENE_VIEW_OPEN`].
    let mut layout = use_signal(TabLayout::default);
    // The active graph as of the last run of the effect below, which needs it to tell whether the
    // active graph changed and which pane a newly opened tab was opened from.
    let mut last_active = use_signal(|| None::<Uuid>);

    // Read once, ahead of the markup. Binding it inside the `rsx!` instead would make the whole tab
    // area an interpolated node, and Dioxus is then entitled to rebuild that subtree rather than
    // patch it - which for the 3D panel means a new canvas, a new WebGL context and a camera back at
    // its starting pose (see `dioxus_glb_viewer`'s invariants).
    let tab_order = use_memo(move || workspace.tab_order().read().clone());

    // Both kinds of tab in one list, so the bar is built from a single sequence.
    let tabs = use_memo(move || {
        tab_order()
            .into_iter()
            .map(TabKey::Graph)
            .chain(SCENE_VIEW_OPEN().then_some(TabKey::Scene))
            .collect::<Vec<_>>()
    });

    // The one effect, and it runs in the safe direction: from the open tabs and the active graph to
    // the layout, never back. Whatever brings a graph forward therefore brings it into view as well,
    // including code that knows nothing about panes - opening a group by double-click, an undo
    // jumping to a node, the fallback when a tab is closed. Setting `active_tab` from the layout
    // instead would let the two drift apart the moment any of those places changed it.
    //
    // `KEEP_SCENE_IN_FRONT` is the one deliberate exception: a 3D view pick changes `active_tab` too
    // (see `GraphsWorkspaceAction::RevealNode`'s `bring_to_front: false` path), so that the
    // properties sidebar - which reads the selection off `active_tab` - shows the revealed node, but
    // the whole point of that click was to keep looking at the 3D view. So the graph is not brought
    // forward where that would cover the 3D view; in the other pane of a split editor it still is,
    // which is exactly what the split is for. `.peek()` reads the flag without subscribing, so this
    // effect never runs on the flag by itself.
    use_effect(move || {
        let open = tabs();
        let active = active_tab();
        let previous = *last_active.peek();
        let mut next = layout.peek().clone();
        let opener = next.side_of(TabKey::Graph(previous.unwrap_or(active)));
        if next.sync(&open, opener).contains(&TabKey::Scene) {
            // Opening the view from the menu brings it to the front; closing it hands the pane back
            // to whatever it showed, because a closed tab is never shown.
            next.bring_to_front(TabKey::Scene);
        }
        if previous != Some(active) {
            last_active.set(Some(active));
            let keep_scene = *KEEP_SCENE_IN_FRONT.peek();
            if keep_scene {
                *KEEP_SCENE_IN_FRONT.write() = false;
            }
            let graph = TabKey::Graph(active);
            let covers_scene = open.contains(&TabKey::Scene)
                && next.side_of(graph) == next.side_of(TabKey::Scene);
            if !(keep_scene && covers_scene) {
                next.bring_to_front(graph);
            }
        }
        // Written only on an actual change: every pane re-renders on a write.
        if next != *layout.peek() {
            layout.set(next);
        }
    });

    // Move a tab to the other pane, from the button at the end of each tab bar. A moved graph also
    // becomes the one being edited: it is now in front, and a graph in front of the pane that holds
    // the active graph would otherwise hide the very graph the sidebar is showing.
    let mut move_tab = move |tab: TabKey, side: Side| {
        layout.write().move_tab(tab, side, &tabs.peek());
        if let TabKey::Graph(id) = tab {
            workspace_processor.send(GraphsWorkspaceAction::SetActiveTab(id));
        }
    };

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
    let mut ctrl_pressed = use_signal(|| false);
    let mut shift_pressed = use_signal(|| false);

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

    // A graph that changes panes may keep its size - both panes are equally wide - and then its
    // view's own `onresize` never fires, leaving it converting pointer positions against the pane it
    // left. So every graph in front is measured again whenever the layout changes.
    use_effect(move || {
        let current = layout();
        let open = tabs();
        let active = active_tab();
        for side in [Side::Left, Side::Right] {
            if let Some(TabKey::Graph(id)) = current.shown(side, &open, active) {
                workspace_processor.send(GraphsWorkspaceAction::GetEditorArea(id));
            }
        }
    });

    // Everything the markup needs about the panes, read once ahead of it.
    let open = tabs();
    let current = layout();
    let active = active_tab();
    let split = current.is_split(&open);
    let panes = if split {
        vec![Side::Left, Side::Right]
    } else {
        vec![Side::Left]
    };
    // The tab each pane shows: a panel is visible exactly when it is one of these.
    let fronts: Vec<TabKey> = panes
        .iter()
        .filter_map(|side| current.shown(*side, &open, active))
        .collect();
    // The pane holding the graph the sidebar edits. Marked only while split - with a single pane
    // there is nothing to tell apart.
    let focused_pane = split.then(|| current.side_of(TabKey::Graph(active)));

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
                onblur: move |_| {
                    ctrl_pressed.set(false);
                    shift_pressed.set(false);
                },
                onfocus: move |_| {
                    ctrl_pressed.set(false);
                    shift_pressed.set(false);
                },

                // One grid for both panes: a row of tab bars above a row of panels, and while split a
                // handle in a column of its own between them. Every panel is a child of this one
                // element, and the pane it shows up in is nothing but its grid column - so moving a
                // tab to the other pane never unmounts it. A graph keeps its view, and the 3D view
                // its WebGL context and camera.
                div {
                    class: "editor-split",
                    style: if split { "grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);" } else { "grid-template-columns: minmax(0, 1fr);" },
                    for side in panes {
                        {
                            let pane_tabs = current.tabs_on(side, &open);
                            let front = current.shown(side, &open, active);
                            // Moving the only tab of an unsplit editor would empty the pane it came
                            // from, which closes that pane again - the click would do nothing.
                            let can_move = front.is_some() && (split || pane_tabs.len() > 1);
                            let focused = focused_pane == Some(side);
                            rsx! {
                                Tabs {
                                    key: "{side:?}",
                                    class: "editor-tabs",
                                    style: "grid-column: {side.grid_column()};",
                                    value: front.map(TabKey::value).unwrap_or_default(),
                                    on_value_change: move |value: String| {
                                        if let Some(tab) = TabKey::parse(&value) {
                                            // Directly as well as through the active graph: clicking
                                            // the tab of the graph already being edited, while the 3D
                                            // view covers it, changes no `active_tab` at all.
                                            layout.write().bring_to_front(tab);
                                            if let TabKey::Graph(id) = tab {
                                                workspace_processor.send(GraphsWorkspaceAction::SetActiveTab(id));
                                            }
                                        }
                                    },
                                    TabList { class: "editor-tab-list",
                                        for (index , key) in pane_tabs.into_iter().enumerate() {
                                            TabTrigger {
                                                key: "{key.value()}",
                                                value: key.value(),
                                                index,
                                                class: match (front == Some(key), focused) {
                                                    (true, true) => "editor-tab active-tab focused-pane",
                                                    (true, false) => "editor-tab active-tab",
                                                    (false, _) => "editor-tab",
                                                },
                                                div { class: "tab-inner",
                                                    match key {
                                                        TabKey::Graph(id) => rsx! {
                                                            span {
                                                                {workspace.tabs().get(id).map(|graph| graph.graph_info().read().name.clone()).unwrap_or_default()}
                                                            }
                                                            if id != root_graph_id() {
                                                                button {
                                                                    class: "tab-close",
                                                                    onclick: move |e: MouseEvent| {
                                                                        e.stop_propagation();
                                                                        workspace_processor.send(GraphsWorkspaceAction::RemoveTabs(vec![id]));
                                                                    },
                                                                }
                                                            }
                                                        },
                                                        TabKey::Scene => rsx! {
                                                            span { "3D View" }
                                                            button {
                                                                class: "tab-close",
                                                                onclick: move |e: MouseEvent| {
                                                                    e.stop_propagation();
                                                                    *SCENE_VIEW_OPEN.write() = false;
                                                                },
                                                            }
                                                        },
                                                    }
                                                }
                                            }
                                        }
                                        div { class: "editor-tab-filler" }
                                        button {
                                            class: "editor-tab-move",
                                            r#type: "button",
                                            title: if side == Side::Left { "Move this tab to the right" } else { "Move this tab to the left" },
                                            disabled: !can_move,
                                            onclick: move |_| {
                                                if let Some(tab) = front {
                                                    move_tab(tab, side.other());
                                                }
                                            },
                                            Icon {
                                                icon: FaArrowRightArrowLeft,
                                                width: 12,
                                                height: 12,
                                                fill: "currentColor",
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if split {
                        div { class: "resizer width_resizer editor-splitter" }
                    }
                    for id in tab_order().into_iter() {
                        if let Some(graph_state) = workspace.tabs().get(id) {
                            div {
                                key: "{id.as_simple().to_string()}",
                                role: "tabpanel",
                                class: "tab-content",
                                style: format!("grid-column: {};", current.side_of(TabKey::Graph(id)).grid_column()),
                                "data-state": if fronts.contains(&TabKey::Graph(id)) { "active" } else { "inactive" },
                                hidden: !fronts.contains(&TabKey::Graph(id)),
                                // Clicking into a graph makes it the one being edited, as clicking into
                                // an editor does in VS Code. A pointer event, because the graph's own
                                // handlers stop `mousedown` from bubbling up here, and one that arrives
                                // before any of them, so the switch is queued ahead of whatever the
                                // click itself does.
                                onpointerdown: move |_| {
                                    if *workspace.active_tab().peek() != id {
                                        workspace_processor.send(GraphsWorkspaceAction::SetActiveTab(id));
                                    }
                                },
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
                    // Deliberately outside the loop above: a keyed list entry may be moved or
                    // recreated when tabs are opened and closed, and recreating this one would take
                    // the canvas - with its WebGL context and the camera the user set up - down with
                    // it. Here its place in the tree never moves; changing panes only changes its
                    // grid column.
                    //
                    // Hidden rather than unmounted while another tab is in front of its pane, for the
                    // same reason. Closing the tab does unmount it, which is when letting the context
                    // go is what was asked for.
                    if SCENE_VIEW_OPEN() {
                        div {
                            role: "tabpanel",
                            class: "tab-content",
                            style: format!("grid-column: {};", current.side_of(TabKey::Scene).grid_column()),
                            "data-state": if fronts.contains(&TabKey::Scene) { "active" } else { "inactive" },
                            hidden: !fronts.contains(&TabKey::Scene),
                            SceneView {}
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
