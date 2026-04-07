use dioxus::{
    html::{
        MouseEvent, PointerInteraction, geometry::euclid::default::Point2D, input_data::MouseButton,
    },
    prelude::*,
};
use opossum_core::prelude::PortType;
use uuid::Uuid;

use crate::{
    CONTEXT_MENU,
    components::{
        context_menu::cx_menu::{CxMenu, CxtCommand},
        scenery_editor::{
            DragStatus, EditorState, GraphState, GraphStore, GraphsWorkspaceAction,
            GraphsWorkspaceState,
            edges::edges_component::{EdgePort, NewEdgeCreationStart},
        },
    },
};

pub fn use_on_mouse_down(
    node_id: Uuid,
    port_name: String,
    port_type: PortType,
    abs_port_position: Point2D<f64>,
) -> EventHandler<MouseEvent> {
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    EventHandler::new(move |event: MouseEvent| {
        if Some(MouseButton::Primary) == event.trigger_button() {
            event.stop_propagation();
            let drag_status = DragStatus::Edge(NewEdgeCreationStart {
                src_node: node_id,
                src_port: port_name.clone(),
                src_port_type: port_type,
                start_pos: abs_port_position,
            });
            workspace_processor.send(GraphsWorkspaceAction::SetDragStatus(drag_status));
        }
    })
}

pub fn use_on_mouse_leave(editor_status: ReadSignal<EditorState>) -> EventHandler<MouseEvent> {
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let graph_id = use_context::<ReadSignal<GraphState>>().read().graph_info.id;

    EventHandler::new(move |event: MouseEvent| {
        let edge_increation = editor_status.read().edge_in_creation.read().clone();
        event.stop_propagation();
        if let Some(mut edge_in_creation) = edge_increation {
            edge_in_creation.set_end_port(None);
            workspace_processor.send(GraphsWorkspaceAction::SetEdgeInCreation {
                graph_id,
                edge_in_creation: Some(edge_in_creation),
            });
        }
    })
}

pub fn use_on_mouse_enter(
    editor_status: ReadSignal<EditorState>,
    port_name: &str,
    node_id: Uuid,
    port_type: PortType,
    is_mapped_port: bool,
) -> EventHandler<MouseEvent> {
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let graph_id = use_context::<ReadSignal<GraphState>>().read().graph_info.id;
    EventHandler::new({
        let port_name = port_name.to_owned();
        move |event: MouseEvent| {
            let edge_increation = editor_status.read().edge_in_creation.read().clone();
            if let Some(mut edge_in_creation) = edge_increation
                && !is_mapped_port
            {
                event.stop_propagation();

                edge_in_creation.set_end_port(Some(EdgePort {
                    node_id,
                    port_name: port_name.clone(),
                    port_type,
                }));

                workspace_processor.send(GraphsWorkspaceAction::SetEdgeInCreation {
                    graph_id,
                    edge_in_creation: Some(edge_in_creation),
                });
            }
        }
    })
}

pub fn use_on_context_menu(
    workspace: ReadSignal<GraphsWorkspaceState>,
    graph_store: ReadSignal<GraphStore>,
    node_id: Uuid,
    port_name: String,
    port_type: PortType,
) -> EventHandler<MouseEvent> {
    EventHandler::new(move |event: MouseEvent| {
        event.prevent_default();
        event.stop_propagation();
        let x_coord = event.page_coordinates().x;
        let y_coord = event.page_coordinates().y;

        let active_tab = *workspace.read().active_tab.read();
        let root_tab = *workspace.read().root_scenery_id.read();
        if active_tab != root_tab {
            let mut cx_menu = CxMenu::new(x_coord, y_coord, vec![]);

            let mapped_external_port_opt = graph_store
                .read()
                .mapped_ports
                .read()
                .external_port_of_mapped_port(node_id, &port_name);
            if let Some(group_port_name) = mapped_external_port_opt {
                let remove_entry = (
                    "Remove port map from group".to_owned(),
                    CxtCommand::RemovePortMap {
                        group_id: active_tab,
                        group_port_name,
                        port_type,
                    },
                );
                cx_menu.add_entry(remove_entry);
            } else {
                let add_entry = (
                    "Map port to group".to_owned(),
                    CxtCommand::MapNodePort {
                        port_type,
                        group_port_name: Uuid::new_v4().as_simple().to_string(),
                        mapped_node_port_name: port_name.clone(),
                        mapped_node_id: node_id,
                        group_id: active_tab,
                    },
                );
                cx_menu.add_entry(add_entry);
            }
            let mut ctx = CONTEXT_MENU.write();
            *ctx = Some(cx_menu);
        }
    })
}
