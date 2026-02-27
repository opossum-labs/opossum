#![allow(clippy::derive_partial_eq_without_eq)]
use crate::{OPOSSUM_UI_LOGS, api::{self, eval_action_run}, components::{
    node_editor::NodeConfigEditor,
    scenery_editor::{
        GraphState, GraphStore, NodeElement, NodeType, constants::{MAX_ZOOM, MIN_NODE_DISTANCE_RADIUS, MIN_ZOOM, NODE_PLACEMENT_MAX_ITERATIONS}, edges::edges_component::{
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
use std::{collections::{BTreeMap, HashMap}, fs, path::PathBuf, rc::Rc, time::Instant};
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

#[derive(Clone, Eq, PartialEq, Default)]
pub struct GraphsWorkspaceState {
    pub tabs: Signal<BTreeMap<Uuid, Signal<GraphState>>>,
    pub active_tab: Signal<Option<Uuid>>,
    pub root_scenery_id: Signal<Uuid>,
    pub needs_saving: Signal<bool>,
    pub file_path: Signal<Option<PathBuf>>,
}

impl GraphsWorkspaceState{
    pub fn get_graph_state(&self, graph_id: Uuid) -> Option<ReadSignal<GraphState>>{
        self.tabs.read().get(&graph_id).map(|g| (*g).into())
    }
    pub fn get_graph_store(&self, graph_id: Uuid) -> Option<ReadSignal<GraphStore>>{
        self.tabs.read().get(&graph_id).map(|g| g.read().graph_store.into())
    }
    pub fn get_editor_state(&self, graph_id: Uuid) -> Option<ReadSignal<EditorState>>{
        self.tabs.read().get(&graph_id).map(|g| g.read().editor_state.into())
    }
    pub fn get_graph_edges(&self, graph_id: Uuid) -> Option<ReadSignal<Vec<ConnectInfo>>>{
        self.tabs.read().get(&graph_id).map(|g| g.read().graph_store.read().edges().into())
    }
    pub fn get_graph_nodes(&self, graph_id: Uuid) -> Option<ReadSignal<HashMap<Uuid, NodeElement>>>{
        self.tabs.read().get(&graph_id).map(|g| g.read().graph_store.read().nodes().into())
    }
    pub fn get_graph_bounding_box(&self, graph_id: Uuid) -> Option<Rect<f64, UnknownUnit>>{
        self.tabs.read().get(&graph_id).map(|g| g.read().graph_store.read().get_bounding_box())
    }
}

#[derive(Clone, PartialEq, Copy)]
pub struct WorkSpaceSignalHandlers {
    pub add_new_group_tab: EventHandler<(String, Uuid)>,
    pub set_root_scenery_id: EventHandler<Uuid>,
    pub remove_tab: EventHandler<Uuid>,
    pub set_needs_saving: EventHandler<bool>,
    pub clear_workspace: EventHandler<()>,
    pub add_root_scenery_nodes: EventHandler<Vec<NodeInfo>>,
    pub add_root_scenery_analyzers: EventHandler<Vec<AnalyzerInfo>>,
    pub add_root_scenery_edges: EventHandler<Vec<ConnectInfo>>,
    pub set_active_tab: EventHandler<Option<Uuid>>,
    pub add_optical_node: EventHandler<(NodeInfo, Uuid)>,
    pub add_reference_node: EventHandler<(NodeInfo, Uuid)>,
    pub add_analyzer_node: EventHandler<(NewAnalyzerInfo, Uuid, Uuid)>,
    pub remove_nodes: EventHandler<(Vec<Uuid>, Uuid)>,
    pub update_node_positions: EventHandler<(HashMap<Uuid, Point2D<f64>>, Uuid)>,
    pub add_edge: EventHandler<(ConnectInfo, Uuid)>,
    pub delete_edge: EventHandler<(ConnectInfo, Uuid)>,
    pub update_edge: EventHandler<(ConnectInfo, Uuid)>,
    pub center_graph: EventHandler<(Rect<f64, UnknownUnit>, bool, Uuid, bool)>,
    pub invert_node: EventHandler<(Uuid, bool, Uuid)>,
    pub update_edges: EventHandler<(Vec<ConnectInfo>, Uuid)>,
    pub set_node_name: EventHandler<(String, Uuid, Uuid)>
}

impl WorkSpaceSignalHandlers {
    pub fn new(
        workspace: Signal<GraphsWorkspaceState>,
    ) -> Self {
        let add_new_group_tab = {
            let mut workspace = workspace;
            EventHandler::new(move |(title, id): (String, Uuid)| {
                let mut graph_state = GraphState::default();
                graph_state.id = id;
                graph_state.name = title;

                workspace
                    .write()
                    .tabs
                    .write()
                    .insert(id, Signal::new(graph_state));

                workspace.write().active_tab.set(Some(id));
            })
        };

        let set_node_name = {
            let mut workspace = workspace;
            EventHandler::new(move |(name, node_id, graph_id)| {
                if let Some(graph_state) = workspace.write().tabs.write().get_mut(&graph_id){
                    graph_state.write().graph_store.write().set_name_of_node(node_id, name);
                }
            })
        };

        let set_root_scenery_id = {
            let mut workspace = workspace;
            EventHandler::new(move |id: Uuid| {
                workspace.write().root_scenery_id.set(id);
            })
        };

        let remove_tab = {
            let mut workspace = workspace;
            EventHandler::new(move |id: Uuid| {
                workspace.write().tabs.write().remove(&id);
            })
        };

        let set_needs_saving = {
            let mut workspace = workspace;
            EventHandler::new(move |needs_saving: bool| {
                workspace.write().needs_saving.set(needs_saving);
            })
        };

        let clear_workspace = {
            let mut workspace = workspace;
            EventHandler::new(move |()| {
                workspace.set(GraphsWorkspaceState::default());
            })
        };

        let update_node_positions = {
            let mut workspace = workspace;
            EventHandler::new(move |(new_positions, graph_id): (HashMap<Uuid, Point2D<f64>>, Uuid)| {
                let mut workspace_write = workspace.write();
                if let Some(graph_state) = workspace_write.tabs.write().get_mut(&graph_id){
                    graph_state.write().graph_store.write().update_node_positions(new_positions);
                }
                workspace_write.needs_saving.set(true);
            })
        };

        let add_edge = {
            let mut workspace = workspace;
            EventHandler::new(move |(connect_info , graph_id): (ConnectInfo, Uuid)| {
                let mut workspace_write = workspace.write();
                if let Some(graph_state) = workspace_write.tabs.write().get_mut(&graph_id){
                    graph_state.write().graph_store.write().edges_mut().write().push(connect_info);
                }
                workspace_write.needs_saving.set(true);
            })
        };

        let delete_edge = {
            let mut workspace = workspace;
            EventHandler::new(move |(edge_to_delete , graph_id): (ConnectInfo, Uuid)| {
                let mut workspace_write = workspace.write();
                if let Some(graph_state) = workspace_write.tabs.write().get_mut(&graph_id){
                    graph_state.write().graph_store
                .write()
                .edges
                .write()
                .retain(|e| e != &edge_to_delete);
                }
                workspace_write.needs_saving.set(true);
            })
        };

        
        let update_edge = {
            let mut workspace = workspace;
            EventHandler::new(move |(ci , graph_id): (ConnectInfo, Uuid)| {
                let mut workspace_write = workspace.write();
                if let Some(graph_state) = workspace_write.tabs.write().get_mut(&graph_id){
                    if let Some(e) =graph_state.write().graph_store
                .write()
                .edges
                .write()
                .iter_mut()
                .find(|e| e.src_uuid() == ci.src_uuid() && e.target_uuid() == ci.target_uuid())
                {
                *e = ci;
            }
                }
                workspace_write.needs_saving.set(true);
            })
        };

        let update_edges = {
            let mut workspace = workspace;
            EventHandler::new(move |(connections , graph_id): (Vec<ConnectInfo>, Uuid)| {
                let mut workspace_write = workspace.write();
                if let Some(graph_state) = workspace_write.tabs.write().get_mut(&graph_id){
                    graph_state.write()
                                .graph_store
                                .write()
                                .edges
                                .set(connections);
                }
                workspace_write.needs_saving.set(true);
            })
        };

        let add_optical_node = {
            let mut workspace = workspace;
            EventHandler::new(move |(node_info , graph_id): (NodeInfo, Uuid)| {
                let mut workspace_write = workspace.write();
                if let Some(graph_state) = workspace_write.tabs.write().get_mut(&graph_id){
                    graph_state.write().graph_store.write().add_new_optical_node(&node_info);
                }
                workspace_write.needs_saving.set(true);
            })
        };

        let add_analyzer_node = {
            let mut workspace = workspace;
            EventHandler::new(move |(analyzer_info, analyzer_id , graph_id): (NewAnalyzerInfo, Uuid, Uuid)| {
                let mut workspace_write = workspace.write();
                if let Some(graph_state) = workspace_write.tabs.write().get_mut(&graph_id){
                    graph_state.write().graph_store.write().add_new_analyzer(analyzer_info, analyzer_id);
                }
                workspace_write.needs_saving.set(true);
            })
        };

        let add_reference_node = {
            let mut workspace = workspace;
            EventHandler::new(move |(node_info , graph_id): (NodeInfo, Uuid)| {
                let mut workspace_write = workspace.write();
                if let Some(graph_state) = workspace_write.tabs.write().get_mut(&graph_id){
                    graph_state.write().graph_store.write().add_new_reference_node(&node_info);
                }
                workspace_write.needs_saving.set(true);
            })
        };

        let remove_nodes = {
            let mut workspace = workspace;
            EventHandler::new(move |(node_ids , graph_id): (Vec<Uuid>, Uuid)| {
                let mut workspace_write = workspace.write();
                if let Some(graph_state) = workspace_write.tabs.write().get_mut(&graph_id){
                    graph_state.write().graph_store.write().remove_nodes_by_id(node_ids);
                }
                workspace_write.needs_saving.set(true);
            })
        };

        let add_root_scenery_nodes = {
            let mut workspace = workspace;
            EventHandler::new(move |nodes: Vec<NodeInfo>| {
                let id = *workspace.read().root_scenery_id.read();
                if let Some(graph_state) =
                    workspace.write().tabs.write().get_mut(&id)
                {
                    graph_state
                        .write()
                        .graph_store
                        .write()
                        .add_nodes(&nodes);
                } else {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log("no root scenery found! Cannot add nodes!");
                }
            })
        };

        let invert_node = {
            let mut workspace = workspace;
            EventHandler::new(move |(node_id, inverted , graph_id): (Uuid, bool, Uuid)| {
                let mut workspace_write = workspace.write();
                if let Some(graph_state) = workspace_write.tabs.write().get_mut(&graph_id){
                    graph_state.write().graph_store.write().set_node_inverted(node_id, inverted);
                }
                workspace_write.needs_saving.set(true);
            })
        };

        let add_root_scenery_analyzers = {
            let mut workspace = workspace;
            EventHandler::new(move |analyzers: Vec<AnalyzerInfo>| {
                let id = *workspace.read().root_scenery_id.read();

                if let Some(graph_state) =
                    workspace.write().tabs.write().get_mut(&id)
                {
                    graph_state
                        .write()
                        .graph_store
                        .write()
                        .add_analyzers(&analyzers);
                } else {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log("no root scenery found! Cannot add analyzers!");
                }
            })
        };

        let add_root_scenery_edges = {
            let mut workspace = workspace;
            EventHandler::new(move |connect_infos: Vec<ConnectInfo>| {
                let id = *workspace.read().root_scenery_id.read();

                if let Some(graph_state) =
                    workspace.write().tabs.write().get_mut(&id)
                {
                    graph_state
                        .write()
                        .graph_store
                        .write()
                        .edges_mut()
                        .set(connect_infos);
                } else {
                    OPOSSUM_UI_LOGS
                        .write()
                        .add_log("no root scenery found! Cannot add edges!");
                }
            })
        };

        let center_graph = {
            let mut workspace = workspace;
            EventHandler::new(move |(bounding_box, zoom_to_fit , graph_id, save_changes): (Rect<f64, UnknownUnit>, bool, Uuid, bool)| {
                let mut workspace_write = workspace.write();
                if let Some(graph_state) = workspace_write.tabs.write().get_mut(&graph_id){
                    println!("centering graph! bounding box: {:?}", bounding_box);
                    graph_state.write().editor_state.write().center_graph(bounding_box, zoom_to_fit);
                }
                if save_changes{
                    workspace_write.needs_saving.set(true);
                }
            })
        };

        let set_active_tab = {
            let mut workspace = workspace;
            EventHandler::new(move |active_tab: Option<Uuid>| {
                workspace.write().active_tab.set(active_tab);
            })
        };


        Self {
            add_new_group_tab,
            set_root_scenery_id,
            remove_tab,
            set_needs_saving,
            clear_workspace,
            add_root_scenery_nodes,
            add_root_scenery_analyzers,
            add_root_scenery_edges,
            set_active_tab,
            add_optical_node,
            add_reference_node,
            add_analyzer_node,
            remove_nodes, 
            update_node_positions,
            add_edge, 
            center_graph,
            delete_edge,
            update_edge, 
            invert_node, 
            update_edges,
            set_node_name
        }
    }
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

    let workspace = use_signal(|| GraphsWorkspaceState::default());
    let root_graph_id = use_memo(move || *workspace.read().root_scenery_id.read());

    let workspace_handlers = WorkSpaceSignalHandlers::new(workspace);

    // //Handler definition
    // let add_new_group_tab_handler = EventHandler::new(move |(title, id): (String, Uuid)|{
    //     let mut graph_state = GraphState::default();
    //     graph_state.id = id;
    //     graph_state.name = title;
    //     workspace.write().tabs.write().insert(id, Signal::new(graph_state));
    //     workspace.write().active_tab.set(Some(id));
    // });
    // let set_root_scenery_id_handler = EventHandler::new(move | id: Uuid|{
    //     workspace.write().root_scenery_id.set(id);
    // });
    // let remove_tab_handler = EventHandler::new(move |id: Uuid|{
    //     workspace.write().tabs.write().remove(&id);
    // });
    // let set_needs_saving_handler = EventHandler::new(move | needs_saving: bool|{
    //     workspace.write().needs_saving.set(needs_saving);
    // });
    // let clear_workspace_handler = EventHandler::new(move |()|{
    //     workspace.set(GraphsWorkspaceState::default());
    // });
    // let add_root_scenery_nodes_handler = EventHandler::new(move | nodes: Vec<NodeInfo>|{
    //     let id = *root_graph_id.read();
    //     if let Some(graph_state) = workspace.write().tabs.write().get_mut(&id){
    //         graph_state.write().graph_store.write().add_nodes(&nodes);
    //     }
    //     else{
    //         OPOSSUM_UI_LOGS.write().add_log("no root scenery found! Cannot add nodes!");
    //     }
    // });
    // let add_root_scenery_analyzers_handler = EventHandler::new(move | analyzers: Vec<AnalyzerInfo>|{
    //     let id = *root_graph_id.read();
    //     if let Some(graph_state) = workspace.write().tabs.write().get_mut(&id){
    //         graph_state.write().graph_store.write().add_analyzers(&analyzers);
    //     }
    //     else{
    //         OPOSSUM_UI_LOGS.write().add_log("no root scenery found! Cannot add analyzers!");
    //     }
    // });
    // let add_root_scenery_edges_handler = EventHandler::new(move | connect_infos: Vec<ConnectInfo>|{
    //     let id = *root_graph_id.read();
    //     if let Some(graph_state) = workspace.write().tabs.write().get_mut(&id){
    //         graph_state.write().graph_store.write().edges_mut().set(connect_infos);
    //     }
    //     else{
    //         OPOSSUM_UI_LOGS.write().add_log("no root scenery found! Cannot add edges!");
    //     }
    // });

    // let set_active_tab_handler = EventHandler::new(move |active_tab: Option<Uuid>|{
    //     workspace.write().active_tab.set(active_tab);
    // });

    let workspace_processor = use_workspace_processor(
        workspace.into(), 
        root_graph_id.into(), 
        workspace_handlers,
        model_file_path_handler
);


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
                    workspace_processor.send(GraphsWorkspaceAction::CenterGraph { zoom_to_fit: true , graph_id: active_tab(), save_changes: true});
                }
                NodeEditorCommand::CenterGraph { zoom_to_fit } => {
                    workspace_processor.send(GraphsWorkspaceAction::CenterGraph {
                        zoom_to_fit: zoom_to_fit,
                        graph_id: active_tab(),
                        save_changes: true
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
                                        add_new_group_tab_handler: workspace_handlers.add_new_group_tab,
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
    workspace: ReadSignal<GraphsWorkspaceState>,
    root_graph_id: Memo<Uuid>, 
    workspace_handlers: WorkSpaceSignalHandlers,
    set_file_path_handler: EventHandler<Option<PathBuf>>
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
                            workspace_handlers,
                            set_file_path_handler
                        ).await;
                    }
                    GraphsWorkspaceAction::SaveToFile(path) => {
                        process_save_root_scenery_to_file(path, set_file_path_handler, workspace_handlers).await;
                    }
                    GraphsWorkspaceAction::DeleteRootScenery => {
                        process_delete_root_scenery(workspace_handlers).await;
                    }
                    GraphsWorkspaceAction::GetRootSceneryId => {
                        process_get_root_scenery_id(workspace_handlers).await;
                    }
                    GraphsWorkspaceAction::AddOpticNode { node_type, graph_id } => {
                        process_add_optic_node(&node_type, workspace, workspace_handlers, graph_id).await;
                    },
                    GraphsWorkspaceAction::AddOpticReference { new_ref_node, graph_id } => {
                        process_add_reference_node(new_ref_node, workspace_handlers, graph_id).await;
                    },
                    GraphsWorkspaceAction::AddAnalyzer { analyzer_type, graph_id } => {
                        process_add_analyzer(analyzer_type, workspace, workspace_handlers, graph_id).await;
                    }
                    GraphsWorkspaceAction::OptimizeLayout { graph_id } => {
                        process_optimize_layout(workspace, workspace_handlers, graph_id).await;
                    },
                    GraphsWorkspaceAction::CenterGraph { zoom_to_fit, graph_id , save_changes} => {
                        process_center_graph(workspace, workspace_handlers, graph_id, zoom_to_fit, save_changes)
                    },
                    GraphsWorkspaceAction::UpdateEdges { connections, graph_id } => {
                        workspace_handlers.update_edges.call((connections, graph_id))
                    },
                    GraphsWorkspaceAction::InvertNode { inverted, graph_id, node_id } => {
                        workspace_handlers.invert_node.call((node_id, inverted, graph_id))
                    },
                    GraphsWorkspaceAction::SetNodeName { name, graph_id, node_id } => {
                        workspace_handlers.set_node_name.call((name, node_id, graph_id))
                    }
                    GraphsWorkspaceAction::UpdateEdge { connection, graph_id } => {
                        process_update_edge(connection, workspace_handlers, graph_id).await;
                    },
                    GraphsWorkspaceAction::DeleteEdge { connection, graph_id } => {
                        process_delete_edge(connection, workspace_handlers, graph_id).await;
                    },
                    GraphsWorkspaceAction::CopyNode { node_type, node_id } => process_copy_node(node_type, node_id).await,
                    GraphsWorkspaceAction::PasteNode { pos, graph_id } => {
                        process_paste_node(pos, workspace_handlers, graph_id).await;                        
                    }
                    GraphsWorkspaceAction::SyncNodePosition { node_id, pos, graph_id } => {
                        eval_action_run(
                            api::update_gui_position(node_id, pos).await,
                            Some(move |_| {
                                workspace_handlers.set_needs_saving.call(true);
                            }),
                        );
                    },
                    GraphsWorkspaceAction::AddEdge { new_edge, graph_id } => {
                        process_add_edge(new_edge, workspace_handlers, graph_id).await;                      

                    },
                    GraphsWorkspaceAction::DeleteNode { node_id, graph_id } => {
                        process_delete_node(node_id, workspace, workspace_handlers, graph_id).await;                      
                    }
                }
            }
        }
    })
}

