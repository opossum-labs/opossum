use crate::components::{
    node_editor::inputs::input_components::UnitInput,
    scenery_editor::{
        constants::{EDGE_BEZIER_OFFSET, EDGE_DISTANCE_FIELD_HEIGHT, EDGE_DISTANCE_FIELD_WIDTH},
        edges::define_bezier_path,
        graph_store::{GraphStore, GraphStoreAction},
    },
};
use dioxus::{html::geometry::euclid::default::Point2D, prelude::*};
use opossum_core::{prelude::*, types::api_types::ConnectInfo};

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
                .peek()
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
            pointer_events: "auto",
            onkeydown: {
                let edge = edge.clone();
                move |event: Event<KeyboardData>| {
                    if event.data().key() == Key::Delete {
                        graph_processor.send(GraphStoreAction::DeleteEdge(edge.clone()));
                    }
                    event.stop_propagation();
                }
            },
            fill: "transparent",
        }
        foreignObject {
            pointer_events: "none",
            x: distance_field_position.x,
            y: distance_field_position.y,
            width: EDGE_DISTANCE_FIELD_WIDTH,
            height: EDGE_DISTANCE_FIELD_HEIGHT,
            div {
                pointer_events: "auto",
                class: "input-with-unit",
                style: "display: flex; align-items: center; background: #fff; border: 1px solid #ccc; border-radius: 4px; padding: 0 8px; box-sizing: border-box;",
                UnitInput {
                    id: format!(
                        "distance-{}{}",
                        edge.src_uuid().as_simple(),
                        edge.target_uuid().as_simple(),
                    ),
                    label: String::new(),
                    value: edge.distance(),
                    base_unit: "m",
                    onchange: {
                        let edge = edge.clone();
                        move |new_distance: f64| {
                            let mut edge = edge.clone();

                            edge.set_distance(new_distance);
                            graph_processor.send(GraphStoreAction::UpdateEdge(edge));
                        }
                    },
                    flushable_input: false,
                    input_class: "edge_distance_input".to_string(),
                }
            }
        }
    }
}
