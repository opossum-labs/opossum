#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::scenery_editor::{
    DragStatus, EditorStateStoreExt, GraphsWorkspaceState, GraphsWorkspaceStateStoreExt, NodeType,
    SelectionBoxComponent,
    edges::edges_component::{EdgeCreationComponent, EdgesComponent},
    graph_editor::{
        BreadCrumbs,
        hooks::{use_drag, use_drag_end, use_on_mouse_down, use_zoom},
    },
    graph_workspace::{GraphState, GraphStateStoreExt, GraphStoreStoreExt, GraphsWorkspaceAction},
    node::Node,
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use std::{collections::HashSet, path::PathBuf};
use uuid::Uuid;
use web_time::Instant;

#[component]
pub fn GraphViewEditor(
    model_modified_sig: ReadSignal<bool>,
    model_modified_handler: EventHandler<bool>,
    model_file_path: ReadSignal<Option<PathBuf>>,
    model_file_path_handler: EventHandler<Option<PathBuf>>,
    current_mouse_pos: Signal<Point2D<f64>>,
    graph_state: ReadStore<GraphState>,
    ctrl_pressed: ReadSignal<bool>,
    shift_pressed: ReadSignal<bool>,
) -> Element {
    let workspace = use_context::<ReadStore<GraphsWorkspaceState>>();
    let graph_id = graph_state.graph_info().read().id;
    let last_auxiliary_click = use_signal(|| Option::<Instant>::None);
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let editor_state = graph_state.editor_state();
    let graph_store = graph_state.graph_store();
    use_context_provider(|| graph_state);
    let onwheel_handler = use_zoom();
    let onmousemove_handler = use_drag(current_mouse_pos);
    let onmousedown_handler = use_on_mouse_down(
        current_mouse_pos,
        last_auxiliary_click,
        ctrl_pressed,
        graph_id,
    );

    let nodes_in_selection = use_memo(move || {
        let selection_box = *workspace.selection_box().read();
        let nodes = graph_store.nodes().peek().clone();

        selection_box.map_or_else(HashSet::<Uuid>::new, |select_box| {
            nodes
                .iter()
                .filter_map(|(id, node)| {
                    let rect = node.get_bounding_box();
                    if select_box.intersects(&rect) {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect::<HashSet<Uuid>>()
        })
    });

    let shift = use_memo(move || *editor_state.shift().read());
    let zoom = use_memo(move || *editor_state.zoom().read());

    let mouse_pos_in_editor = use_memo(move || {
        let editor_origin = workspace.editor_area().peek().origin;
        Point2D::new(
            (current_mouse_pos.read().x - editor_origin.x - shift.peek().x) / *zoom.peek(),
            (current_mouse_pos.read().y - editor_origin.y - shift.peek().y) / *zoom.peek(),
        )
    });

    use_effect(move || {
        let mouse = mouse_pos_in_editor.read();

        if *workspace.drag_status().read() != DragStatus::Nodes {
            return;
        }

        let selected_nodes = graph_store.node_selection().read().all_nodes.read().clone();

        let mut best_match = None;

        for (_, node) in graph_store.nodes().iter() {
            let node_read = node.read();
            if selected_nodes.contains_key(&node_read.id()) {
                continue;
            }
            if let NodeType::Optical(t) = node_read.node_type() {
                if t != "group" {
                    continue;
                }

                if node_read.get_bounding_box().contains(*mouse) {
                    let z = node_read.z_index();

                    match best_match {
                        Some((_, best_z)) if z <= best_z => {}
                        _ => best_match = Some((node_read.id(), z)),
                    }
                }
            }
        }

        workspace_processor.send(GraphsWorkspaceAction::SetDropInGroup(best_match));
    });

    use_effect(move || {
        workspace_processor.send(GraphsWorkspaceAction::CenterGraph {
            graph_id,
            save_changes: false,
            // Programmatic mount-time centering (new project / file load / first tab open) is not
            // a user gesture - it must not create an undo step or enable the Undo button on a
            // fresh document.
            record_undo: false,
        });
    });

    let bread_crumbs = graph_state.graph_info().read().hierarchy.clone();

    rsx! {
        div { class: "graph-view-container",

            BreadCrumbs {
                bread_crumbs,
                bread_crumb_click_event: EventHandler::new(move |(group_id, group_name)| {
                    workspace_processor
                        .send(GraphsWorkspaceAction::OpenGroupTab {
                            group_id,
                            group_name,
                        });
                }),
            }
            div {
                class: "graph-editor",
                id: format!("editor_{}", graph_id.as_simple()),
                draggable: false,

                onwheel: onwheel_handler,
                onmousedown: onmousedown_handler,
                onmouseup: use_drag_end(workspace, Some(nodes_in_selection())),
                onmousemove: onmousemove_handler,
                div {
                    draggable: false,
                    style: format!(
                        "transform-origin: 0 0; transform: translate({}px, {}px) scale({});",
                        shift().x,
                        shift().y,
                        zoom(),
                    ),
                    for (_ , node) in graph_store.nodes().iter() {
                        {
                            rsx! {
                                Node {
                                    node,
                                    ctrl_pressed,
                                    shift_pressed,
                                    mouse_pos_in_editor,
                                    nodes_in_selection,
                                }
                            }
                        }
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
                                SelectionBoxComponent {}
                            }
                        }
                    }
                }
            }
        }
    }
}
