use dioxus::html::geometry::euclid::default::Point2D;
pub mod edge_component;
pub mod edge_element;
pub mod edges_component;

pub fn define_bezier_path(start: Point2D<f64>, end: Point2D<f64>, bezier_offset: f64) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::scenery_editor::constants::EDGE_BEZIER_OFFSET;

    /// The points of an SVG path, in order, whatever commands lie between them.
    fn points(d: &str) -> Vec<Point2D<f64>> {
        let numbers: Vec<f64> = d
            .split(|c: char| c.is_ascii_alphabetic() || c == ',' || c.is_whitespace())
            .filter(|number| !number.is_empty())
            .map(|number| number.parse().unwrap())
            .collect();
        numbers
            .chunks(2)
            .map(|pair| Point2D::new(pair[0], pair[1]))
            .collect()
    }

    /// Asserts that `d` runs from `start` to `end`, leaves `start` horizontally to the right and
    /// enters `end` horizontally from the left.
    fn assert_leaves_and_enters_horizontally(d: &str, start: Point2D<f64>, end: Point2D<f64>) {
        let points = points(d);
        let [after_start, before_end] = [points[1], points[points.len() - 2]];
        assert!((points[0] - start).length() < 1e-9, "{d} starts elsewhere");
        assert!(
            (points[points.len() - 1] - end).length() < 1e-9,
            "{d} ends elsewhere"
        );
        assert!(
            (after_start.y - start.y).abs() < 1e-9 && after_start.x > start.x,
            "{d}"
        );
        assert!(
            (before_end.y - end.y).abs() < 1e-9 && before_end.x < end.x,
            "{d}"
        );
    }

    #[test]
    fn a_forward_edge_leaves_and_enters_its_ports_horizontally() {
        let (start, end) = (Point2D::new(0.0, 0.0), Point2D::new(300.0, 0.0));
        let d = define_bezier_path(start, end, EDGE_BEZIER_OFFSET);
        assert_leaves_and_enters_horizontally(&d, start, end);
    }

    #[test]
    fn a_forward_edge_to_another_height_leaves_and_enters_its_ports_horizontally() {
        let (start, end) = (Point2D::new(0.0, 0.0), Point2D::new(300.0, 200.0));
        let d = define_bezier_path(start, end, EDGE_BEZIER_OFFSET);
        assert_leaves_and_enters_horizontally(&d, start, end);
    }

    #[test]
    fn a_backward_edge_leaves_and_enters_its_ports_horizontally() {
        let (start, end) = (Point2D::new(500.0, 0.0), Point2D::new(0.0, 200.0));
        let d = define_bezier_path(start, end, EDGE_BEZIER_OFFSET);
        assert_leaves_and_enters_horizontally(&d, start, end);
    }
}
