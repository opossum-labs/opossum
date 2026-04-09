#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::scenery_editor::{
    GraphsWorkspaceState, SelectionBoxComponent,
    edges::edges_component::{EdgeCreationComponent, EdgesComponent},
    graph_editor::{
        BreadCrumbs,
        hooks::{use_drag, use_drag_end, use_on_mouse_down, use_zoom},
    },
    graph_workspace::{GraphState, GraphsWorkspaceAction},
    nodes::Nodes,
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use std::{collections::HashSet, path::PathBuf, time::Instant};
use uuid::Uuid;

#[component]
pub fn GraphViewEditor(
    model_modified_sig: ReadSignal<bool>,
    model_modified_handler: EventHandler<bool>,
    model_file_path_sig: ReadSignal<Option<PathBuf>>,
    model_file_path_handler: EventHandler<Option<PathBuf>>,
    current_mouse_pos: Signal<Point2D<f64>>,
    graph_state: ReadSignal<GraphState>,
    ctrl_pressed: ReadSignal<bool>,
    shift_pressed: ReadSignal<bool>,
) -> Element {
    let last_auxiliary_click = use_signal(|| Option::<Instant>::None);
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let editor_state = graph_state.read().editor_state;
    let graph_store = graph_state.read().graph_store;
    let graph_id = graph_state.read().graph_info.id;
    let workspace = use_context::<ReadSignal<GraphsWorkspaceState>>();

    use_context_provider(|| graph_state);
    use_context_provider(|| ReadSignal::from(editor_state));
    use_context_provider(|| ReadSignal::from(graph_store));
    let onwheel_handler = use_zoom();
    let onmousemove_handler = use_drag(current_mouse_pos);
    let onmousedown_handler = use_on_mouse_down(
        current_mouse_pos,
        last_auxiliary_click,
        ctrl_pressed,
        graph_id,
    );

    let nodes_in_selection = use_memo(move || {
        let selection_box = *workspace.peek().selection_box.read();
        let nodes = graph_store.peek().nodes().peek().clone();

        if let Some(select_box) = selection_box {
            nodes
                .iter()
                .filter_map(|(id, node)| {
                    let rect = node.get_bounding_box(); // hast du schon 👍
                    if select_box.intersects(&rect) {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect::<HashSet<Uuid>>()
        } else {
            HashSet::<Uuid>::new()
        }
    });

    let shift = use_memo(move || *editor_state.read().shift.read());
    let zoom = use_memo(move || *editor_state.read().zoom.read());

    let mouse_pos_in_editor = use_memo(move || {
        let editor_origin = workspace.peek().editor_area.peek().origin;
        Point2D::new(
            (current_mouse_pos.read().x - editor_origin.x - shift.peek().x) / *zoom.peek(),
            (current_mouse_pos.read().y - editor_origin.y - shift.peek().y) / *zoom.peek(),
        )
    });

    use_effect(move || {
        workspace_processor.send(GraphsWorkspaceAction::CenterGraph {
            graph_id,
            save_changes: false,
        });
    });

    let bread_crumbs = graph_state.read().graph_info.hierarchy.clone();

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
                onmouseup: use_drag_end(workspace.into(), Some(nodes_in_selection())),
                onmousemove: onmousemove_handler,
                div {
                    draggable: false,
                    style: format!(
                        "transform-origin: 0 0; transform: translate({}px, {}px) scale({});",
                        shift().x,
                        shift().y,
                        zoom(),
                    ),
                    Nodes {
                        graph_store,
                        graph_id,
                        ctrl_pressed,
                        shift_pressed,
                        mouse_pos_in_editor,
                        nodes_in_selection
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
