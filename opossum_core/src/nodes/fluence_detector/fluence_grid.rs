//! The regular grid of points a fluence map shows its fluences at.
use std::{cmp::Ordering, ops::Range};

use nalgebra::{DMatrix, DVector, Point2, Vector2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uom::si::{
    f64::{Area, Length},
    length::meter,
};

use super::Fluence;
use crate::{
    meter, square_meter,
    types::SiScalarSchema,
    utils::{math_utils::signed_area, to_f64, try_f64_to_usize},
};

/// A regular grid of points with a fluence at each: the points lie evenly from one end of each
/// range to the other, both included. Triangles drawn onto it add up where they overlap.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct FluenceGrid {
    /// the range of the points along x
    #[schemars(with = "Range<SiScalarSchema>")]
    x: Range<Length>,
    /// the range of the points along y
    #[schemars(with = "Range<SiScalarSchema>")]
    y: Range<Length>,
    /// the fluences, a row per point along y and a column per point along x: column by column,
    /// then the number of rows and of columns
    #[schemars(with = "(Vec<SiScalarSchema>, usize, usize)")]
    values: DMatrix<Fluence>,
}
/// The evenly spaced points of a grid along one axis, in meters.
struct Axis {
    start: f64,
    end: f64,
    count: usize,
}
impl Axis {
    /// Creates a new [`Axis`] of `count` points from the start to the end of `range`, both
    /// included.
    fn new(range: &Range<Length>, count: usize) -> Self {
        Self {
            start: range.start.get::<meter>(),
            end: range.end.get::<meter>(),
            count,
        }
    }
    /// The spacing of the points, zero for fewer than two.
    fn step(&self) -> f64 {
        if self.count > 1 {
            (self.end - self.start) / to_f64(self.count - 1)
        } else {
            0.0
        }
    }
    /// The coordinate of the point with the given index.
    fn at(&self, index: usize) -> f64 {
        self.step().mul_add(to_f64(index), self.start)
    }
    /// The length the point with the given index stands for: the spacing, half of it at either
    /// end (the trapezoid rule), and the whole range for a single point.
    fn cell(&self, index: usize) -> f64 {
        if self.count < 2 {
            self.end - self.start
        } else if index == 0 || index + 1 == self.count {
            self.step() / 2.0
        } else {
            self.step()
        }
    }
    /// Where `at` lies among the points, counted in points from the first; `None` beyond them.
    fn position(&self, at: f64) -> Option<f64> {
        let last = to_f64(self.count.saturating_sub(1));
        let position = (at - self.start) / (self.end - self.start) * last;
        (0.0..=last).contains(&position).then_some(position)
    }
    /// The indices of the points from `from` to `to`, both included.
    fn within(&self, from: f64, to: f64) -> Range<usize> {
        // A point on the border of the range may be off by rounding; which side of an edge it
        // lies on is decided when the triangle is drawn.
        const ROUNDING: f64 = 1e-9;
        let step = self.step();
        if self.count == 0 {
            return 0..0;
        }
        if step <= 0.0 {
            return if (from..=to).contains(&self.start) {
                0..1
            } else {
                0..0
            };
        }
        let first = ((from - self.start) / step - ROUNDING).ceil().max(0.0);
        let last = ((to - self.start) / step + ROUNDING)
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
        Self::with_values(
            x.clone(),
            y.clone(),
            DMatrix::zeros(nr_of_points.1, nr_of_points.0),
        )
    }
    /// Creates a new [`FluenceGrid`] with the given fluences.
    ///
    /// # Arguments
    ///
    /// * `x` - the range of the grid along x, both ends included.
    /// * `y` - the range of the grid along y, both ends included.
    /// * `values` - the fluences, a row per point along y and a column per point along x.
    #[must_use]
    pub const fn with_values(x: Range<Length>, y: Range<Length>, values: DMatrix<Fluence>) -> Self {
        Self { x, y, values }
    }
    /// Returns the range of the points of this [`FluenceGrid`] along x.
    #[must_use]
    pub const fn x_range(&self) -> &Range<Length> {
        &self.x
    }
    /// Returns the range of the points of this [`FluenceGrid`] along y.
    #[must_use]
    pub const fn y_range(&self) -> &Range<Length> {
        &self.y
    }
    /// Returns the fluences of this [`FluenceGrid`], a row per point along y and a column per
    /// point along x.
    #[must_use]
    pub const fn values(&self) -> &DMatrix<Fluence> {
        &self.values
    }
    /// Returns the fluences of this [`FluenceGrid`], a row per point along y and a column per
    /// point along x.
    #[must_use]
    pub fn into_values(self) -> DMatrix<Fluence> {
        self.values
    }
    /// Returns the coordinates of the points of this [`FluenceGrid`] along x and along y.
    #[must_use]
    pub fn points(&self) -> (DVector<Length>, DVector<Length>) {
        let points = |axis: Axis| DVector::from_fn(axis.count, |index, _| meter!(axis.at(index)));
        let (x, y) = self.axes();
        (points(x), points(y))
    }
    /// Returns where the given point lies among the points of this [`FluenceGrid`], counted in
    /// points from the first: its column and its row as fractions, `None` beyond the grid.
    #[must_use]
    pub fn position_of(&self, at: Point2<Length>) -> Option<(f64, f64)> {
        let (x, y) = self.axes();
        Some((
            x.position(at.x.get::<meter>())?,
            y.position(at.y.get::<meter>())?,
        ))
    }
    /// Returns the area each point of this [`FluenceGrid`] stands for, a row per point along y.
    ///
    /// Each point stands for a cell as large as the spacing of the points, cut at the border of
    /// the grid: a point on an edge for half of one, a point in a corner for a quarter (the
    /// trapezoid rule). A single point along an axis stands for its whole range.
    #[must_use]
    pub fn point_areas(&self) -> DMatrix<Area> {
        let (x, y) = self.axes();
        DMatrix::from_fn(y.count, x.count, |row, column| {
            square_meter!(x.cell(column) * y.cell(row))
        })
    }
    /// The points of this [`FluenceGrid`] along x and along y.
    fn axes(&self) -> (Axis, Axis) {
        (
            Axis::new(&self.x, self.values.ncols()),
            Axis::new(&self.y, self.values.nrows()),
        )
    }
    /// Adds a triangle to the grid, linearly between the fluences at its corners.
    ///
    /// # Arguments
    ///
    /// * `corners` - the corners of the triangle, in meters.
    /// * `values` - the fluences at the corners.
    pub(crate) fn add_triangle(&mut self, corners: [Point2<f64>; 3], values: [Fluence; 3]) {
        let (x, y) = self.axes();
        for_each_covered(&x, &y, corners, |cell, lambda| {
            if let Some(value) = self.values.get_mut(cell) {
                *value += lambda
                    .iter()
                    .zip(values)
                    .map(|(l, v)| v * *l)
                    .sum::<Fluence>();
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
    pub(crate) fn add_deviation(&mut self, corners: [Point2<f64>; 3], along_edges: [Fluence; 3]) {
        let [w0, w1, w2] = along_edges;
        let (x, y) = self.axes();
        for_each_covered(&x, &y, corners, |cell, [l0, l1, l2]| {
            if let Some(value) = self.values.get_mut(cell) {
                *value -= (w0 * (l1 * l2) + w1 * (l2 * l0) + w2 * (l0 * l1)) * 0.5;
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
    use crate::{J_per_m2, meter};
    use approx::assert_relative_eq;

    /// A grid of 1 m spacing from -2 m to 2 m in x and y.
    fn unit_grid() -> FluenceGrid {
        FluenceGrid::new(
            &(meter!(-2.0)..meter!(2.0)),
            &(meter!(-2.0)..meter!(2.0)),
            (5, 5),
        )
    }
    /// The fluences of a grid, in J/m².
    fn in_j_per_m2(grid: FluenceGrid) -> DMatrix<f64> {
        grid.into_values().map(|fluence| fluence.value)
    }
    /// The points of a grid lie evenly from one end of each range to the other, both included.
    #[test]
    fn the_points_of_a_grid_span_its_ranges() {
        let (x, y) = unit_grid().points();
        let expected = [-2.0, -1.0, 0.0, 1.0, 2.0].map(|x| meter!(x));
        assert_eq!(x.as_slice(), expected);
        assert_eq!(y.as_slice(), expected);
    }
    /// A point lies among the points of a grid as far from the first as it lies from the start of
    /// each range, counted in points; beyond the grid it lies nowhere.
    #[test]
    fn a_point_lies_among_the_points_of_a_grid() {
        let grid = unit_grid();
        assert_eq!(
            grid.position_of(Point2::new(meter!(0.5), meter!(-2.0))),
            Some((2.5, 0.0))
        );
        assert_eq!(
            grid.position_of(Point2::new(meter!(2.0), meter!(2.0))),
            Some((4.0, 4.0))
        );
        assert!(
            grid.position_of(Point2::new(meter!(2.5), meter!(0.0)))
                .is_none()
        );
        assert!(
            grid.position_of(Point2::new(meter!(0.0), meter!(-2.5)))
                .is_none()
        );
    }
    /// A grid writes its fluences as its schema says: column by column, then the number of rows
    /// and of columns.
    #[test]
    fn a_grid_writes_its_fluences_column_by_column() {
        let grid = FluenceGrid::with_values(
            meter!(0.0)..meter!(2.0),
            meter!(0.0)..meter!(1.0),
            nalgebra::dmatrix![
                J_per_m2!(1.0), J_per_m2!(2.0), J_per_m2!(3.0);
                J_per_m2!(4.0), J_per_m2!(5.0), J_per_m2!(6.0)],
        );
        let written = serde_json::to_value(&grid).unwrap();
        assert_eq!(
            written["values"],
            serde_json::json!([[1.0, 4.0, 2.0, 5.0, 3.0, 6.0], 2, 3])
        );
    }
    /// A point inside a grid stands for a whole cell, one on an edge for half of it, one in a
    /// corner for a quarter, and a single point along an axis for its whole range.
    #[test]
    fn the_points_of_a_grid_stand_for_the_cells_around_them() {
        let (x, y) = (meter!(0.0)..meter!(4.0), meter!(0.0)..meter!(2.0));
        let areas = FluenceGrid::new(&x, &y, (5, 3))
            .point_areas()
            .map(|area| area.value);
        assert_eq!(
            areas,
            nalgebra::dmatrix![
                0.25, 0.5, 0.5, 0.5, 0.25;
                0.5, 1.0, 1.0, 1.0, 0.5;
                0.25, 0.5, 0.5, 0.5, 0.25]
        );
        let single = FluenceGrid::new(&x, &y, (1, 1)).point_areas();
        assert_eq!(single[(0, 0)].value, 8.0);
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
        grid.add_triangle([bottom_left, bottom_right, top_right], [J_per_m2!(1.0); 3]);
        grid.add_triangle([bottom_left, top_right, top_left], [J_per_m2!(1.0); 3]);
        let values = in_j_per_m2(grid);
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
                grid.add_triangle([bottom_left, bottom_right, top_right], [J_per_m2!(1.0); 3]);
                grid.add_triangle([top_right, top_left, bottom_left], [J_per_m2!(1.0); 3]);
            }
        }
        assert!(in_j_per_m2(grid).iter().all(|v| (*v - 1.0).abs() < 1e-12));
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
            grid.add_triangle(corners, values.map(|value| J_per_m2!(value)));
            let values = in_j_per_m2(grid);
            // the value is x + 2 y + 7.5: 7.5 at the origin, 8.5 one step along x
            assert_relative_eq!(values[(2, 2)], 7.5, epsilon = 1e-12);
            assert_relative_eq!(values[(2, 3)], 8.5, epsilon = 1e-12);
            assert_relative_eq!(values[(3, 2)], 9.5, epsilon = 1e-12);
        }
    }
}
