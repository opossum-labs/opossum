#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::{
    node_editor::NodeConfigEditor,
    scenery_editor::{
        GraphState, GraphStore, GraphStoreAction,
        constants::{MAX_ZOOM, MIN_ZOOM},
        edges::edges_component::{
            EdgeCreation, EdgeCreationComponent, EdgesComponent, NewEdgeCreationStart,
        },
        graph_editor::hooks::{
            use_drag, use_drag_end, use_on_key_down, use_on_mouse_down, use_on_resize, use_zoom,
        },
        nodes::Nodes,
        use_graph_processor,
    },
};
use dioxus::{
    html::geometry::{
        Pixels, PixelsSize,
        euclid::{Rect, Size2D, UnknownUnit, default::Point2D},
    },
    prelude::*,
};
use dioxus_primitives::tabs::{TabContent, TabList, TabTrigger, Tabs};
use opossum_core::{prelude::*, types::api_types::NewRefNode};
use std::{collections::HashMap, path::PathBuf, rc::Rc, time::Instant};
use uuid::Uuid;
#[derive(Debug)]
pub enum NodeEditorCommand {
    DeleteAll,
    AddNode(String),
    AddNodeRef(NewRefNode),
    AddAnalyzer(AnalyzerType),
    LoadFile(PathBuf),
    SaveFile(PathBuf),
    AutoLayout,
    CenterGraph { zoom_to_fit: bool },
}

#[derive(Clone, Copy)]
pub struct EditorState {
    pub editor_size: Signal<PixelsSize>,
    pub drag_status: Signal<DragStatus>,
    pub edge_in_creation: Signal<Option<EdgeCreation>>,
    pub zoom: Signal<f64>,
    pub shift: Signal<Point2D<f64>>,
    pub rect: Signal<Rect<f64, Pixels>>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            editor_size: Signal::<PixelsSize>::default(),
            drag_status: Signal::<DragStatus>::default(),
            edge_in_creation: Signal::<Option<EdgeCreation>>::default(),
            zoom: Signal::new(1.),
            shift: Signal::<Point2D<f64>>::default(),
            rect: Signal::<Rect<f64, Pixels>>::default(),
        }
    }
}

impl EditorState {
    pub fn get_view_port_center(&self) -> Point2D<f64> {
        let editor_size = *self.editor_size.read();

        Point2D::new(editor_size.width / 2., editor_size.height / 2.)
    }
    pub fn get_view_port_size(&self) -> Size2D<f64, Pixels> {
        *self.editor_size.read()
    }

    pub fn center_graph(&mut self, bounding_box: Rect<f64, UnknownUnit>, zoom_to_fit: bool) {
        if zoom_to_fit {
            self.zoom_to_fit(bounding_box);
        }
        let center = bounding_box.center();
        let zoom = *self.zoom.read();
        let view_center = self.get_view_port_center();
        self.shift.set(Point2D::new(
            center.x.mul_add(-zoom, view_center.x),
            center.y.mul_add(-zoom, view_center.y),
        ));
    }

