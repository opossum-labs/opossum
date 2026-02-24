#![allow(clippy::derive_partial_eq_without_eq)]
use crate::{OPOSSUM_UI_LOGS, api::{self, eval_action_run}, components::{
    node_editor::NodeConfigEditor,
    scenery_editor::{
        GraphState, NodeType, constants::{MAX_ZOOM, MIN_NODE_DISTANCE_RADIUS, MIN_ZOOM, NODE_PLACEMENT_MAX_ITERATIONS}, edges::edges_component::{
            EdgeCreation, EdgeCreationComponent, EdgesComponent, NewEdgeCreationStart,
        }, graph_editor::hooks::{
            use_drag, use_drag_end, use_on_key_down, use_on_mouse_down, use_on_resize, use_zoom,
        }, graph_store::optimize_layout_and_sync, nodes::Nodes
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
use opossum_core::{opm_document::AnalyzerInfo, prelude::*, types::api_types::{ConnectInfo, NewAnalyzerInfo, NewNode, NewRefNode, NodeInfo}};
use std::{collections::BTreeMap, fs, path::PathBuf, rc::Rc, time::Instant};
use uuid::Uuid;
#[derive(Debug, Clone)]
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

#[derive(Clone, Eq, PartialEq, Default)]
pub struct GraphsWorkspaceState {
    pub tabs: Signal<BTreeMap<Uuid, Signal<GraphState>>>,
    pub active_tab: Signal<Option<Uuid>>,
    pub root_scenery_id: Signal<Uuid>,
    pub needs_saving: Signal<bool>,
    pub file_path: Signal<Option<PathBuf>>,
}

#[component]
pub fn GraphEditor(
    command: ReadSignal<Option<NodeEditorCommand>>,
    node_editor_command_handler: EventHandler<Option<NodeEditorCommand>>,
    model_modified_sig: ReadSignal<bool>,
    model_modified_handler: EventHandler<bool>,
    model_file_path_sig: ReadSignal<Option<PathBuf>>,
    model_file_path_handler: EventHandler<Option<PathBuf>>,
) -> Element {

    let mut workspace = use_signal(|| GraphsWorkspaceState::default());
    let root_graph_id = use_memo(move || *workspace.read().root_scenery_id.read());

    //Handler definition
    let add_new_group_tab_handler = EventHandler::new(move |(title, id): (String, Uuid)|{
        let mut graph_state = GraphState::default();
        graph_state.id = id;
        graph_state.name = title;
        workspace.write().tabs.write().insert(id, Signal::new(graph_state));
        workspace.write().active_tab.set(Some(id));
    });
    let set_root_scenery_id_handler = EventHandler::new(move | id: Uuid|{
        workspace.write().root_scenery_id.set(id);
    });
    let remove_tab_handler = EventHandler::new(move |id: Uuid|{
        workspace.write().tabs.write().remove(&id);
    });
    let set_needs_saving_handler = EventHandler::new(move | needs_saving: bool|{
        workspace.write().needs_saving.set(needs_saving);
    });
    let clear_workspace_handler = EventHandler::new(move |()|{
        workspace.set(GraphsWorkspaceState::default());
        // workspace.write().tabs.write().clear();
        // workspace.write().active_tab.set(None);
        // workspace.write().root_scenery_id.set(Uuid::nil());
        // workspace.write().needs_saving.set(false);
    });
    let add_root_scenery_nodes_handler = EventHandler::new(move | nodes: Vec<NodeInfo>|{
        let id = *root_graph_id.read();
        if let Some(graph_state) = workspace.write().tabs.write().get_mut(&id){
            graph_state.write().graph_store.write().add_nodes(&nodes);
        }
        else{
            OPOSSUM_UI_LOGS.write().add_log("no root scenery found! Cannot add nodes!");
        }
    });
    let add_root_scenery_analyzers_handler = EventHandler::new(move | analyzers: Vec<AnalyzerInfo>|{
        let id = *root_graph_id.read();
        if let Some(graph_state) = workspace.write().tabs.write().get_mut(&id){
            graph_state.write().graph_store.write().add_analyzers(&analyzers);
        }
        else{
            OPOSSUM_UI_LOGS.write().add_log("no root scenery found! Cannot add analyzers!");
        }
    });
    let add_root_scenery_edges_handler = EventHandler::new(move | connect_infos: Vec<ConnectInfo>|{
        let id = *root_graph_id.read();
        if let Some(graph_state) = workspace.write().tabs.write().get_mut(&id){
            graph_state.write().graph_store.write().edges_mut().set(connect_infos);
        }
        else{
            OPOSSUM_UI_LOGS.write().add_log("no root scenery found! Cannot add edges!");
        }
    });

    let set_active_tab_handler = EventHandler::new(move |active_tab: Option<Uuid>|{
        workspace.write().active_tab.set(active_tab);
    });

    let workspace_processor = use_workspace_processor(
        workspace, 
        root_graph_id.into(), 
        add_new_group_tab_handler, 
        set_root_scenery_id_handler, 
        model_file_path_handler, 
        set_needs_saving_handler,
        clear_workspace_handler,
        add_root_scenery_nodes_handler,
        add_root_scenery_analyzers_handler,
        add_root_scenery_edges_handler);


    let active_tab = use_memo(move || {
        workspace
            .read().active_tab.read()
            .map_or_else(|| Uuid::nil(), |t| t)
        }
    );

    use_effect(move || {
        let cmd = {
            command.read().cloned()
        };
        if let Some(command) = cmd {
            match command {
                NodeEditorCommand::DeleteAll => {
                    workspace_processor.send(GraphsWorkspaceAction::DeleteRootScenery);
                    workspace_processor.send(GraphsWorkspaceAction::GetRootSceneryId);
                }
                NodeEditorCommand::AddNode(node_type) => {
                    println!("NodeEditorCommand::AddNode triggered");
                    workspace_processor.send(GraphsWorkspaceAction::AddOpticNode{
                        node_type: node_type.clone(),
                        graph_id: active_tab()
                    });
                }
                NodeEditorCommand::AddNodeRef(new_ref_node) => {
                    workspace_processor.send(GraphsWorkspaceAction::AddOpticReference{new_ref_node, graph_id: active_tab()});
                }
                NodeEditorCommand::AddAnalyzer(analyzer_type) => {
                    workspace_processor.send(GraphsWorkspaceAction::AddAnalyzer{analyzer_type: analyzer_type.clone(), graph_id: active_tab()});
                }
                NodeEditorCommand::AutoLayout => {
                    workspace_processor.send(GraphsWorkspaceAction::OptimizeLayout{graph_id: active_tab()});
                    workspace_processor.send(GraphsWorkspaceAction::CenterGraph { zoom_to_fit: true , graph_id: active_tab()});
                }
                NodeEditorCommand::CenterGraph { zoom_to_fit } => {
                    workspace_processor.send(GraphsWorkspaceAction::CenterGraph {
                        zoom_to_fit: zoom_to_fit,
                        graph_id: active_tab()
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

    
        
    let current_mouse_pos = use_signal(Point2D::default);
                    
    use_effect(move || {
        workspace_processor.send(GraphsWorkspaceAction::GetRootSceneryId);
    });

    use_effect(move || {
        let is_unsaved = *workspace.peek().needs_saving.read();
        // graph_store.peek().needs_saving();
        // let is_unsaved = *needs_saving_signal.read();
        if *model_modified_sig.peek() != is_unsaved {
            model_modified_handler.call(is_unsaved);
        }
        let current_path = workspace.peek().file_path.read().clone();

        if *model_file_path_sig.peek() != current_path {
            model_file_path_handler.call(current_path);
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

    // use_context_provider(|| graph_state().graph_store);
    // use_context_provider(|| graph_state().editor_state);

    let onmouseleave_handler = use_drag_end(workspace);
    let onkeydownhandler = use_on_key_down(current_mouse_pos, workspace);

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
                                                        move |_| remove_tab_handler.call(id_copy)
                                                    },

                                                }
                                            }
                                        }
                                    }
                                }
                                div { class: "editor-tab-filler" }
                            }
                            for (i , (id , graph_state)) in tabs.iter().enumerate() {
                                TabContent {
                                    key: "{id.as_simple().to_string()}",
                                    value: id.as_simple().to_string(),
                                    index: i,
                                    GraphViewEditor {
                                        onmouseup_handler: EventHandler::new(use_drag_end(workspace)),
                                        command,
                                        model_modified_sig,
                                        model_modified_handler,
                                        model_file_path_sig,
                                        model_file_path_handler,
                                        current_mouse_pos,
                                        add_new_group_tab_handler,
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


pub fn use_workspace_processor(
    mut workspace: Signal<GraphsWorkspaceState>,
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
                        if let Some(graph_state) = workspace.read().tabs.read().get(&*root_graph_id.read()){
                            process_center_graph(*graph_state, false)
                        }
                    }
                    GraphsWorkspaceAction::SaveToFile(path) => {
                        process_save_root_scenery_to_file(path, set_file_path_handler, set_needs_saving_handler).await;
                    }
                    GraphsWorkspaceAction::DeleteRootScenery => {
                        process_delete_root_scenery(clear_workspace_handler).await;
                    }
                    GraphsWorkspaceAction::GetRootSceneryId => {
                        process_get_root_scenery_id(add_new_group_tab_handler, set_root_scenery_id_handler).await;
                    }
                    GraphsWorkspaceAction::AddOpticNode { node_type, graph_id } => {
                        if let Some(graph_state) = workspace.read().tabs.read().get(&graph_id){
                            process_add_optic_node(&node_type, *graph_state).await;
                        }
                    },
                    GraphsWorkspaceAction::AddOpticReference { new_ref_node, graph_id } => {
                        if let Some(graph_state) = workspace.read().tabs.read().get(&graph_id){
                            process_add_reference_node(new_ref_node, *graph_state).await;
                        }
                    },
                    GraphsWorkspaceAction::AddAnalyzer { analyzer_type, graph_id } => 
                        if let Some(graph_state) = workspace.read().tabs.read().get(&graph_id){
                            process_add_analyzer(analyzer_type, *graph_state).await;
                        }
                    GraphsWorkspaceAction::OptimizeLayout { graph_id } => {
                        if let Some(graph_state) = workspace.read().tabs.read().get(&graph_id){
                            process_optimize_layout(*graph_state).await;
                        }
                    },
                    GraphsWorkspaceAction::CenterGraph { zoom_to_fit, graph_id } => {
                        if let Some(graph_state) = workspace.read().tabs.read().get(&graph_id){
                            process_center_graph(*graph_state, zoom_to_fit)
                        }
                    },
                    GraphsWorkspaceAction::UpdateEdges { connections, graph_id } => {
                        if let Some(graph_state) = workspace.write().tabs.write().get_mut(&graph_id){
                            graph_state.write()
                                .graph_store
                                .write()
                                .edges
                                .set(connections.clone());
                            graph_state.write()
                                .graph_store
                                .write()
                                .needs_saving
                                .set(true);
                        }
                    },
                    GraphsWorkspaceAction::InvertNode { inverted, graph_id, node_id } => {
                        if let Some(graph_state) = workspace.write().tabs.write().get_mut(&graph_id){
                            graph_state.write().graph_store.write().set_node_inverted(node_id, inverted);
                        }
                    },
                    GraphsWorkspaceAction::SetNodeName { name, graph_id, node_id } => {
                        if let Some(graph_state) = workspace.write().tabs.write().get_mut(&graph_id){
                            graph_state.write().graph_store.write().set_name_of_node(node_id, name);
                        }
                    }
                    GraphsWorkspaceAction::UpdateEdge { connection, graph_id } => {
                        if let Some(graph_state) = workspace.read().tabs.read().get(&graph_id){
                            process_update_edge(connection, *graph_state).await;
                        }
                    },
                    GraphsWorkspaceAction::DeleteEdge { connection, graph_id } => {
                        if let Some(graph_state) = workspace.read().tabs.read().get(&graph_id){
                            process_delete_edge(connection, *graph_state).await;
                        }
                    },
                    GraphsWorkspaceAction::CopyNode { node_type, node_id } => process_copy_node(node_type, node_id).await,
                    GraphsWorkspaceAction::PasteNode { pos, graph_id } => {
                        if let Some(graph_state) = workspace.read().tabs.read().get(&graph_id){
                            process_paste_node(pos, *graph_state).await;                        
                        }
                    }
                    GraphsWorkspaceAction::SyncNodePosition { node_id, pos, graph_id } => {
                        if let Some(graph_state) = workspace.read().tabs.read().get(&graph_id).cloned(){
                            eval_action_run(
                                api::update_gui_position(node_id, pos).await,
                                Some(move |_| {
                                    let mut graph_store = graph_state.read().graph_store;
                                    graph_store.write().needs_saving.set(true);
                                }),
                            );
                        }
                    },
                    GraphsWorkspaceAction::AddEdge { new_edge, graph_id } => {
                        if let Some(graph_state) = workspace.read().tabs.read().get(&graph_id){
                            process_add_edge(new_edge, *graph_state).await;                      
                        }
                    },
                    GraphsWorkspaceAction::DeleteNode { node_id, graph_id } => {
                        if let Some(graph_state) = workspace.read().tabs.read().get(&graph_id){
                            process_delete_node(node_id, *graph_state).await;                      
                        }
                    }
                }
            }
        }
    })
}

async fn process_add_edge(connect_info: ConnectInfo, graph_state: Signal<GraphState>) {
    let mut graph_store = graph_state.read().graph_store;
    eval_action_run(
        api::post_add_connection(connect_info).await,
        Some(move |ci| {
            graph_store.write().edges_mut().write().push(ci);
            graph_store.write().needs_saving.set(true);
        }),
    );
}

async fn process_delete_analyzer_node(analyzer_id: Uuid, graph_state: Signal<GraphState>) {
    eval_action_run(
        api::delete_analyzer(analyzer_id).await,
        Some(move |deleted_id| {
            let mut graph_store = graph_state.read().graph_store;
            graph_store.write().remove_nodes_by_id(vec![deleted_id]);
        }),
    );
}

async fn process_delete_optical_node(node_id: Uuid, graph_state: Signal<GraphState>) {
    eval_action_run(
        api::delete_node(node_id).await,
        Some(move |deleted_ids| {
            let mut graph_store = graph_state.read().graph_store;
            graph_store.write().remove_nodes_by_id(deleted_ids);
        }),
    );
}

async fn process_delete_node(node_id: Uuid, graph_state: Signal<GraphState>) {
    let mut graph_store = graph_state.read().graph_store;
    let node_type_opt = graph_store.read().get_node_type(node_id);
    if let Some(node_type) = node_type_opt {
        match node_type {
            NodeType::Optical(_) => {
                process_delete_optical_node(node_id, graph_state).await;
            }
            NodeType::Analyzer(_) => {
                process_delete_analyzer_node(node_id, graph_state).await;
            }
        }
        graph_store.write().needs_saving.set(true);
    } else {
        OPOSSUM_UI_LOGS
            .write()
            .add_log("Node could not be deleted, as uuid was not found");
    }
}


async fn process_paste_node(pos: Point2D<f64>, graph_state: Signal<GraphState>) {
    let mut graph_store = graph_state.read().graph_store;
    let group_id = graph_state.read().id;
    match api::get_copied_node_type().await {
        Ok(node_type) => {
            if node_type {
                eval_action_run(
                    api::post_paste_optical_node(group_id, pos).await,
                    Some(move |node_info_opt| {
                        if let Some(node_info) = node_info_opt {
                            graph_store.write().add_new_optical_node(&node_info);
                            graph_store.write().needs_saving.set(true);
                        }
                    }),
                );
            } else {
                eval_action_run(
                    api::post_paste_analyzer_node(pos).await,
                    Some(move |analyzer_info: AnalyzerInfo| {
                        let id = analyzer_info.id();
                        graph_store
                            .write()
                            .add_new_analyzer(NewAnalyzerInfo::from(analyzer_info), id);
                        graph_store.write().needs_saving.set(true);
                    }),
                );
            }
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }
}

async fn process_copy_node(node_type: NodeType, node_id: Uuid) {
    match node_type {
        NodeType::Optical(_) => eval_action_run(
            api::post_copy_optical_node(node_id).await,
            None::<fn(String)>,
        ),
        NodeType::Analyzer(_) => eval_action_run(
            api::post_copy_analyzer_node(node_id).await,
            None::<fn(String)>,
        ),
    }
}

async fn process_delete_edge(connect_info: ConnectInfo, graph_state: Signal<GraphState>) {
    let mut graph_store = graph_state.read().graph_store;
    let edge_to_delete = connect_info.clone();

    eval_action_run(
        api::delete_connection(connect_info).await,
        Some(move |_| {
            graph_store
                .write()
                .edges
                .write()
                .retain(|e| e != &edge_to_delete);
            graph_store.write().needs_saving.set(true);
        }),
    );
}

async fn process_update_edge(connect_info: ConnectInfo, graph_state: Signal<GraphState>) {
    let mut graph_store = graph_state.read().graph_store;
    eval_action_run(
        api::update_distance(connect_info).await,
        Some(move |ci: ConnectInfo| {
            if let Some(e) = graph_store
                .write()
                .edges
                .write()
                .iter_mut()
                .find(|e| e.src_uuid() == ci.src_uuid() && e.target_uuid() == ci.target_uuid())
            {
                *e = ci;
            }
        }),
    );
}

fn process_center_graph(graph_state: Signal<GraphState>, zoom_to_fit: bool) {
    let mut editor_state_signal = graph_state.read().editor_state;
    let bounding_box = graph_state.read().graph_store.read().get_bounding_box();
    editor_state_signal
        .write()
        .center_graph(bounding_box, zoom_to_fit);
}

async fn process_optimize_layout(graph_state: Signal<GraphState>) {
    let mut graph_store = graph_state.read().graph_store;
    let edges = graph_store.read().edges().read().clone();
    eval_action_run(
        optimize_layout_and_sync(edges).await,
        Some(move |new_positions| {
            graph_store.write().update_node_positions(new_positions);
            graph_store.write().needs_saving.set(true);
        }),
    );
}

async fn process_add_analyzer(analyzer_type: AnalyzerType, graph_state: Signal<GraphState>) {
    let editor_state = graph_state.read().editor_state;
    let mut graph_store = graph_state.read().graph_store;

    // calculate center of viewport (in graph coordinates)
    let zoom = *editor_state.peek().zoom.peek();
    let view_port_center = editor_state.peek().get_view_port_center();
    let shift = *editor_state.peek().shift.peek();

    let proposed_element_position = (
        (view_port_center.x - shift.x) / zoom,
        (view_port_center.y - shift.y) / zoom,
    );
    let existing_element_positions: Vec<(f64, f64)> = graph_store.peek().nodes()()
        .values()
        .map(|element| (element.pos().x, element.pos().y))
        .collect();
    let element_position =
        find_suitable_element_position(proposed_element_position, &existing_element_positions);
    let new_analyzer_info = NewAnalyzerInfo::new(analyzer_type, element_position);
    eval_action_run(
        api::post_add_analyzer(new_analyzer_info.clone()).await,
        Some(move |analyzer_id| {
            graph_store
                .write()
                .add_new_analyzer(new_analyzer_info, analyzer_id);
            graph_store.write().needs_saving.set(true);
        }),
    );
}

async fn process_add_reference_node(new_ref_node: NewRefNode, graph_state: Signal<GraphState>) {
    let mut graph_store = graph_state.read().graph_store;
    let scenery_id = graph_state.read().id;
    eval_action_run(
        api::post_add_ref_node(new_ref_node, scenery_id).await,
        Some(move |node_info| {
            graph_store.write().add_new_reference_node(&node_info);
            graph_store.write().needs_saving.set(true);
        }),
    );
}


async fn process_add_optic_node(new_node_type_string: &str, graph_state: Signal<GraphState>) {
    let editor_state = graph_state.read().editor_state;
    let mut graph_store = graph_state.read().graph_store;
    let scenery_id = graph_state.read().id;

    // calculate center of viewport (in graph coordinates)
    let zoom = *editor_state.peek().zoom.peek();
    let view_port_center = editor_state.peek().get_view_port_center();
    let shift = *editor_state.peek().shift.peek();

    let proposed_element_position = (
        (view_port_center.x - shift.x) / zoom,
        (view_port_center.y - shift.y) / zoom,
    );
    let existing_element_positions: Vec<(f64, f64)> = graph_store.peek().nodes()()
        .values()
        .map(|element| (element.pos().x, element.pos().y))
        .collect();
    let element_position =
        find_suitable_element_position(proposed_element_position, &existing_element_positions);
    let new_node_info = NewNode::new(new_node_type_string.to_lowercase(), element_position);
    eval_action_run(
        api::post_add_node(new_node_info, scenery_id).await,
        Some(move |node_info| {
            graph_store.write().add_new_optical_node(&node_info);
            graph_store.write().needs_saving.set(true);
        }),
    );
}


fn find_suitable_element_position(
    proposed_position: (f64, f64),
    existing_element_positions: &[(f64, f64)],
) -> (f64, f64) {
    let mut final_position = proposed_position;
    let min_dist_squared = MIN_NODE_DISTANCE_RADIUS.powi(2);
    for _ in 0..NODE_PLACEMENT_MAX_ITERATIONS {
        let has_collision = existing_element_positions.iter().any(|&(pos_x, pos_y)| {
            let dist_x = final_position.0 - pos_x;
            let dist_y = final_position.1 - pos_y;
            let dist_sq = dist_x.mul_add(dist_x, dist_y * dist_y);
            dist_sq < min_dist_squared
        });
        if has_collision {
            final_position.0 += MIN_NODE_DISTANCE_RADIUS;
            final_position.1 += MIN_NODE_DISTANCE_RADIUS;
        } else {
            return final_position;
        }
    }
    final_position // fallback: return last position after reaching max iterations
}

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
                Some(move |nodes: Vec<NodeInfo>| add_root_scenery_nodes_handler.call(nodes)), 
            );
            eval_action_run(
                api::get_analyzers().await,
                Some(move |analyzers: Vec<AnalyzerInfo>| {
                    add_root_scenery_analyzers_handler.call(analyzers);
                }),
            );
            eval_action_run(
                api::get_connections(scenery_id).await,
                Some(move |connect_infos: Vec<ConnectInfo>| {
                    add_root_scenery_edges_handler.call(connect_infos)
                }),
            );
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }
}

async fn process_delete_root_scenery(clear_workspace_handler: EventHandler<()>) {
    eval_action_run(
        api::delete_scenery().await,
        Some(move |_| {
            clear_workspace_handler.call(());
        }),
    );
}

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
    GetRootSceneryId,
    DeleteRootScenery,
    AddOpticNode{node_type: String, graph_id: Uuid},
    AddOpticReference{new_ref_node: NewRefNode, graph_id: Uuid},
    AddAnalyzer{analyzer_type: AnalyzerType, graph_id: Uuid},
    OptimizeLayout{graph_id: Uuid},
    CenterGraph{zoom_to_fit: bool, graph_id: Uuid},
    UpdateEdges{connections: Vec<ConnectInfo>, graph_id: Uuid},
    UpdateEdge{connection: ConnectInfo, graph_id: Uuid},
    DeleteEdge{connection: ConnectInfo, graph_id: Uuid},
    AddEdge{new_edge: ConnectInfo, graph_id: Uuid},
    InvertNode{inverted: bool, graph_id: Uuid, node_id: Uuid},
    SetNodeName{name: String, graph_id: Uuid, node_id: Uuid},
    CopyNode{node_type: NodeType, node_id: Uuid},
    PasteNode{pos: Point2D<f64>, graph_id: Uuid},
    SyncNodePosition{pos: Point2D<f64>, graph_id: Uuid, node_id: Uuid},
    DeleteNode{node_id: Uuid, graph_id: Uuid},
}

#[component]
pub fn GraphViewEditor(
    // workspace: Signal<GraphsWorkspaceState>,
    onmouseup_handler: EventHandler<Event<MouseData>>,
    command: ReadSignal<Option<NodeEditorCommand>>,
    model_modified_sig: ReadSignal<bool>,
    model_modified_handler: EventHandler<bool>,
    model_file_path_sig: ReadSignal<Option<PathBuf>>,
    model_file_path_handler: EventHandler<Option<PathBuf>>,
    current_mouse_pos: Signal<Point2D<f64>>,
    add_new_group_tab_handler: EventHandler<(String, Uuid)>,
    graph_state: Signal<GraphState>
) -> Element {
    let last_auxiliary_click = use_signal(|| Option::<Instant>::None);
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let editor_state = graph_state.read().editor_state;
    let graph_store = graph_state.read().graph_store;
    let graph_id = graph_state.read().id;

    use_context_provider(|| graph_state);
    use_context_provider(|| editor_state);
    use_context_provider(|| graph_store);
    let mut on_mounted: Signal<Option<Rc<MountedData>>> = use_signal(|| None);
    let onwheel_handler = use_zoom(on_mounted);
    let onmousedown_handler = use_on_mouse_down(current_mouse_pos, last_auxiliary_click);
    let onmousemove_handler = use_drag(current_mouse_pos);
    // let onmouseup_handler = use_drag_end(workspace);
    let onresizehandler = use_on_resize(on_mounted);

    let shift = use_memo(move || *editor_state.read().shift.read());
    let zoom = use_memo(move || *editor_state.read().zoom.read());

    rsx! {
        div {
            class: "graph-editor",
            id: format!("editor_{}", graph_id.as_simple()),
            draggable: false,

            onwheel: onwheel_handler,
            onmousedown: onmousedown_handler,
            onmouseup: move |e| onmouseup_handler.call(e),
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
                Nodes {
                    add_new_group_tab_handler,
                    graph_store,
                    graph_id,
                }
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
