use dioxus::prelude::*;

use crate::components::scenery_editor::{EditorState, GraphsWorkspaceState};

#[component]
pub fn SelectionBoxComponent() -> Element {
    let editor_status = use_context::<Signal<EditorState>>();
    let workspace = use_context::<Signal<GraphsWorkspaceState>>();
    let zoom = *editor_status.read().zoom.read();
    workspace.read().selection_box.read().clone().map_or_else(
        || rsx! {},
        |select_box| {
            rsx! {
                rect {
                    x: select_box.origin.x,
                    y: select_box.origin.y,
                    width: select_box.width(),
                    height: select_box.height(),
                    stroke: "rgba(74, 107, 255, 0.53)",
                    fill: "rgba(103, 131, 255, 0.17)",
                    stroke_width: format!("{}", 1. / zoom),
                }
            }
        },
    )
}
