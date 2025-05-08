use super::edge::Edge;
use crate::components::scenery_editor::edges::define_bezier_path;
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};

#[component]
pub fn EdgeComponent(edge: Edge) -> Element {
    // let mut distance_val = use_signal(|| format!("{}", edge.distance()));

    let start_position = edge.start_position();
    let end_position = edge.end_position();

    let new_path = define_bezier_path(start_position, end_position, 50.0);
    let distance_field_position= Point2D::new((start_position.x+end_position.x)/2.0, (start_position.y+end_position.y)/2.0);
    rsx! {
        path {
            d: new_path,
            // oncontextmenu: use_edge_context_menu(edge.conn_info),
            stroke: "black",
            fill: "transparent",
            stroke_width: format!("{}", 2.),
        }
        foreignObject {
            class: "distance-field",
            x: distance_field_position.x,
            y: distance_field_position.y,
            style: "background-color: white;",
            input {
                r#type: "number",
                value: 0.0,
                // oninput: move |e| distance_val.set(e.value()),
            }
        }
    }
}
// #[must_use]
// pub fn use_edge_context_menu(conn_info: ConnectInfo) -> Callback<Event<MouseData>> {
//     use_callback(move |evt: Event<MouseData>| {
//         evt.prevent_default();
//         let mut cx_menu = CONTEXT_MENU.write();
//         *cx_menu = CxMenu::new(
//             evt.page_coordinates().x,
//             evt.page_coordinates().y,
//             vec![(
//                 "Delete connection".to_owned(),
//                 use_delete_edge(conn_info.clone()),
//             )],
//         );
//     })
// }
// #[must_use]
// pub fn use_delete_edge(conn_info: ConnectInfo) -> Callback<Event<MouseData>> {
//     use_callback(move |_: Event<MouseData>| {
//         let conn_info = conn_info.clone();
//         spawn(async move {
//             match api::delete_connection(&HTTP_API_CLIENT(), conn_info).await {
//                 Ok(conn_info) => {
//                     EDGES.write().remove_edge(&conn_info);
//                     OPOSSUM_UI_LOGS
//                         .write()
//                         .add_log("Removed edge successfully!");
//                 }
//                 Err(err_str) => OPOSSUM_UI_LOGS.write().add_log(&err_str),
//             }
//         });
//     })
// }
