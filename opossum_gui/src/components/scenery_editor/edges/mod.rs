use crate::components::scenery_editor::constants::{EDGE_CORNER_RADIUS, EDGE_PORT_STUB};
use dioxus::html::geometry::euclid::default::{Point2D, Rect};
pub mod edge_component;
pub mod edge_element;
pub mod edges_component;

/// The SVG path of a connection from an output port to an input port.
///
/// The connection leaves the output port and enters the input port horizontally, each with a
/// straight piece of [`EDGE_PORT_STUB`]. If the input port lies to the right of these pieces, an
/// S-curve joins them whose control points lie out by half the horizontal distance, or by a quarter
/// of the vertical one if that is more: long connections bend gently, steep ones stay round. If it
/// lies to the left, the connection turns down (or up) to halfway between the ports, runs back
/// there and turns into the input port, with corners rounded by up to [`EDGE_CORNER_RADIUS`]. Either
/// way it passes the midpoint between the two ports.
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
    let between = if to >= from {
        let offset = (to - from).max((end.y - start.y).abs() / 2.0) / 2.0;
        format!(
            "C{},{} {},{} {to},{}",
            from + offset,
            start.y,
            to - offset,
            end.y,
            end.y,
        )
    } else {
        // The quarter keeps both vertical runs at least two corners long.
        let turn = ((end.y - start.y) / 4.0).clamp(-EDGE_CORNER_RADIUS, EDGE_CORNER_RADIUS);
        let (right, left) = (from + turn.abs(), to - turn.abs());
        let (start_y, end_y, middle) = (start.y, end.y, f64::midpoint(start.y, end.y));
        format!(
            "Q{right},{start_y} {right},{} L{right},{} Q{right},{middle} {from},{middle} \
             L{to},{middle} Q{left},{middle} {left},{} L{left},{} Q{left},{end_y} {to},{end_y}",
            start_y + turn,
            middle - turn,
            middle + turn,
            end_y - turn,
        )
    };
    format!(
        "M{},{} L{from},{} {between} L{},{}",
        start.x, start.y, start.y, end.x, end.y,
    )
}

/// The box that the path [`edge_path`] draws between two ports covers.
///
/// It spans both ports, widened on either side by a straight piece and the largest corner: as far
/// as a connection reaches that runs back to an input port on its left. An S-curve to the right
/// stays within it unless it is very steep, and then it bulges only into the span of the two nodes
/// it joins.
///
/// # Arguments
///
/// * `output_port` - where the connection starts.
/// * `input_port` - where it ends.
///
/// # Returns
///
/// The box; it has no height if both ports are at one height.
pub fn edge_bounds(output_port: Point2D<f64>, input_port: Point2D<f64>) -> Rect<f64> {
    Rect::from_points([output_port, input_port]).inflate(EDGE_PORT_STUB + EDGE_CORNER_RADIUS, 0.0)
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

    /// Stubs end at x = 615 and 85; a quarter of the 200 down exceeds the largest corner, so the
    /// corners have radius 20 and the run back lies at y = 200, halfway.
    #[test]
    fn a_backward_edge_turns_down_runs_back_halfway_and_turns_into_its_port() {
        let d = edge_path(Point2D::new(600.0, 100.0), Point2D::new(100.0, 300.0));
        assert_eq!(
            d,
            "M600,100 L615,100 Q635,100 635,120 L635,180 Q635,200 615,200 \
             L85,200 Q65,200 65,220 L65,280 Q65,300 85,300 L100,300"
        );
    }

    /// The same edge with its input port 200 higher instead of lower turns up instead of down.
    #[test]
    fn a_backward_edge_to_a_higher_port_turns_up() {
        let d = edge_path(Point2D::new(600.0, 300.0), Point2D::new(100.0, 100.0));
        assert_eq!(
            d,
            "M600,300 L615,300 Q635,300 635,280 L635,220 Q635,200 615,200 \
             L85,200 Q65,200 65,180 L65,120 Q65,100 85,100 L100,100"
        );
    }

    /// Only 40 down, so the corners shrink to a quarter of that (10); the edge stays within the stubs
    /// plus one corner, and its run back passes the midpoint (250, 20) between the ports.
    #[test]
    fn a_backward_edge_passes_the_midpoint_and_stays_within_its_corners() {
        let d = edge_path(Point2D::new(400.0, 0.0), Point2D::new(100.0, 40.0));
        let points = points(&d);

        assert!(points.iter().all(|p| (75.0..=425.0).contains(&p.x)), "{d}");
        let runs_through_midpoint = points.windows(2).any(|pair| {
            let at_middle = pair.iter().all(|p| (p.y - 20.0).abs() < 1e-9);
            at_middle && pair[0].x.min(pair[1].x) <= 250.0 && 250.0 <= pair[0].x.max(pair[1].x)
        });
        assert!(runs_through_midpoint, "{d}");
    }

    #[test]
    fn edge_bounds_cover_every_point_of_the_path() {
        let ports = [
            ((0.0, 0.0), (300.0, 200.0)),
            ((0.0, 0.0), (130.0, 400.0)),
            ((600.0, 100.0), (100.0, 300.0)),
            ((600.0, 300.0), (100.0, 100.0)),
            ((400.0, 0.0), (100.0, 40.0)),
        ];
        for ((start_x, start_y), (end_x, end_y)) in ports {
            let (start, end) = (Point2D::new(start_x, start_y), Point2D::new(end_x, end_y));
            let bounds = edge_bounds(start, end);
            let d = edge_path(start, end);
            for p in points(&d) {
                assert!((bounds.min_x()..=bounds.max_x()).contains(&p.x), "{d}");
                assert!((bounds.min_y()..=bounds.max_y()).contains(&p.y), "{d}");
            }
        }
    }

    /// A backward edge with full corners reaches the bounds on both sides.
    #[test]
    fn edge_bounds_are_tight_on_a_backward_edge() {
        let (start, end) = (Point2D::new(600.0, 100.0), Point2D::new(100.0, 300.0));
        let bounds = edge_bounds(start, end);
        let xs: Vec<f64> = points(&edge_path(start, end)).iter().map(|p| p.x).collect();

        assert!((xs.iter().copied().fold(f64::INFINITY, f64::min) - bounds.min_x()).abs() < 1e-9);
        assert!(
            (xs.iter().copied().fold(f64::NEG_INFINITY, f64::max) - bounds.max_x()).abs() < 1e-9
        );
    }
}
