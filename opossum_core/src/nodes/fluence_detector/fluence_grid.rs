//! A regular grid of points that triangles of a fluence map are drawn onto.
use std::{cmp::Ordering, ops::Range};

use nalgebra::{DMatrix, Point2, Vector2};
use uom::si::{f64::Length, length::meter};

use crate::utils::{math_utils::signed_area, to_f64, try_f64_to_usize};

/// A regular grid of points that tubes are drawn onto, the values of overlapping tubes adding up.
pub struct FluenceGrid {
    x: Axis,
    y: Axis,
    /// The values, with a row per point along y and a column per point along x.
    values: DMatrix<f64>,
}
/// The evenly spaced points of a grid along one axis, in meters.
struct Axis {
    start: f64,
    step: f64,
    count: usize,
}
impl Axis {
    /// Creates a new [`Axis`] of `count` points from the start to the end of `range`, both
    /// included.
    fn new(range: &Range<Length>, count: usize) -> Self {
        let (start, end) = (range.start.get::<meter>(), range.end.get::<meter>());
        let step = if count > 1 {
            (end - start) / to_f64(count - 1)
        } else {
            0.0
        };
        Self { start, step, count }
    }
    /// The coordinate of the point with the given index.
    fn at(&self, index: usize) -> f64 {
        self.step.mul_add(to_f64(index), self.start)
    }
    /// The indices of the points from `from` to `to`, both included.
    fn within(&self, from: f64, to: f64) -> Range<usize> {
        // A point on the border of the range may be off by rounding; which side of an edge it
        // lies on is decided when the triangle is drawn.
        const ROUNDING: f64 = 1e-9;
        if self.count == 0 {
            return 0..0;
        }
        if self.step <= 0.0 {
            return if (from..=to).contains(&self.start) {
                0..1
            } else {
                0..0
            };
        }
        let first = ((from - self.start) / self.step - ROUNDING).ceil().max(0.0);
        let last = ((to - self.start) / self.step + ROUNDING)
            .floor()
            .min(to_f64(self.count - 1));
        match (try_f64_to_usize(first), try_f64_to_usize(last)) {
            (Some(first), Some(last)) if first <= last => first..last + 1,
            _ => 0..0,
        }
    }
}
impl FluenceGrid {
    /// Creates a new [`FluenceGrid`] of zeros.
    ///
    /// # Arguments
    ///
    /// * `x` - the range of the grid along x, both ends included.
    /// * `y` - the range of the grid along y, both ends included.
    /// * `nr_of_points` - the number of points along x and y.
    #[must_use]
    pub fn new(x: &Range<Length>, y: &Range<Length>, nr_of_points: (usize, usize)) -> Self {
        Self {
            x: Axis::new(x, nr_of_points.0),
            y: Axis::new(y, nr_of_points.1),
            values: DMatrix::zeros(nr_of_points.1, nr_of_points.0),
        }
    }
    /// Returns the values of this [`FluenceGrid`], with a row per point along y and a column per
    /// point along x.
    #[must_use]
    pub fn into_values(self) -> DMatrix<f64> {
        self.values
    }
    /// Adds a triangle to the grid, linearly between the values at its corners.
    ///
    /// # Arguments
    ///
    /// * `corners` - the corners of the triangle, in meters.
    /// * `values` - the values at the corners.
    pub(crate) fn add_triangle(&mut self, corners: [Point2<f64>; 3], values: [f64; 3]) {
        for_each_covered(&self.x, &self.y, corners, |cell, lambda| {
            if let Some(value) = self.values.get_mut(cell) {
                *value += lambda.iter().zip(values).map(|(l, v)| l * v).sum::<f64>();
            }
        });
    }
    /// Adds the deviation of a quadratic fluence from its linear interpolation over a triangle:
    /// `−½ Σ_{i<j} λ_i λ_j eᵀ H e` over the edges `e` of the triangle, with the barycentric
    /// coordinates `λ` of each point (see the handbook, "Fluence uncertainty").
    ///
    /// # Arguments
    ///
    /// * `corners` - the corners of the triangle, in meters.
    /// * `along_edges` - `eᵀ H e` along each edge, the edge opposite the corner of the same index.
    pub(crate) fn add_deviation(&mut self, corners: [Point2<f64>; 3], along_edges: [f64; 3]) {
        let [w0, w1, w2] = along_edges;
        for_each_covered(&self.x, &self.y, corners, |cell, [l0, l1, l2]| {
            if let Some(value) = self.values.get_mut(cell) {
                let curvature = (l1 * l2).mul_add(w0, (l2 * l0).mul_add(w1, l0 * l1 * w2));
                *value = curvature.mul_add(-0.5, *value);
            }
        });
    }
}
/// Calls `at` for each point of the grid along `x` and `y` that the given triangle covers, with
/// the row and the column of the point and its barycentric coordinates, one per corner in the
/// given order.
///
/// A point on an edge two triangles share is covered by one of them only: the one for which the
/// edge is a top or left one.
fn for_each_covered(
    x: &Axis,
    y: &Axis,
    mut corners: [Point2<f64>; 3],
    mut at: impl FnMut((usize, usize), [f64; 3]),
) {
    let twice_area = 2.0 * signed_area(corners);
    let turned = match twice_area.partial_cmp(&0.0) {
        Some(Ordering::Greater) => false,
        Some(Ordering::Less) => true,
        _ => return,
    };
    // the corners run counterclockwise from here on
    if turned {
        corners.swap(1, 2);
    }
    let per_area = twice_area.abs().recip();
    let [p0, p1, p2] = corners;
    // each edge lies opposite the corner of the same index
    let edges = [(p1, p2), (p2, p0), (p0, p1)];
    let top_left = edges.map(|(a, b)| is_top_left(b - a));
    let (lower, upper) = (p0.inf(&p1).inf(&p2), p0.sup(&p1).sup(&p2));
    for column in x.within(lower.x, upper.x) {
        for row in y.within(lower.y, upper.y) {
            let point = Point2::new(x.at(column), y.at(row));
            let weights = edges.map(|(a, b)| edge_weight(a, b, point));
            let covered = weights
                .iter()
                .zip(top_left)
                .all(|(w, top_left)| *w > 0.0 || (top_left && *w >= 0.0));
            if covered {
                let mut lambda = weights.map(|w| w * per_area);
                if turned {
                    lambda.swap(1, 2);
                }
                at((row, column), lambda);
            }
        }
    }
}
/// How far the given point lies left of the edge from `a` to `b`, times the length of the edge.
///
/// The edge is always measured from the same one of its two ends, so that the two triangles
/// sharing it see a point on exactly opposite sides, rounding included.
fn edge_weight(a: Point2<f64>, b: Point2<f64>, point: Point2<f64>) -> f64 {
    let a_first = a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)) != Ordering::Greater;
    if a_first {
        (b - a).perp(&(point - a))
    } else {
        -(a - b).perp(&(point - b))
    }
}
/// Whether an edge running along the given direction, of a triangle running counterclockwise, is
/// a top or a left edge.
fn is_top_left(direction: Vector2<f64>) -> bool {
    match direction.y.partial_cmp(&0.0) {
        Some(Ordering::Less) => true,
        Some(Ordering::Equal) => direction.x > 0.0,
        _ => false,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::meter;
    use approx::assert_relative_eq;

    /// A grid of 1 m spacing from -2 m to 2 m in x and y.
    fn unit_grid() -> FluenceGrid {
        FluenceGrid::new(
            &(meter!(-2.0)..meter!(2.0)),
            &(meter!(-2.0)..meter!(2.0)),
            (5, 5),
        )
    }
    /// The two triangles of a square cover the point on their shared diagonal once, and of the
    /// points on the border of the square those on its bottom and left edges: the others belong
    /// to the squares beyond.
    #[test]
    fn two_triangles_of_a_square_cover_each_point_once() {
        let mut grid = unit_grid();
        let (bottom_left, bottom_right, top_right, top_left) = (
            Point2::new(-1.0, -1.0),
            Point2::new(1.0, -1.0),
            Point2::new(1.0, 1.0),
            Point2::new(-1.0, 1.0),
        );
        grid.add_triangle([bottom_left, bottom_right, top_right], [1.0; 3]);
        grid.add_triangle([bottom_left, top_right, top_left], [1.0; 3]);
        let values = grid.values;
        assert_relative_eq!(values.iter().sum::<f64>(), 4.0);
        assert!(values.iter().all(|v| *v <= 1.0), "{values}");
        assert_relative_eq!(values[(2, 2)], 1.0);
    }
    /// A plane tiled by triangles covers every point exactly once.
    #[test]
    fn a_tiling_covers_every_point_once() {
        let mut grid = unit_grid();
        for x in -3..3 {
            for y in -3..3 {
                let (x, y) = (f64::from(x), f64::from(y));
                let (bottom_left, bottom_right, top_right, top_left) = (
                    Point2::new(x, y),
                    Point2::new(x + 1.0, y),
                    Point2::new(x + 1.0, y + 1.0),
                    Point2::new(x, y + 1.0),
                );
                grid.add_triangle([bottom_left, bottom_right, top_right], [1.0; 3]);
                grid.add_triangle([top_right, top_left, bottom_left], [1.0; 3]);
            }
        }
        assert!(grid.values.iter().all(|v| (*v - 1.0).abs() < 1e-12));
    }
    /// Inside a triangle the value runs linearly between its corners, whichever way they run.
    #[test]
    fn a_triangle_interpolates_linearly() {
        for clockwise in [false, true] {
            let mut grid = unit_grid();
            let mut corners = [
                Point2::new(-3.0, -3.0),
                Point2::new(5.0, -3.0),
                Point2::new(-3.0, 5.0),
            ];
            let mut values = [-1.5, 6.5, 14.5];
            if clockwise {
                corners.swap(1, 2);
                values.swap(1, 2);
            }
            grid.add_triangle(corners, values);
            let values = grid.values;
            // the value is x + 2 y + 7.5: 7.5 at the origin, 8.5 one step along x
            assert_relative_eq!(values[(2, 2)], 7.5, epsilon = 1e-12);
            assert_relative_eq!(values[(2, 3)], 8.5, epsilon = 1e-12);
            assert_relative_eq!(values[(3, 2)], 9.5, epsilon = 1e-12);
        }
    }
}