async fn process_add_edge(connect_info: ConnectInfo, ws_handler: WorkSpaceSignalHandlers, graph_id: Uuid) {
    eval_action_run(
        api::post_add_connection(connect_info, graph_id).await,
        Some(move |ci| {
            ws_handler.add_edge.call((ci, graph_id));
        }),
    );
}

async fn process_delete_node(node_id: Uuid, workspace: ReadSignal<GraphsWorkspaceState>, ws_handler: WorkSpaceSignalHandlers, graph_id: Uuid) {
    let node_type_to_delete = {
        let graph = workspace
                .read()
                .tabs
                .read()
                .get(&graph_id)
                .cloned();

        let Some(graph) = graph else {
            OPOSSUM_UI_LOGS
                .write()
                .add_log(&format!("No graph with id '{}' found", graph_id.as_simple()));
            return;
        };

        graph.read().graph_store.read().get_node_type(node_id)
    };
    if let Some(node_type) = node_type_to_delete {
        match node_type {
            NodeType::Optical(_) => {
                    eval_action_run(
                    api::delete_node(node_id).await,
                    Some(move |deleted_ids| {
                        ws_handler.remove_nodes.call((deleted_ids, graph_id));
                    }),
                );
            }
            NodeType::Analyzer(_) => {
                eval_action_run(
                    api::delete_analyzer(node_id).await,
                    Some(move |deleted_id| {
                        ws_handler.remove_nodes.call((vec![deleted_id], graph_id));
                    }),
                );
            }
        }
    } else {
        OPOSSUM_UI_LOGS
            .write()
            .add_log("Node could not be deleted, as uuid was not found");
    }
}


