use dioxus::html::geometry::{Pixels, euclid::Point2D};
pub mod edge_component;
pub mod edges_component;

pub fn define_bezier_path(
    start: Point2D<f64, Pixels>,
    end: Point2D<f64, Pixels>,
    bezier_offset: f64,
) -> String {
    format!(
        "M{},{} C{},{} {},{} {},{}",
        start.x,
        start.y,
        start.x + bezier_offset,
        start.y,
        end.x - bezier_offset,
        end.y,
        end.x,
        end.y,
    )
}
