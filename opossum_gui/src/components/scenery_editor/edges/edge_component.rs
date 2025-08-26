use crate::components::scenery_editor::{
    constants::{EDGE_BEZIER_OFFSET, EDGE_DISTANCE_FIELD_HEIGHT, EDGE_DISTANCE_FIELD_WIDTH},
    edges::define_bezier_path,
    graph_store::{GraphStore, GraphStoreAction},
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_backend::{PortType, nodes::ConnectInfo};

#[component]
pub fn EdgeComponent(edge: ConnectInfo) -> Element {
    let graph_store = use_context::<Signal<GraphStore>>();
    let graph_processor = use_coroutine_handle::<GraphStoreAction>();

    // Memoize the start and end positions. This will only re-read the node
    // positions and re-calculate when the `edge` prop itself changes.
    // Dioxus's signal system will ensure this only triggers a re-render if
    // the underlying node data that `abs_port_position` depends on has changed.
    let start_position = use_memo({
        let edge = edge.clone();
        move || {
            graph_store
                .read()
                .nodes()
                .read()
                .get(&edge.src_uuid())
                .map(|n| n.abs_port_position(&PortType::Output, edge.src_port()))
                .unwrap_or_default()
        }
    });

    let end_position = use_memo({
        let edge = edge.clone();
        move || {
            graph_store
                .read()
                .nodes()
                .read()
                .get(&edge.target_uuid())
                .map(|n| n.abs_port_position(&PortType::Input, edge.target_port()))
                .unwrap_or_default()
        }
    });

    let new_path = define_bezier_path(start_position(), end_position(), EDGE_BEZIER_OFFSET);
    let distance_field_position = Point2D::new(
        f64::midpoint(start_position().x, end_position().x) - EDGE_DISTANCE_FIELD_WIDTH / 2.0,
        f64::midpoint(start_position().y, end_position().y) - EDGE_DISTANCE_FIELD_HEIGHT / 2.0,
    );
    rsx! {
        path {
            d: new_path,
            tabindex: 0,
            onkeydown: {
                let edge = edge.clone();
                move |event: Event<KeyboardData>| {
                    if event.data().key() == Key::Delete {
                        graph_processor.send(GraphStoreAction::DeleteEdge(edge.clone()));
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
            width: EDGE_DISTANCE_FIELD_WIDTH,
            height: EDGE_DISTANCE_FIELD_HEIGHT,
            div {
                class: "input-with-unit",
                style: "display: flex; align-items: center; background: #fff; border: 1px solid #ccc; border-radius: 4px; padding: 0 8px; box-sizing: border-box;",
                input {
                    style: "text-align: right; flex:1; min-width: 0; height: 100%; font-size: 11pt; border: none; outline: none; padding: 0;",
                    r#type: "number",
                    r#step: "0.0001",
                    value: format!("{:.4}", edge.distance()),
                    onchange: {
                        move |event: Event<FormData>| {
                            if let Ok(new_distance) = event.data.parsed::<f64>() {
                                edge.set_distance(new_distance);
                                let edge = edge.clone();
                                graph_processor.send(GraphStoreAction::UpdateEdge(edge));
                            }
                        }
                    },
                    ondoubleclick: |event| {
                        event.stop_propagation();
                    },
                }
                span { style: "font-size: 9pt; margin-left: 3px; color: #555;", "m" }
            }
                // input {
        //     class: "form-control",
        //     style: "text-align: center; width: 100%; height: 100%; font-size: 11pt",
        //     r#type: "number",
        //     value: edge.distance(),
        //     onchange: {
        //         move |event: Event<FormData>| {
        //             if let Ok(new_distance) = event.data.parsed::<f64>() {
        //                 edge.set_distance(new_distance);
        //                 let edge = edge.clone();
        //                 graph_processor.send(GraphStoreAction::UpdateEdge(edge));
        //             }
        //         }
        //     },
        //     ondoubleclick: |event| {
        //         event.stop_propagation();
        //     },
        // }
        }
    }
}
