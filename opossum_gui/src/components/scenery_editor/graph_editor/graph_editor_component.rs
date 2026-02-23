#![allow(clippy::derive_partial_eq_without_eq)]
use crate::{OPOSSUM_UI_LOGS, api::{self, eval_action_run}, components::{
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
}};
use dioxus::{
    html::geometry::{
        Pixels, PixelsSize,
        euclid::{Rect, Size2D, UnknownUnit, default::Point2D},
    },
    prelude::*,
};
use dioxus_primitives::tabs::{TabContent, TabList, TabTrigger, Tabs};
use futures_util::StreamExt;
use opossum_core::{opm_document::AnalyzerInfo, prelude::*, types::api_types::{ConnectInfo, NewRefNode, NodeInfo}};
use std::{collections::{BTreeMap, HashMap}, fs, path::PathBuf, rc::Rc, time::Instant};
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
    pub tabs: Signal<BTreeMap<Uuid, GraphState>>,
    pub active_tab: Signal<Option<Uuid>>,
    pub root_scenery_id: Signal<Uuid>,
    pub needs_saving: Signal<bool>
}

#[component]
pub fn GraphEditor(
    command: ReadSignal<Option<NodeEditorCommand>>,
    model_modified_sig: ReadSignal<bool>,
    model_modified_handler: EventHandler<bool>,
    mut model_file_path: Signal<Option<PathBuf>>,
) -> Element {

    let mut workspace = use_signal(|| GraphsWorkspaceState::default());
    let root_graph_id = use_memo(move || *workspace.read().root_scenery_id.read());

    //Handler definition
    let add_new_group_tab_handler = EventHandler::new(move |(title, id): (String, Uuid)|{
        let mut graph_state = GraphState::default();
        graph_state.graph_store.write().set_scenery_id(id);
        workspace.write().tabs.write().insert(id, GraphState::default());
    });
    let set_root_scenery_id_handler = EventHandler::new(move | id: Uuid|{
        workspace.write().root_scenery_id.set(id);
    });
    let remove_tab_handler = EventHandler::new(move |id: Uuid|{
        workspace.write().tabs.write().remove(&id);
    });
    let set_file_path_handler = EventHandler::new(move | path_opt: Option<PathBuf>|{
        model_file_path.set(path_opt);
    });
    let set_needs_saving_handler = EventHandler::new(move | needs_saving: bool|{
        workspace.write().needs_saving.set(needs_saving);
    });
    let clear_workspace_handler = EventHandler::new(move |()|{
        workspace.write().tabs.write().clear();
        workspace.write().active_tab.set(None);
        workspace.write().root_scenery_id.set(Uuid::nil());
        workspace.write().needs_saving.set(false);
    });
    let add_root_scenery_nodes_handler = EventHandler::new(move | nodes: Vec<NodeInfo>|{
        let id = *root_graph_id.read();
        if let Some(graph_state) = workspace.write().tabs.write().get_mut(&id){
            graph_state.graph_store.write().add_nodes(&nodes);
        }
        else{
            OPOSSUM_UI_LOGS.write().add_log("no root scenery found! Cannot add nodes!");
        }
    });
    let add_root_scenery_analyzers_handler = EventHandler::new(move | analyzers: Vec<AnalyzerInfo>|{
        let id = *root_graph_id.read();
        if let Some(graph_state) = workspace.write().tabs.write().get_mut(&id){
            graph_state.graph_store.write().add_analyzers(&analyzers);
        }
        else{
            OPOSSUM_UI_LOGS.write().add_log("no root scenery found! Cannot add analyzers!");
        }
    });
    let add_root_scenery_edges_handler = EventHandler::new(move | connect_infos: Vec<ConnectInfo>|{
        let id = *root_graph_id.read();
        if let Some(graph_state) = workspace.write().tabs.write().get_mut(&id){
            graph_state.graph_store.write().edges_mut().set(connect_infos);
        }
        else{
            OPOSSUM_UI_LOGS.write().add_log("no root scenery found! Cannot add edges!");
        }
    });

    let set_active_tab_handler = EventHandler::new(move |active_tab: Option<Uuid>|{
        workspace.write().active_tab.set(active_tab);
    });

    let workspace_processor = use_workspace_processor(
        root_graph_id.into(), 
        add_new_group_tab_handler, 
        set_root_scenery_id_handler, 
        set_file_path_handler, 
        set_needs_saving_handler,
        clear_workspace_handler,
        add_root_scenery_nodes_handler,
        add_root_scenery_analyzers_handler,
        add_root_scenery_edges_handler);


    let active_tab = use_memo(move || {
        workspace
            .read().active_tab.read()
            .map_or_else(|| Uuid::nil(), |t| t)
        });



    
        
    let current_mouse_pos = use_signal(Point2D::default);
        
    let graph_state: Signal<GraphState> = use_signal(GraphState::default);
    let graph_processor = use_graph_processor(graph_state, add_new_group_tab_handler);
            
    use_effect(move || {
        workspace_processor.send(GraphsWorkspaceAction::GetSceneryId);
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
                        println!("value changing");
                        if let Ok(new_id) = Uuid::parse_str(&v) {
                            set_active_tab_handler.call(Some(new_id));
                        }
                    },
                    {
                        let tabs = workspace.read().tabs.read().clone();
                        rsx! {
                            TabList { class: "editor-tab-list",
                                for (i , id) in tabs.keys().enumerate() {
                                    TabTrigger {
                                        key: "{id.as_simple().to_string()}",
                                        value: id.as_simple().to_string(),
                                        index: i,
                                        class: if active_tab() == *id { "editor-tab active-tab" } else { "editor-tab" },
                                        div { class: "tab-inner",
                                            span { "todo: naming" }
                                            if *id != root_graph_id() {
                                                button {
                                                    class: "tab-close",
                                                    onclick: {
                                                        let id_copy = *id;
                                                        move |_| remove_tab_handler.call(id_copy)
                                                    },

                                                }
                                            }
                                        }
                                    }
                                }
                                div { class: "editor-tab-filler" }
                            }
                            for (i , id) in tabs.keys().enumerate() {
                                TabContent {
                                    key: "{id.as_simple().to_string()}",
                                    value: id.as_simple().to_string(),
                                    index: i,
                                    GraphViewEditor {
                                        command,
                                        model_modified_sig,
                                        model_modified_handler,
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
    }
}


pub fn use_workspace_processor(
    root_graph_id: ReadSignal<Uuid>, 
    add_new_group_tab_handler: EventHandler<(String, Uuid)>, 
    set_root_scenery_id_handler: EventHandler<Uuid>, 
    set_file_path_handler: EventHandler<Option<PathBuf>>, 
    set_needs_saving_handler: EventHandler<bool>,
    clear_workspace_handler: EventHandler<()>,
    add_root_scenery_nodes_handler: EventHandler<Vec<NodeInfo>>,
    add_root_scenery_analyzers_handler: EventHandler<Vec<AnalyzerInfo>>,
    add_root_scenery_edges_handler: EventHandler<Vec<ConnectInfo>>
) -> Coroutine<GraphsWorkspaceAction> {
    use_coroutine(move |mut rx: UnboundedReceiver<GraphsWorkspaceAction>| {
        async move {
            // This loop runs forever in the background, waiting for actions.
            while let Some(action) = rx.next().await {
                match action {
                    GraphsWorkspaceAction::LoadFromFile(path) => {
                        process_load_from_file(
                            path, 
                            root_graph_id,
                            clear_workspace_handler, 
                            add_new_group_tab_handler, 
                            set_root_scenery_id_handler, 
                            set_file_path_handler, 
                            set_needs_saving_handler,
                            add_root_scenery_nodes_handler,
                            add_root_scenery_analyzers_handler,
                            add_root_scenery_edges_handler
                        ).await;
                        // process_center_graph(graph_state, false);
                    }
                    GraphsWorkspaceAction::SaveToFile(path) => {
                        process_save_root_scenery_to_file(path, set_file_path_handler, set_needs_saving_handler).await;
                    }
                    GraphsWorkspaceAction::DeleteScenery => {
                        process_delete_root_scenery(clear_workspace_handler).await;
                    }
                    GraphsWorkspaceAction::GetSceneryId => {
                        process_get_root_scenery_id(add_new_group_tab_handler, set_root_scenery_id_handler).await;
                    }
                }
            }
        }
    })
}

#[allow(clippy::future_not_send)]
async fn process_load_from_file(
    path: PathBuf, 
    scenery_id_sig: ReadSignal<Uuid>,
    clear_workspace_handler: EventHandler<()>, 
    add_new_group_tab_handler: EventHandler<(String, Uuid)>,
    set_root_scenery_id_handler: EventHandler<Uuid>,
    set_file_path_handler: EventHandler<Option<PathBuf>>,
    set_needs_saving_handler: EventHandler<bool>,
    add_root_scenery_nodes_handler: EventHandler<Vec<NodeInfo>>,
    add_root_scenery_analyzers_handler: EventHandler<Vec<AnalyzerInfo>>,
    add_root_scenery_edges_handler: EventHandler<Vec<ConnectInfo>>
) {
    process_delete_root_scenery(clear_workspace_handler).await;
    //is process_get_root_scenery_id necessary here?
    process_get_root_scenery_id(add_new_group_tab_handler, set_root_scenery_id_handler).await;
    set_file_path_handler.call(None);

    let opm_string = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            OPOSSUM_UI_LOGS.write().add_log(&e.to_string());
            return;
        }
    };
    match api::post_opm_file(opm_string).await {
        Ok(_) => {
            process_get_root_scenery_id(add_new_group_tab_handler, set_root_scenery_id_handler).await;
            set_needs_saving_handler.call(false);
            set_file_path_handler.call(Some(path));
            let scenery_id = *scenery_id_sig.read();
            eval_action_run(
                api::get_nodes(scenery_id).await,
                Some(move |nodes: Vec<NodeInfo>| add_root_scenery_nodes_handler.call(nodes)), // graph_store.write().add_nodes(&nodes)),
            );
            eval_action_run(
                api::get_analyzers().await,
                Some(move |analyzers: Vec<AnalyzerInfo>| {
                    add_root_scenery_analyzers_handler.call(analyzers);
                    // graph_store.write().add_analyzers(&analyzers);
                }),
            );
            eval_action_run(
                api::get_connections(scenery_id).await,
                Some(move |connect_infos: Vec<ConnectInfo>| {
                    add_root_scenery_edges_handler.call(connect_infos)
                    // graph_store.write().edges.set(connect_infos);
                }),
            );
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }
}

#[allow(clippy::future_not_send)]
async fn process_delete_root_scenery(clear_workspace_handler: EventHandler<()>) {
    eval_action_run(
        api::delete_scenery().await,
        Some(move |_| {
            clear_workspace_handler.call(());
        }),
    );
}


#[allow(clippy::future_not_send)]
async fn process_save_root_scenery_to_file(path: PathBuf, set_file_path_handler: EventHandler<Option<PathBuf>>, set_needs_saving_handler: EventHandler<bool>) {
    eval_action_run(
        api::get_opm_file().await,
        Some(move |opm_string| {
            if let Err(err_str) = fs::write(&path, opm_string) {
                OPOSSUM_UI_LOGS.write().add_log(&err_str.to_string());
            } else {
                set_file_path_handler.call(Some(path));
                set_needs_saving_handler.call(false);
            }
        }),
    );
}

async fn process_get_root_scenery_id(add_new_group_tab_handler: EventHandler<(String, Uuid)>,  set_root_scenery_id_handler: EventHandler<Uuid>) {
    eval_action_run(
        api::get_scenery_uuid().await,
        Some(move |id| {
            set_root_scenery_id_handler.call(id);
            add_new_group_tab_handler.call(("Main Graph".to_string(), id))
        }),
    );
}

pub enum GraphsWorkspaceAction {
    LoadFromFile(PathBuf),
    SaveToFile(PathBuf),
    GetSceneryId,
    DeleteScenery,
}

#[component]
pub fn GraphViewEditor(
    command: ReadSignal<Option<NodeEditorCommand>>,
    model_modified_sig: ReadSignal<bool>,
    model_modified_handler: EventHandler<bool>,
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
        if *model_modified_sig.peek() != is_unsaved {
            model_modified_handler.call(is_unsaved);
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