async fn process_paste_node(pos: Point2D<f64>, ws_handler: WorkSpaceSignalHandlers, graph_id: Uuid) {
    match api::get_copied_node_type().await {
        Ok(node_type) => {
            if node_type {
                eval_action_run(
                    api::post_paste_optical_node(graph_id, pos).await,
                    Some(move |node_info_opt| {
                        if let Some(node_info) = node_info_opt {
                            ws_handler.add_optical_node.call((node_info, graph_id))
                        }
                    }),
                );
            } else {
                eval_action_run(
                    api::post_paste_analyzer_node(pos).await,
                    Some(move |analyzer_info: AnalyzerInfo| {
                        let analyzer_id = analyzer_info.id();
                        ws_handler.add_analyzer_node.call((NewAnalyzerInfo::from(analyzer_info), analyzer_id, graph_id))
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

async fn process_delete_edge(connect_info: ConnectInfo, ws_handler: WorkSpaceSignalHandlers, graph_id: Uuid) {
    eval_action_run(
        api::delete_connection(connect_info.clone(), graph_id).await,
        Some(move |_| {
            ws_handler.delete_edge.call((connect_info, graph_id))
        }),
    );
}

async fn process_update_edge(connect_info: ConnectInfo, ws_handler: WorkSpaceSignalHandlers, graph_id: Uuid) {
    eval_action_run(
        api::update_distance(connect_info, graph_id).await,
        Some(move |ci: ConnectInfo| {
            ws_handler.update_edge.call((ci, graph_id))
            
        }),
    );
}

fn process_center_graph(workspace: ReadSignal<GraphsWorkspaceState>, ws_handler: WorkSpaceSignalHandlers, graph_id: Uuid, zoom_to_fit: bool, save_changes: bool) {
    let Some(bounding_box) = workspace.read().get_graph_bounding_box(graph_id)
    else {
            OPOSSUM_UI_LOGS
                .write()
                .add_log(&format!("No graph with id '{}' found", graph_id.as_simple()));
            return;
        };
    ws_handler.center_graph.call((bounding_box, zoom_to_fit, graph_id, save_changes))
    
}

async fn process_optimize_layout(workspace: ReadSignal<GraphsWorkspaceState>, ws_handler: WorkSpaceSignalHandlers, graph_id: Uuid) {
    let Some(edges) = workspace
            .peek()
            .tabs
            .peek()
            .get(&graph_id).map(|g|g.read().graph_store.read().edges().read().clone())else {
            OPOSSUM_UI_LOGS
                .write()
                .add_log(&format!("No graph with id '{}' found", graph_id.as_simple()));
            return;
        };

    eval_action_run(
        optimize_layout_and_sync(edges).await,
        Some(move |new_positions| {
            ws_handler.update_node_positions.call((new_positions, graph_id))
        }),
    );
}

async fn process_add_analyzer(analyzer_type: AnalyzerType, workspace: ReadSignal<GraphsWorkspaceState>, ws_handler: WorkSpaceSignalHandlers, graph_id: Uuid) {
    // ----- READ PHASE -----
    let new_analyzer_info = {
        let graph = workspace
            .peek()
            .tabs
            .peek()
            .get(&graph_id)
            .cloned();

        let Some(graph) = graph else {
            OPOSSUM_UI_LOGS
                .write()
                .add_log(&format!("No graph with id '{}' found", graph_id.as_simple()));
            return;
        };

        let editor_state = *graph.peek().editor_state.peek();
        let graph_store = *graph.peek().graph_store.peek();

        let zoom = *editor_state.zoom.peek();
        let shift = *editor_state.shift.peek();
        let center = editor_state.get_view_port_center();

        let proposed_pos = (
            (center.x - shift.x) / zoom,
            (center.y - shift.y) / zoom,
        );

        let existing_positions: Vec<_> = graph_store
            .nodes()()
            .values()
            .map(|n| (n.pos().x, n.pos().y))
            .collect();

        let final_pos =
            find_suitable_element_position(proposed_pos, &existing_positions);

        NewAnalyzerInfo::new(analyzer_type, final_pos)
    };

    eval_action_run(
        api::post_add_analyzer(new_analyzer_info.clone()).await,
        Some(move |analyzer_id| {
            ws_handler.add_analyzer_node.call((new_analyzer_info, analyzer_id, graph_id))
        }),
    );
}

async fn process_add_reference_node(new_ref_node: NewRefNode, ws_handler: WorkSpaceSignalHandlers, graph_id: Uuid) {
    let result = api::post_add_ref_node(new_ref_node, graph_id).await;
    eval_action_run(
        result,
        Some(move |node_info| {
            ws_handler.add_reference_node.call((node_info, graph_id))
        }),
    );
}


async fn process_add_optic_node(new_node_type_string: &str, workspace: ReadSignal<GraphsWorkspaceState>, ws_handler: WorkSpaceSignalHandlers, graph_id: Uuid) {
    // ----- READ PHASE -----
    let new_node_info = {
        let graph = workspace
            .peek()
            .tabs
            .peek()
            .get(&graph_id)
            .cloned();

        let Some(graph) = graph else {
            OPOSSUM_UI_LOGS
                .write()
                .add_log(&format!("No graph with id '{}' found", graph_id.as_simple()));
            return;
        };

        let editor_state = *graph.peek().editor_state.peek();
        let graph_store = *graph.peek().graph_store.peek();

        let zoom = *editor_state.zoom.peek();
        let shift = *editor_state.shift.peek();
        let center = editor_state.get_view_port_center();

        let proposed_pos = (
            (center.x - shift.x) / zoom,
            (center.y - shift.y) / zoom,
        );

        let existing_positions: Vec<_> = graph_store
            .nodes()()
            .values()
            .map(|n| (n.pos().x, n.pos().y))
            .collect();

        let final_pos =
            find_suitable_element_position(proposed_pos, &existing_positions);

        NewNode::new(new_node_type_string.to_lowercase(), final_pos)
    };

    // ----- ASYNC PHASE -----
    let result = api::post_add_node(new_node_info, graph_id).await;

    // ----- WRITE PHASE -----
    eval_action_run(result, Some(move |node_info| {
        ws_handler
            .add_optical_node
            .call((node_info, graph_id));
    }));
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
    scenery_id_sig: Memo<Uuid>,
    ws_handler: WorkSpaceSignalHandlers,
    set_file_path_handler: EventHandler<Option<PathBuf>>
) {
    // let old_root_editor = workspace.read().get_editor_state(scenery_id_sig()).unwrap().read().
    process_delete_root_scenery(ws_handler).await;
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
            process_get_root_scenery_id(ws_handler).await;
            ws_handler.set_needs_saving.call(false);
            set_file_path_handler.call(Some(path));
            let scenery_id = *scenery_id_sig.read();
            eval_action_run(
                api::get_nodes(scenery_id).await,
                Some(move |nodes: Vec<NodeInfo>| ws_handler.add_root_scenery_nodes.call(nodes)), 
            );
            eval_action_run(
                api::get_analyzers().await,
                Some(move |analyzers: Vec<AnalyzerInfo>| {
                    ws_handler.add_root_scenery_analyzers.call(analyzers);
                }),
            );
            eval_action_run(
                api::get_connections(scenery_id).await,
                Some(move |connect_infos: Vec<ConnectInfo>| {
                    ws_handler.add_root_scenery_edges.call(connect_infos)
                }),
            );
        }
        Err(err_str) => {
            OPOSSUM_UI_LOGS.write().add_log(&err_str);
        }
    }
}

async fn process_delete_root_scenery(workspace_handlers: WorkSpaceSignalHandlers) {
    eval_action_run(
        api::delete_scenery().await,
        Some(move |_| {
            workspace_handlers.clear_workspace.call(());
        }),
    );
}

async fn process_save_root_scenery_to_file(path: PathBuf, set_file_path_handler: EventHandler<Option<PathBuf>>, ws_handler: WorkSpaceSignalHandlers) {
    eval_action_run(
        api::get_opm_file().await,
        Some(move |opm_string| {
            if let Err(err_str) = fs::write(&path, opm_string) {
                OPOSSUM_UI_LOGS.write().add_log(&err_str.to_string());
            } else {
                set_file_path_handler.call(Some(path));
                ws_handler.set_needs_saving.call(false);
            }
        }),
    );
}

async fn process_get_root_scenery_id(ws_handler: WorkSpaceSignalHandlers) {
    eval_action_run(
        api::get_scenery_uuid().await,
        Some(move |id| {
            ws_handler.set_root_scenery_id.call(id);
            ws_handler.add_new_group_tab.call(("Main Graph".to_string(), id));        }),
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
    CenterGraph{zoom_to_fit: bool, graph_id: Uuid, save_changes: bool},
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

    use_effect(move ||{
        workspace_processor.send(GraphsWorkspaceAction::CenterGraph { zoom_to_fit: false, graph_id, save_changes: false });
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
