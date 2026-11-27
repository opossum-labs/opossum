use crate::components::scenery_editor::constants::EDGE_PORT_STUB;
use dioxus::html::geometry::euclid::default::Point2D;
pub mod edge_component;
pub mod edge_element;
pub mod edges_component;

/// The SVG path of a connection from an output port to an input port.
///
/// The connection leaves the output port and enters the input port horizontally, each with a
/// straight piece of [`EDGE_PORT_STUB`]. Between them runs an S-curve whose control points lie out
/// by half the horizontal distance, or by a quarter of the vertical one if that is more: long
/// connections bend gently, steep ones stay round.
///
/// # Arguments
///
/// * `output_port` - where the connection starts.
/// * `input_port` - where it ends.
///
/// # Returns
///
/// The path, as the `d` attribute of an SVG `path`.
pub fn edge_path(output_port: Point2D<f64>, input_port: Point2D<f64>) -> String {
    let (start, end) = (output_port, input_port);
    let (from, to) = (start.x + EDGE_PORT_STUB, end.x - EDGE_PORT_STUB);
    let offset = (to - from).max((end.y - start.y).abs() / 2.0) / 2.0;
    format!(
        "M{},{} L{},{} C{},{} {},{} {},{} L{},{}",
        start.x,
        start.y,
        from,
        start.y,
        from + offset,
        start.y,
        to - offset,
        end.y,
        to,
        end.y,
        end.x,
        end.y,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let d = edge_path(start, end);
        assert_leaves_and_enters_horizontally(&d, start, end);
    }

    #[test]
    fn a_forward_edge_to_another_height_leaves_and_enters_its_ports_horizontally() {
        let (start, end) = (Point2D::new(0.0, 0.0), Point2D::new(300.0, 200.0));
        let d = edge_path(start, end);
        assert_leaves_and_enters_horizontally(&d, start, end);
    }

    #[test]
    fn a_backward_edge_leaves_and_enters_its_ports_horizontally() {
        let (start, end) = (Point2D::new(500.0, 0.0), Point2D::new(0.0, 200.0));
        let d = edge_path(start, end);
        assert_leaves_and_enters_horizontally(&d, start, end);
    }

    /// Stubs end at x = 15 and 285; the control points lie half of those 270 out.
    #[test]
    fn a_forward_edge_at_one_height_is_a_straight_line_through_both_stubs() {
        let d = edge_path(Point2D::new(0.0, 0.0), Point2D::new(300.0, 0.0));
        assert_eq!(d, "M0,0 L15,0 C150,0 150,0 285,0 L300,0");
    }

    /// The horizontal distance between the stubs (270) outweighs half the vertical one (100), so the
    /// control points lie 135 out.
    #[test]
    fn a_long_edge_bends_by_half_its_horizontal_distance() {
        let d = edge_path(Point2D::new(0.0, 0.0), Point2D::new(300.0, 200.0));
        assert_eq!(d, "M0,0 L15,0 C150,0 150,200 285,200 L300,200");
    }

    /// Half the vertical distance (200) outweighs the horizontal one between the stubs (100), so the
    /// control points lie a quarter of the vertical distance (100) out.
    #[test]
    fn a_steep_edge_bends_by_a_quarter_of_its_vertical_distance() {
        let d = edge_path(Point2D::new(0.0, 0.0), Point2D::new(130.0, 400.0));
        assert_eq!(d, "M0,0 L15,0 C115,0 15,400 115,400 L130,400");
    }
}