    fn zoom_to_fit(&mut self, bounding_box: Rect<f64, UnknownUnit>) {
        let padding_fac = 0.95;
        let view_box = self.get_view_port_size();
        let zoom = *self.zoom.read();
        let height_fac = view_box.height * padding_fac / zoom / bounding_box.height();
        let width_fac = view_box.width * padding_fac / zoom / bounding_box.width();
        self.zoom
            .set((zoom * width_fac.min(height_fac)).clamp(MIN_ZOOM, MAX_ZOOM));
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

#[derive(Clone, PartialEq)]
pub struct GraphTab {
    pub graph_id: String, // "root" oder group_node_id
    pub title: String,
    pub is_active: bool,
}

#[derive(Clone, Copy, Eq, PartialEq, Default)]
pub struct GraphsWorkspaceState {
    pub tabs: Signal<HashMap<Uuid, GraphState>>,
    pub active_tab: Signal<Option<Uuid>>,
}

#[component]
pub fn GraphEditor(
    command: ReadSignal<Option<NodeEditorCommand>>,
    model_modified: Signal<bool>,
    model_file_path: Signal<Option<PathBuf>>,
) -> Element {
    let mut open_tabs: Signal<Vec<GraphTab>> = use_signal(|| {
        vec![]
    });

    let workspace = use_signal(|| GraphsWorkspaceState::default());
    let active_tab = use_memo(move || {
        workspace
            .read().active_tab.read()
            .map_or_else(|| "root".to_string(), |t| t.as_simple().to_string())
        });

    let is_modified: Memo<Signal<bool>> = use_memo(move || workspace.read().tabs.read().iter().any(|(_,g)| g.graph_store.read().needs_saving()));

    let add_new_group_tab_handler = EventHandler::new(move |(title, id): (String, Uuid)|{
        todo!();
        open_tabs
                            .write()
                            .push(GraphTab {
                                graph_id: id.as_simple().to_string(),
                                title,
                                is_active: true,
                            })
    });

    
        
    let current_mouse_pos = use_signal(Point2D::default);
        
    let graph_state: Signal<GraphState> = use_signal(GraphState::default);
    let graph_processor = use_graph_processor(graph_state, add_new_group_tab_handler);
            
    let root_graph_id = use_memo(move || graph_state.read().graph_store.read().scenery_id());
    use_effect(move || {
        graph_processor.send(GraphStoreAction::GetSceneryId);
    });

    let active_node_opt = use_memo(move || {
        graph_state
            .read()
            .graph_store
            .read()
            .get_active_node()
            .map(|n| (n.node_type().clone(), n.id()))
    });

    use_context_provider(|| graph_state().graph_store);
    use_context_provider(|| graph_state().editor_state);

    let onmouseleave_handler = use_drag_end();
    let onkeydownhandler = use_on_key_down(current_mouse_pos);

    rsx! {
        div { class: "row main-content-row",
            div { style: "min-width:256px;", class: "col-2 sidebar",
                NodeConfigEditor { active_node_opt, is_modified: is_modified() }
            }
            div {
                class: "col px-0 graph-editor-container",
                tabindex: 0,
                onkeydown: onkeydownhandler,
                onmouseleave: onmouseleave_handler,

                Tabs {
                    class: "editor-tabs",
                    value: active_tab.read().clone(),
                    on_value_change: move |v| {
                        println!("value changing");
                        open_tabs
                            .write()
                            .iter_mut()
                            .for_each(|t| {
                                if t.graph_id == v {
                                    t.is_active = true;
                                } else {
                                    t.is_active = false;
                                }
                            });
                    },
                    TabList { class: "editor-tab-list",
                        {

                            rsx! {

                                for (i , tab) in open_tabs().iter().enumerate() {

                                    TabTrigger {
                                        key: "{tab.graph_id}",
                                        value: tab.graph_id.clone(),
                                        index: i,
                                        class: if active_tab() == tab.graph_id { "editor-tab active-tab" } else { "editor-tab" },

                                        div { class: "tab-inner",
                                            span { "{tab.title}" }
                                            if tab.graph_id != root_graph_id().as_simple().to_string() {
                                                button {
                                                    class: "tab-close",
                                                    onclick: {
                                                        let id = tab.graph_id.clone();
                                                        move |_| {
                                                            open_tabs.write().retain(|t| t.graph_id != id);
                                                        }
                                                    },
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        div { class: "editor-tab-filler" }
                    }

                    for (i , tab) in open_tabs().iter().enumerate() {
                        TabContent {
                            key: "{tab.graph_id}",
                            value: tab.graph_id.clone(),
                            index: i,
                            GraphViewEditor {
                                command,
                                model_modified,
                                model_file_path,
                                current_mouse_pos,
                                add_new_group_tab_handler,
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
    command: ReadSignal<Option<NodeEditorCommand>>,
    model_modified: Signal<bool>,
    model_file_path: Signal<Option<PathBuf>>,
    current_mouse_pos: Signal<Point2D<f64>>,
    add_new_group_tab_handler: EventHandler<(String, Uuid)>
) -> Element {
    let last_auxiliary_click = use_signal(|| Option::<Instant>::None);
    let graph_processor = use_context::<Coroutine<GraphStoreAction>>();
    let editor_state = use_context::<Signal<EditorState>>();
    let graph_store = use_context::<Signal<GraphStore>>();
    let mut on_mounted: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    let onwheel_handler = use_zoom(on_mounted);
    let onmousedown_handler = use_on_mouse_down(current_mouse_pos, last_auxiliary_click);
    let onmousemove_handler = use_drag(current_mouse_pos);
    let onmouseup_handler = use_drag_end();
    let onresizehandler = use_on_resize(on_mounted);

    let shift = use_memo(move || *editor_state.read().shift.read());
    let zoom = use_memo(move || *editor_state.read().zoom.read());

    use_effect(move || {
        let needs_saving_signal = graph_store.peek().needs_saving();
        let is_unsaved = *needs_saving_signal.read();
        if *model_modified.peek() != is_unsaved {
            model_modified.set(is_unsaved);
        }
        let file_path_signal = graph_store.peek().file_path();
        let current_path = (*file_path_signal.read()).clone();

        if *model_file_path.peek() != current_path {
            model_file_path.set(current_path);
        }
    });

    use_effect(move || {
        if let Some(command) = command.read().as_ref() {
            match command {
                NodeEditorCommand::DeleteAll => {
                    graph_processor.send(GraphStoreAction::DeleteScenery);
                    graph_processor.send(GraphStoreAction::GetSceneryId);
                }
                NodeEditorCommand::AddNode(node_type) => {
                    graph_processor.send(GraphStoreAction::AddOpticNode(node_type.clone()));
                }
                NodeEditorCommand::AddNodeRef(new_ref_node) => {
                    graph_processor.send(GraphStoreAction::AddOpticReference(*new_ref_node));
                }
                NodeEditorCommand::AddAnalyzer(analyzer_type) => {
                    graph_processor.send(GraphStoreAction::AddAnalyzer(analyzer_type.clone()));
                }
                NodeEditorCommand::AutoLayout => {
                    graph_processor.send(GraphStoreAction::OptimizeLayout);
                    graph_processor.send(GraphStoreAction::CenterGraph { zoom_to_fit: true });
                }
                NodeEditorCommand::CenterGraph { zoom_to_fit } => {
                    graph_processor.send(GraphStoreAction::CenterGraph {
                        zoom_to_fit: *zoom_to_fit,
                    });
                }
                NodeEditorCommand::LoadFile(path) => {
                    graph_processor.send(GraphStoreAction::LoadFromFile(path.to_owned()));
                }
                NodeEditorCommand::SaveFile(path) => {
                    graph_processor.send(GraphStoreAction::SaveToFile(path.to_owned()));
                }
            }
        }
    });

    rsx! {
        div {
            class: "graph-editor",
            id: "editor",
            draggable: false,

            onwheel: onwheel_handler,
            onmousedown: onmousedown_handler,
            onmouseup: onmouseup_handler,
            onmousemove: onmousemove_handler,
            onresize: onresizehandler,
            onmounted: move |event| { on_mounted.set(Some(event.data)) },
            div {
                draggable: false,
                style: format!(
                    "transform-origin: 0 0; transform: translate({}px, {}px) scale({});",
                    shift().x,
                    shift().y,
                    zoom(),
                ),
                Nodes { add_new_group_tab_handler }
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
