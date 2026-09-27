use crate::components::app::SIDEBAR_SWITCHER_WIDTH;
use crate::components::scene_view::SceneView;
use crate::components::{
    context_menu::cx_menu::CxMenu,
    node_editor::{NodeConfigEditor, PumpScenarioEditor},
    scenery_editor::{
        DragStatus, NodeEditorCommand, SelectedNode,
        graph_editor::{
            GraphViewEditor,
            hooks::{use_drag_end, use_on_key_down, use_on_key_up},
            tab_layout::{DropTarget, Side, TabDrag, TabKey, TabLayout},
        },
        graph_workspace::{
            GraphStateStoreExt, GraphsWorkspaceAction, GraphsWorkspaceState,
            GraphsWorkspaceStateStoreExt, WorkSpaceSignalHandlers, use_workspace_processor,
            workspace_action::node_editor_command,
        },
    },
};
use crate::{
    CONTEXT_MENU, KEEP_SCENE_IN_FRONT, SCENE_VIEW_OPEN, SIDEBAR_COLLAPSED, SIDEBAR_VIEW,
    SIDEBAR_WIDTH,
};
use dioxus::{
    html::{geometry::euclid::default::Point2D, input_data::MouseButton},
    prelude::*,
};
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
    // The width both panes share, kept up to date by the grid's gauge element: dragging the handle
    // has to turn a pointer position into a share of it.
    let mut split_width = use_signal(|| 0.0_f64);
    // The handle between the panes is dragged the way the sidebar's is (see `CommonAppLayout`):
    // `Some((pointer x, left pane width))` at the moment the drag started, `None` while not
    // dragging. The width asked for is derived from those two on every move rather than
    // accumulated, so dragging past a pane's minimum and back picks up again the moment the pointer
    // returns, instead of wherever the clamped width had got stuck.
    let mut split_drag = use_signal(|| None::<(f64, f64)>);

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
            let covers_scene =
                open.contains(&TabKey::Scene) && next.side_of(graph) == next.side_of(TabKey::Scene);
            if !(keep_scene && covers_scene) {
                next.bring_to_front(graph);
            }
        }
        // Written only on an actual change: every pane re-renders on a write.
        if next != *layout.peek() {
            layout.set(next);
        }
    });

    // Put a tab into another pane - from the button at the end of each tab bar, a tab's context
    // menu, or dropping a dragged tab. A moved graph also becomes the one being edited: it is now in
    // front, and a graph in front of the pane that holds the active graph would otherwise hide the
    // very graph the sidebar is showing.
    let mut place_tab = move |tab: TabKey, target: DropTarget| {
        layout.write().drop(tab, target, &tabs.peek());
        if let TabKey::Graph(id) = tab {
            workspace_processor.send(GraphsWorkspaceAction::SetActiveTab(id));
        }
    };

    // The same move from a tab's context menu: the tab right-clicked and the pane it is to go to,
    // set when the menu opens and carried out by its entry. One callback for every menu, rather
    // than a new one per right-click that would live as long as this editor does.
    let mut tab_menu_target = use_signal(|| None::<(TabKey, Side)>);
    let move_from_tab_menu = use_callback(move |()| {
        if let Some((tab, side)) = *tab_menu_target.peek() {
            place_tab(tab, DropTarget::Pane(side));
        }
    });

    // A tab held down with the mouse on its way to another pane, and the drop zone it is over. The
    // markup only reads `dragged_tab`, which changes when a drag starts and ends - not on every move
    // of the pointer, which only the ghost label following it has to know about.
    let mut tab_drag = use_signal(|| None::<TabDrag>);
    let mut drop_target = use_signal(|| None::<DropTarget>);
    let dragged_tab = use_memo(move || tab_drag().filter(TabDrag::is_started).map(|drag| drag.tab));
    // Ends a tab drag, dropping the tab if `drop` is set and it is over a drop zone. Written only
    // while one is running, like the pane handle's drag.
    let mut end_tab_drag = move |drop: bool| {
        let Some(drag) = *tab_drag.peek() else {
            return;
        };
        let target = *drop_target.peek();
        tab_drag.set(None);
        if target.is_some() {
            drop_target.set(None);
        }
        if drop
            && drag.is_started()
            && let Some(target) = target
        {
            place_tab(drag.tab, target);
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

    // The tab each pane shows, left then right. A memo of its own so that what depends on it changes
    // only when a tab comes forward or changes panes - not on every step of a drag of the handle
    // between the panes, which changes the layout too.
    let pane_fronts = use_memo(move || {
        let current = layout();
        let open = tabs();
        let active = active_tab();
        [Side::Left, Side::Right].map(|side| current.shown(side, &open, active))
    });

    // A graph that changes panes may keep its size - both panes can be equally wide - and then its
    // view's own `onresize` never fires, leaving it converting pointer positions against the pane it
    // left. So every graph in front is measured again whenever that changes.
    use_effect(move || {
        for front in pane_fronts() {
            if let Some(TabKey::Graph(id)) = front {
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
    // A panel is visible exactly when it is one of these.
    let fronts: Vec<TabKey> = pane_fronts().into_iter().flatten().collect();
    // The pane holding the graph the sidebar edits. Marked only while split - with a single pane
    // there is nothing to tell apart.
    let focused_pane = split.then(|| current.side_of(TabKey::Graph(active)));
    let grid_columns = current.grid_columns(&open);
    let row_class = if split_drag().is_some() {
        "row main-content-row resizing"
    } else if dragged_tab().is_some() {
        "row main-content-row tab-dragging"
    } else {
        "row main-content-row"
    };
    // The text a tab carries, in its tab bar and on the label following it while it is dragged.
    let tab_label = move |tab: TabKey| match tab {
        TabKey::Graph(id) => workspace
            .tabs()
            .get(id)
            .map(|graph| graph.graph_info().read().name.clone())
            .unwrap_or_default(),
        TabKey::Scene => "3D View".to_owned(),
    };

    // Ends a drag of the handle between the panes. Written only while one is running, so the
    // countless mouse-ups and leaves of normal work do not re-render the editor.
    let mut end_split_drag = move || {
        if split_drag.peek().is_some() {
            split_drag.set(None);
        }
    };

    rsx! {
        div {
            class: row_class,
            // The move and release listeners of both drags - the pane handle's and a tab's - sit
            // out here, like the sidebar's, so a drag survives the pointer leaving the element it
            // started on: the narrow handle, or the tab. Leaving this row altogether ends it.
            onmousemove: move |e: MouseEvent| {
                let held = *tab_drag.peek();
                if let Some(mut drag) = held {
                    drag.move_to(Point2D::new(e.client_coordinates().x, e.client_coordinates().y));
                    tab_drag.set(Some(drag));
                }
                if let Some((start_x, start_width)) = *split_drag.peek() {
                    let mut next = layout.peek().clone();
                    next.set_left_width(
                        start_width + e.client_coordinates().x - start_x,
                        *split_width.peek(),
                    );
                    // Written only on an actual change, which is none at all while a pane sits at
                    // its minimum width.
                    if next != *layout.peek() {
                        layout.set(next);
                    }
                }
            },
            onmouseup: move |_| {
                end_split_drag();
                end_tab_drag(true);
            },
            onmouseleave: move |_| {
                end_split_drag();
                end_tab_drag(false);
            },
            // Any press in the sidebar or the editor closes an open context menu - a tab's, say,
            // when another tab is clicked next. The graph's own presses stop here before
            // reaching this, and close it themselves.
            onmousedown: move |_| {
                if CONTEXT_MENU.peek().is_some() {
                    *CONTEXT_MENU.write() = None;
                }
            },
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
                    style: "grid-template-columns: {grid_columns};",
                    // Measures the width both panes share: an empty element spanning the whole grid,
                    // rather than an `onresize` on the grid itself. Dioxus desktop 0.7 reports every
                    // `resize` as bubbling, so a listener on the grid would also receive the size of
                    // each graph view inside it - and a drag of the handle would then scale the
                    // pointer's movement by one pane's width instead of both panes', overshooting
                    // and swinging back and forth. With no children, nothing can bubble into this.
                    div {
                        class: "editor-split-gauge",
                        onresize: move |e: ResizeEvent| {
                            if let Ok(size) = e.get_border_box_size() {
                                split_width.set(size.width);
                            }
                        },
                    }
                    for side in panes {
                        {
                            let pane_tabs = current.tabs_on(side, &open);
                            let front = current.shown(side, &open, active);
                            // Moving the only tab of an unsplit editor would empty the pane it came
                            // from, which closes that pane again - the click would do nothing.
                            let can_move = split || pane_tabs.len() > 1;
                            let move_label = side.move_label(split);
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
                                                div {
                                                    class: "tab-inner",
                                                    // Pressing a tab may be the start of dragging it to
                                                    // another pane; it only becomes one once the pointer
                                                    // has travelled a few pixels, so a click stays a click.
                                                    onmousedown: move |e: MouseEvent| {
                                                        if e.trigger_button() == Some(MouseButton::Primary) {
                                                            let at = Point2D::new(
                                                                e.client_coordinates().x,
                                                                e.client_coordinates().y,
                                                            );
                                                            tab_drag.set(Some(TabDrag::new(key, at)));
                                                        }
                                                    },
                                                    oncontextmenu: move |e: MouseEvent| {
                                                        e.prevent_default();
                                                        if !can_move {
                                                            return;
                                                        }
                                                        tab_menu_target.set(Some((key, side.other())));
                                                        let mut menu = CxMenu::new(
                                                            e.page_coordinates().x,
                                                            e.page_coordinates().y,
                                                            vec![],
                                                        );
                                                        menu.add_view_entry(move_label.to_owned(), move_from_tab_menu);
                                                        *CONTEXT_MENU.write() = Some(menu);
                                                    },
                                                    match key {
                                                        TabKey::Graph(id) => rsx! {
                                                            span { {tab_label(key)} }
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
                                                            span { {tab_label(key)} }
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
                                            title: move_label,
                                            disabled: !can_move || front.is_none(),
                                            onclick: move |_| {
                                                if let Some(tab) = front {
                                                    place_tab(tab, DropTarget::Pane(side.other()));
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
                        div {
                            class: "resizer width_resizer editor-splitter",
                            onmousedown: move |e: MouseEvent| {
                                // Start from the width the left pane actually has on screen, so
                                // the handle follows the pointer from the first pixel.
                                let left_width = layout.peek().ratio() * *split_width.peek();
                                split_drag.set(Some((e.client_coordinates().x, left_width)));
                            },
                        }
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
                    // Where a dragged tab can go, over the panels so neither a graph nor the 3D view
                    // takes the pointer for its own. Their enter and leave stop here: Dioxus desktop
                    // bubbles those too, and a zone's leave reaching this row's `onmouseleave` would
                    // end the drag the moment the pointer left a zone.
                    if let Some(dragged) = dragged_tab() {
                        for target in current.drop_targets(dragged, &open) {
                            div {
                                key: "{target:?}",
                                class: if drop_target() == Some(target) { "tab-drop-zone hovered" } else { "tab-drop-zone" },
                                style: target.zone_style(),
                                onmouseenter: move |e: MouseEvent| {
                                    e.stop_propagation();
                                    drop_target.set(Some(target));
                                },
                                onmouseleave: move |e: MouseEvent| {
                                    e.stop_propagation();
                                    if *drop_target.peek() == Some(target) {
                                        drop_target.set(None);
                                    }
                                },
                            }
                        }
                    }
                }
                // In here rather than directly in the row: Bootstrap's `.row > *` gives every child
                // of a row the row's full width, which would stretch the label across the window.
                TabDragGhost {
                    drag: tab_drag,
                    label: dragged_tab().map(tab_label).unwrap_or_default(),
                }
            }
        }
    }
}

/// The label of a tab being dragged, following the pointer.
///
/// A component of its own so that only it re-renders on every move of the pointer, not the whole
/// editor.
#[component]
fn TabDragGhost(drag: ReadSignal<Option<TabDrag>>, label: String) -> Element {
    let Some(drag) = drag().filter(TabDrag::is_started) else {
        return rsx! {};
    };
    rsx! {
        div {
            class: "tab-drag-ghost",
            style: "left: {drag.pointer.x + 14.0}px; top: {drag.pointer.y + 10.0}px;",
            "{label}"
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
