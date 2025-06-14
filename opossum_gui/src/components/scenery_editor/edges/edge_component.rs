use crate::components::scenery_editor::{edges::define_bezier_path, graph_store::GraphStore};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_backend::{nodes::ConnectInfo, PortType};

#[component]
pub fn EdgeComponent(edge: ConnectInfo) -> Element {
    let mut graph_store = use_context::<GraphStore>();
    let optic_nodes = graph_store.nodes();
    let mut start_position = use_signal(|| Point2D::new(0.0, 0.0));
    let mut end_position = use_signal(|| Point2D::new(0.0, 0.0));

    use_effect({
        let edge = edge.clone();
        move || {
            let optic_nodes = optic_nodes();
            let src_node = optic_nodes.get(&edge.src_uuid()).unwrap();
            let target_node = optic_nodes.get(&edge.target_uuid()).unwrap();
            start_position.set(src_node.abs_port_position(&PortType::Output, edge.src_port()));
            end_position.set(target_node.abs_port_position(&PortType::Input, edge.target_port()));
        }
    });

    let new_path = define_bezier_path(start_position(), end_position(), 50.0);

    let distance_field_height = 25.0;
    let distance_field_width = 60.0;

    let distance_field_position = Point2D::new(
        f64::midpoint(start_position().x, end_position().x) - distance_field_width / 2.0,
        f64::midpoint(start_position().y, end_position().y) - distance_field_height / 2.0,
    );
    rsx! {
        path {
            d: new_path,
            tabindex: 0,
            onkeydown: {
                let edge = edge.clone();
                move |event: Event<KeyboardData>| {
                    let edge = edge.clone();
                    if event.data().key() == Key::Delete {
                        spawn(async move { graph_store.delete_edge(edge).await });
                    }
                    event.stop_propagation();
                }
            },
            stroke: "black",
            fill: "transparent",
            stroke_width: format!("{}", 2.),
        }
        foreignObject {
            x: distance_field_position.x,
            y: distance_field_position.y,
            width: distance_field_width + 50.0,
            height: distance_field_height + 10.0,
            input {
                class: "form-control",
                style: format!(
                    "text-align: right; width: {}pt; height: {}pt",
                    distance_field_width,
                    distance_field_height,
                ),
                r#type: "number",
                value: edge.distance(),
                onchange: {
                    move |event: Event<FormData>| {
                        if let Ok(new_distance) = event.data.parsed::<f64>() {
                            edge.set_distance(new_distance);
                            let edge = edge.clone();
                            spawn(async move { graph_store.update_edge(&edge).await });
                        }
                    }
                },
                ondoubleclick: |event| {
                    event.stop_propagation();
                },
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
