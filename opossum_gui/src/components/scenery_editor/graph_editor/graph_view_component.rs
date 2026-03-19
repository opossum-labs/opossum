#![allow(clippy::derive_partial_eq_without_eq)]
use crate::components::scenery_editor::{
    SelectionBoxComponent,
    edges::edges_component::{EdgeCreationComponent, EdgesComponent},
    graph_editor::{
        graph_workspace::{GraphState, GraphsWorkspaceAction},
        hooks::{use_drag, use_on_mouse_down, use_zoom},
    },
    nodes::Nodes,
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use std::{path::PathBuf, time::Instant};
use uuid::Uuid;

#[component]
pub fn GraphViewEditor(
    onmouseup_handler: EventHandler<Event<MouseData>>,
    model_modified_sig: ReadSignal<bool>,
    model_modified_handler: EventHandler<bool>,
    model_file_path_sig: ReadSignal<Option<PathBuf>>,
    model_file_path_handler: EventHandler<Option<PathBuf>>,
    current_mouse_pos: Signal<Point2D<f64>>,
    graph_state: Signal<GraphState>,
    ctrl_pressed: Signal<bool>,
    shift_pressed: Signal<bool>,
) -> Element {
    let last_auxiliary_click = use_signal(|| Option::<Instant>::None);
    let workspace_processor = use_coroutine_handle::<GraphsWorkspaceAction>();
    let editor_state = graph_state.read().editor_state;
    let graph_store = graph_state.read().graph_store;
    let graph_id = graph_state.read().graph_info.id;

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
                    Nodes {
                        graph_store,
                        graph_id,
                        ctrl_pressed,
                        shift_pressed,
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

#[component]
pub fn BreadCrumbs(
    bread_crumbs: Vec<(Uuid, String)>,
    bread_crumb_click_event: EventHandler<(Uuid, String)>,
) -> Element {
    rsx! {
        div { class: "graph-breadcrumbs",
            for (i , (id , name)) in bread_crumbs.iter().enumerate() {
                {
                    let name = name.clone();
                    let id = *id;
                    rsx! {
                        span {
                            class: "breadcrumb",
                            onclick: move |_| bread_crumb_click_event.call((id, name.clone())),
                            "{name}"
                        }

                        if i < bread_crumbs.len() - 1 {
                            span { class: "breadcrumb-sep", " › " }
                        }
                    }
                }
            }
        }
    }
}
