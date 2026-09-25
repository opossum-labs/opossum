use crate::components::app::SIDEBAR_SWITCHER_WIDTH;
use crate::components::scene_view::SceneView;
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
use crate::{KEEP_SCENE_IN_FRONT, SCENE_VIEW_OPEN, SIDEBAR_COLLAPSED, SIDEBAR_VIEW, SIDEBAR_WIDTH};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use dioxus_primitives::tabs::{TabList, TabTrigger, Tabs};
use std::path::PathBuf;
use uuid::Uuid;

/// The value the tab bar identifies the 3D view by.
///
/// Every other tab is named by the uuid of the graph it shows; this one is not a graph, so it needs
/// a name that no uuid can collide with.
const SCENE_TAB_VALUE: &str = "scene-3d";

/// One entry of the editor's tab bar.
///
/// A view type, deliberately not a key of [`GraphsWorkspaceState`]. The store is a map of graphs and
/// stays that way: its `active_tab` answers "which graph is being edited", which is what its several
/// dozen readers - the properties panel, the selection, undo targets - actually want, and what must
/// *not* change just because someone looks at the 3D view for a moment.
///
/// What this type adds is the other question, "which tab is in front", and having it as one value
/// is what lets every panel derive its visibility from a single comparison. Before, the graph panels
/// hid on `active_tab` while the 3D panel hid on a flag of its own, so with the 3D view in front the
/// last graph stayed visible beside it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TabKey {
    /// The graph with this uuid.
    Graph(Uuid),
    /// The 3D view of the model.
    Scene,
}

impl TabKey {
    /// The string the tab bar identifies this tab by.
    fn value(self) -> String {
        match self {
            Self::Graph(id) => id.as_simple().to_string(),
            Self::Scene => SCENE_TAB_VALUE.to_owned(),
        }
    }

    /// Read back what [`Self::value`] wrote.
    ///
    /// # Returns
    ///
    /// The tab, or `None` for a value from neither of the two shapes.
    fn parse(value: &str) -> Option<Self> {
        if value == SCENE_TAB_VALUE {
            Some(Self::Scene)
        } else {
            Uuid::parse_str(value).ok().map(Self::Graph)
        }
    }
}

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

    // Which tab is in front is this editor's own business, unlike whether the 3D view exists at all
    // - that is driven from the menu bar, so it lives in [`SCENE_VIEW_OPEN`].
    let mut scene_in_front = use_signal(|| false);

    // Derived, never stored. Storing a `TabKey` and setting `active_tab` from it would let the two
    // drift apart the moment anything else changes the active tab - opening a group by double-click,
    // an undo jumping to a node, the fallback when a tab is closed - because none of those places
    // knows about this signal.
    let shown = use_memo(move || {
        if scene_in_front() {
            TabKey::Scene
        } else {
            TabKey::Graph(active_tab())
        }
    });

    // The one effect, and it runs in the safe direction: from the active graph to the flag, never
    // back. Whatever brings a graph forward therefore brings it into view as well, including code
    // that knows nothing about the 3D view.
    //
    // `KEEP_SCENE_IN_FRONT` is the one deliberate exception: a 3D view pick changes `active_tab` too
    // (see `GraphsWorkspaceAction::RevealNode`'s `bring_to_front: false` path), so that the
    // properties sidebar - which reads the selection off `active_tab` - shows the revealed node, but
    // the whole point of that click was to keep looking at the 3D view. `.peek()` reads it without
    // subscribing, so this effect still runs only on an `active_tab` change, never on the flag by
    // itself.
    use_effect(move || {
        active_tab();
        if *KEEP_SCENE_IN_FRONT.peek() {
            *KEEP_SCENE_IN_FRONT.write() = false;
        } else {
            scene_in_front.set(false);
        }
    });

    // Opening the view from the menu brings it to the front; closing it hands the graph back.
    use_effect(move || scene_in_front.set(SCENE_VIEW_OPEN()));

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

                Tabs {
                    class: "editor-tabs",
                    value: shown().value(),
                    on_value_change: move |value: String| {
                        match TabKey::parse(&value) {
                            Some(TabKey::Scene) => scene_in_front.set(true),
                            Some(TabKey::Graph(id)) => {
                                scene_in_front.set(false);
                                workspace_processor.send(GraphsWorkspaceAction::SetActiveTab(id));
                            }
                            None => {}
                        }
                    },
                    TabList { class: "editor-tab-list",
                        for (index , key) in tabs().into_iter().enumerate() {
                            TabTrigger {
                                key: "{key.value()}",
                                value: key.value(),
                                index,
                                class: if shown() == key { "editor-tab active-tab" } else { "editor-tab" },
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
                    }
                    div {
                        id: "graphEditorContentContainer",
                        class: "graph-editor-tab-content",
                        onresize: move |_| workspace_processor.send(GraphsWorkspaceAction::GetEditorArea),
                        for id in tab_order().into_iter() {
                            if let Some(graph_state) = workspace.tabs().get(id) {
                                div {
                                    key: "{id.as_simple().to_string()}",
                                    role: "tabpanel",
                                    class: "tab-content",
                                    "data-state": if shown() == TabKey::Graph(id) { "active" } else { "inactive" },
                                    hidden: shown() != TabKey::Graph(id),
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
                        // Deliberately outside the loop above, although its `hidden` comes from the
                        // same [`shown`]: a keyed list entry may be moved or recreated when tabs are
                        // opened and closed, and recreating this one would take the canvas - with its
                        // WebGL context and the camera the user set up - down with it. Here its place
                        // in the tree never moves.
                        //
                        // Hidden rather than unmounted while a graph is in front, for the same reason.
                        // Closing the tab does unmount it, which is when letting the context go is
                        // what was asked for.
                        if SCENE_VIEW_OPEN() {
                            div {
                                role: "tabpanel",
                                class: "tab-content",
                                "data-state": if shown() == TabKey::Scene { "active" } else { "inactive" },
                                hidden: shown() != TabKey::Scene,
                                SceneView {}
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
