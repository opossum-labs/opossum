#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::scenery_editor::{
    edges::edges_component::{EdgeCreationComponent, EdgesComponent},
    graph_editor::{
        graph_workspace::{GraphState, GraphsWorkspaceAction, GraphsWorkspaceState},
        hooks::{use_drag, use_on_mouse_down, use_zoom},
    },
    nodes::Nodes,
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use std::{path::PathBuf, time::Instant};

#[component]
pub fn GraphViewEditor(
    onmouseup_handler: EventHandler<Event<MouseData>>,
    model_modified_sig: ReadSignal<bool>,
    model_modified_handler: EventHandler<bool>,
    model_file_path_sig: ReadSignal<Option<PathBuf>>,
    model_file_path_handler: EventHandler<Option<PathBuf>>,
    current_mouse_pos: Signal<Point2D<f64>>,
    graph_state: Signal<GraphState>,
) -> Element {
    let last_auxiliary_click = use_signal(|| Option::<Instant>::None);
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let workspace = use_context::<Signal<GraphsWorkspaceState>>();
    let editor_state = graph_state.read().editor_state;
    let graph_store = graph_state.read().graph_store;
    let graph_id = graph_state.read().id;

    use_context_provider(|| graph_state);
    use_context_provider(|| editor_state);
    use_context_provider(|| graph_store);
    let onwheel_handler = use_zoom();
    let onmousemove_handler = use_drag(current_mouse_pos);
    let onmousedown_handler = use_on_mouse_down(current_mouse_pos, last_auxiliary_click);

    let shift = use_memo(move || *editor_state.read().shift.read());
    let zoom = use_memo(move || *editor_state.read().zoom.read());

    use_effect(move || {
        workspace_processor.send(GraphsWorkspaceAction::CenterGraph {
            graph_id,
            save_changes: false,
        });
    });

    let breadcrumbs: Memo<Vec<(uuid::Uuid, String)>> = use_memo(move || {
        let workspace_read = workspace.read();
        workspace_read.build_breadcrumbs(graph_state.read().id)
    });

    rsx! {
        div { class: "graph-view-container",

            div { class: "graph-breadcrumbs",
                {
                    let path = breadcrumbs();
                    rsx! {
                        for (i , (_ , name)) in path.iter().enumerate() {
                            span { class: "breadcrumb", "{name}" }

                            if i < path.len() - 1 {
                                span { class: "breadcrumb-sep", " / " }
                            }
                        }
                    }
                }
            }
            div {
                class: "graph-editor",
                id: format!("editor_{}", graph_id.as_simple()),
                draggable: false,

                onwheel: onwheel_handler,
                onmousedown: onmousedown_handler,
                onmouseup: move |e| onmouseup_handler.call(e),
                onmousemove: onmousemove_handler,
                div {
                    draggable: false,
                    style: format!(
                        "transform-origin: 0 0; transform: translate({}px, {}px) scale({});",
                        shift().x,
                        shift().y,
                        zoom(),
                    ),
                    Nodes { graph_store, graph_id }
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
}
