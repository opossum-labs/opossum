use dioxus::prelude::*;

use crate::components::scenery_editor::{GraphState, GraphsWorkspaceState, graph_workspace::{GraphStateStoreExt, EditorStateStoreExt}};

#[component]
pub fn SelectionBoxComponent() -> Element {
let graph_state = use_context::<ReadStore<GraphState>>();
    let editor_status = graph_state.editor_state();    let workspace = use_context::<ReadSignal<GraphsWorkspaceState>>();
    let zoom = *editor_status.zoom().read();
    let select_box_opt = use_memo(move || *workspace.read().selection_box.read());
    rsx! {
        if let Some(select_box) = select_box_opt(){
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
    }
    // select_box_opt().map_or_else(
    //     || rsx! {},
    //     |select_box| {
    //         rsx! {
    //             rect {
    //                 x: select_box.origin.x,
    //                 y: select_box.origin.y,
    //                 width: select_box.width(),
    //                 height: select_box.height(),
    //                 stroke: "rgba(74, 107, 255, 0.53)",
    //                 fill: "rgba(103, 131, 255, 0.17)",
    //                 stroke_width: format!("{}", 1. / zoom),
    //             }
    //         }
    //     },
    // )
}
