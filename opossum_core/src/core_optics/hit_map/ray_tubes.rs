//! Fluence maps that follow the ray tubes of the sources (see [`RayMesh`](crate::light::RayMesh)).
//!
//! A tube is alive on a surface if all three of its corners arrived there. The map is drawn over
//! the alive tubes only, so where a bore or an edge cut the light away, it stays dark. Each edge
//! of the alive tubes that no alive tube continues gets a margin of half a tube height: the light
//! ends somewhere inside the torn tube, halfway on average (see the handbook, "Fluence").
use std::{cmp::Ordering, ops::Range};

use log::warn;
use nalgebra::{DMatrix, Point2, Vector2};
use spade::{DelaunayTriangulation, Triangulation, handles::VertexHandle};
use uom::si::f64::Length;

use super::fluence_estimator::FluenceEstimator;
use crate::{
    J_per_m2,
    error::{OpmResult, OpossumError},
    light::ray_mesh::{RayVertex, Tube, margin},
    meter,
    nodes::fluence_detector::fluence_data::FluenceData,
    utils::{to_f64, try_f64_to_usize},
};

/// Where a corner of the ray tubes arrived on a surface and what its rays brought there: their
/// summed energy or fluence.
#[derive(Clone, Copy, Debug)]
pub struct Arrival {
    /// Where the corner arrived, in the coordinates of the surface, in meters.
    pub position: Point2<f64>,
    /// The summed energy (in J) or fluence (in J/m²) of the rays of the corner.
    pub value: f64,
}

/// A sheet of ray tubes, with the corners that arrived on a surface.
pub struct ArrivedSheet<'a> {
    sheet: &'a DelaunayTriangulation<RayVertex>,
    /// The arrival of each corner, indexed like the vertices of the sheet.
    arrivals: Vec<Option<Arrival>>,
}
impl<'a> ArrivedSheet<'a> {
    /// Creates a new [`ArrivedSheet`].
    ///
    /// # Arguments
    ///
    /// * `sheet` - the triangulation of the sheet at its source.
    /// * `arrival_of` - where a corner arrived and what it brought, `None` if it did not arrive.
    pub fn new(
        sheet: &'a DelaunayTriangulation<RayVertex>,
        arrival_of: impl Fn(&RayVertex) -> Option<Arrival>,
    ) -> Self {
        // spade hands out the vertices in the order of their indices
        let arrivals = sheet.vertices().map(|v| arrival_of(v.data())).collect();
        Self { sheet, arrivals }
    }
    /// Returns where the given corner arrived, if it did.
    fn arrival(&self, vertex: VertexHandle<'_, RayVertex>) -> Option<Arrival> {
        self.arrivals.get(vertex.index()).copied().flatten()
    }
    /// Returns where the three corners of the given tube arrived, if all of them did.
    fn positions(&self, tube: Tube<'_>) -> Option<[Point2<f64>; 3]> {
        let [Some(a), Some(b), Some(c)] = tube
            .vertices()
            .map(|v| self.arrival(v).map(|arrival| arrival.position))
        else {
            return None;
        };
        Some([a, b, c])
    }
    /// Returns whether all three corners of the given tube arrived.
    fn alive(&self, tube: Tube<'_>) -> bool {
        self.positions(tube).is_some()
    }
    /// Returns the tubes of this sheet whose corners all arrived, with where they arrived.
    fn alive_tubes(&self) -> impl Iterator<Item = (Tube<'a>, [Point2<f64>; 3])> + '_ {
        self.sheet
            .inner_faces()
            .filter_map(|tube| self.positions(tube).map(|points| (tube, points)))
    }
    /// Returns the fluence, in J/m², each corner of this sheet shows from the energy it brought.
    ///
    /// The energy of a corner spreads over the area it stands for at its source (see
    /// [`RayVertex::cell_area`]), magnified by as much as its alive tubes grew on the way:
    /// `E / A_src · D_src / D_surface`, where `D` is the area of the alive tubes around the
    /// corner. A tube torn away counts on neither side, so a corner at a cut shows the fluence of
    /// the light that is there.
    ///
    /// # Returns
    ///
    /// One value per vertex of the sheet, `None` for a corner without an alive tube.
    pub fn fluences_from_energies(&self) -> Vec<Option<f64>> {
        // the area of the alive tubes around each corner, at the source and on the surface
        let mut areas = vec![(0.0, 0.0); self.arrivals.len()];
        for (tube, points) in self.alive_tubes() {
            let (source, surface) = (tube.area(), signed_area(points).abs());
            for corner in tube.vertices() {
                if let Some((s, d)) = areas.get_mut(corner.index()) {
                    *s += source;
                    *d += surface;
                }
            }
        }
        self.sheet
            .vertices()
            .zip(&self.arrivals)
            .zip(areas)
            .map(|((corner, arrival), (source, surface))| {
                let (arrival, cell) = ((*arrival)?, corner.data().cell_area());
                (surface > 0.0 && cell > 0.0).then(|| arrival.value / cell * source / surface)
            })
            .collect()
    }
    /// Returns the value each corner of this sheet brought, as the fluence it shows when its
    /// rays bring their fluence along: `None` for a corner that did not arrive.
    pub fn arrived_values(&self) -> Vec<Option<f64>> {
        self.arrivals
            .iter()
            .map(|arrival| arrival.map(|arrival| arrival.value))
            .collect()
    }
    /// Hands each triangle this sheet covers the surface with to `draw`: its alive tubes, each
    /// with the values of its corners, and at each edge no alive tube continues a margin of half
    /// a tube height, carrying the values of the edge across.
    ///
    /// # Arguments
    ///
    /// * `values` - the value of each corner, indexed like the vertices of the sheet.
    /// * `draw` - what to do with each triangle, told whether it lies in a margin.
    pub fn for_each_triangle(&self, values: &[Option<f64>], mut draw: impl FnMut(Triangle, bool)) {
        let value_of = |v: VertexHandle<'_, RayVertex>| values.get(v.index()).copied().flatten();
        for (tube, points) in self.alive_tubes() {
            let [Some(fa), Some(fb), Some(fc)] = tube.vertices().map(value_of) else {
                continue;
            };
            draw((points, [fa, fb, fc]), false);
            let alive = |tube: Tube<'_>| self.alive(tube);
            let at = |v: VertexHandle<'_, RayVertex>| self.arrival(v).map(|a| a.position);
            for edge in tube.adjacent_edges() {
                if let (Some(corners), Some(fa), Some(fb)) = (
                    margin(edge, &alive, &at),
                    value_of(edge.from()),
                    value_of(edge.to()),
                ) {
                    margin_triangles(corners, [fa, fb])
                        .into_iter()
                        .flatten()
                        .for_each(|triangle| draw(triangle, true));
                }
            }
        }
    }
    /// Returns whether the light of this sheet folds over itself on the surface, as it does at a
    /// caustic: some of its alive tubes arrived turned over and others did not.
    ///
    /// A tube squashed to less than a billionth of its area at the source counts as neither: it
    /// lies flat, as at a line focus, and rounding decides which way it turns.
    pub fn folds(&self) -> bool {
        const SQUASHED: f64 = 1e-9;
        let (mut kept, mut turned) = (false, false);
        for (tube, points) in self.alive_tubes() {
            // spade's tubes run counterclockwise at the source
            let magnification = signed_area(points) / tube.area();
            kept |= magnification > SQUASHED;
            turned |= magnification < -SQUASHED;
        }
        kept && turned
    }
    /// Returns the smallest box around everything this sheet draws with the given values, as its
    /// lower and upper corner, `None` if it draws nothing.
    ///
    /// # Arguments
    ///
    /// * `values` - the value of each corner, indexed like the vertices of the sheet.
    pub fn extent(&self, values: &[Option<f64>]) -> Option<(Point2<f64>, Point2<f64>)> {
        let mut extent: Option<(Point2<f64>, Point2<f64>)> = None;
        self.for_each_triangle(values, |(corners, _), _| {
            for corner in corners {
                extent = Some(extent.map_or((corner, corner), |(lower, upper)| {
                    (lower.inf(&corner), upper.sup(&corner))
                }));
            }
        });
        extent
    }
}
/// A triangle to draw: its corners on the surface and the values there.
pub type Triangle = ([Point2<f64>; 3], [f64; 3]);
/// Returns the triangles of a margin (see [`margin`]), carrying the values of its edge across:
/// two for a margin of four corners, one where the margin closes to a triangle, whose tip takes
/// the mean of both values.
///
/// # Arguments
///
/// * `corners` - the start and the end of the edge, then the points beyond its end and its start.
/// * `values` - the values at the start and the end of the edge.
fn margin_triangles(
    [start, end, end_out, start_out]: [Point2<f64>; 4],
    [fs, fe]: [f64; 2],
) -> [Option<Triangle>; 2] {
    if end_out == start_out {
        return [
            Some(([start, end, end_out], [fs, fe, fs.midpoint(fe)])),
            None,
        ];
    }
    [
        Some(([start, end, end_out], [fs, fe, fe])),
        Some(([start, end_out, start_out], [fs, fe, fs])),
    ]
}
/// Draws sheets of ray tubes, each with the values of its corners, into one fluence map.
///
/// Where tubes overlap, their values add up. If the light of a sheet folds over itself (see
/// [`ArrivedSheet::folds`]), a warning says that the map is unreliable near the fold. The map
/// tells which share of its energy lies in the margins (see [`FluenceData::edge_share`]), summed
/// over the triangles exactly rather than over the points of the map.
///
/// # Arguments
///
/// * `sheets` - the sheets, each with the fluence (in J/m²) at each of its corners.
/// * `nr_of_points` - the number of points of the map along x and y.
/// * `x`, `y` - the ranges the map covers; if `None`, it covers all the sheets draw.
/// * `estimator` - the estimator the map stands for.
///
/// # Errors
///
/// This function returns an error if no tube arrived complete, so that the sheets draw nothing.
pub fn draw_map(
    sheets: &[(ArrivedSheet<'_>, Vec<Option<f64>>)],
    nr_of_points: (usize, usize),
    x: Option<&Range<Length>>,
    y: Option<&Range<Length>>,
    estimator: FluenceEstimator,
) -> OpmResult<FluenceData> {
    let (lower, upper) = sheets
        .iter()
        .filter_map(|(sheet, values)| sheet.extent(values))
        .reduce(|(lower, upper), (l, u)| (lower.inf(&l), upper.sup(&u)))
        .ok_or_else(|| {
            OpossumError::Analysis(
                "no ray tube arrived complete on this surface, so the light shows no fluence map"
                    .into(),
            )
        })?;
    if sheets.iter().any(|(sheet, _)| sheet.folds()) {
        warn!(
            "light folds over itself on this surface (caustic): the fluence near the fold is unreliable"
        );
    }
    let x = x.cloned().unwrap_or(meter!(lower.x)..meter!(upper.x));
    let y = y.cloned().unwrap_or(meter!(lower.y)..meter!(upper.y));
    let mut grid = FluenceGrid::new(
        &(x.start.value..x.end.value),
        &(y.start.value..y.end.value),
        nr_of_points,
    );
    // the energy of the tubes and of the margins: a linear triangle holds its area times its mean
    let (mut in_tubes, mut in_margins) = (0.0, 0.0);
    for (sheet, values) in sheets {
        sheet.for_each_triangle(values, |(corners, values), in_margin| {
            let energy = signed_area(corners).abs() * values.iter().sum::<f64>() / 3.0;
            if in_margin {
                in_margins += energy;
            } else {
                in_tubes += energy;
            }
            grid.add_triangle(corners, values);
        });
    }
    let map = FluenceData::new(
        grid.values.map(|fluence| J_per_m2!(fluence)),
        x,
        y,
        estimator,
    );
    let total = in_tubes + in_margins;
    Ok(if total > 0.0 {
        map.with_edge_share(in_margins / total)
    } else {
        map
    })
}
/// The signed area of a triangle: positive if its corners run counterclockwise.
fn signed_area([a, b, c]: [Point2<f64>; 3]) -> f64 {
    (b - a).perp(&(c - a)) / 2.0
}

/// A regular grid of points that tubes are drawn onto, the values of overlapping tubes adding up.
pub struct FluenceGrid {
    x: Axis,
    y: Axis,
    /// The values, with a row per point along y and a column per point along x.
    values: DMatrix<f64>,
}
/// The evenly spaced points of a grid along one axis.
struct Axis {
    start: f64,
    step: f64,
    count: usize,
}
impl Axis {
    /// Creates a new [`Axis`] of `count` points from the start to the end of `range`, both
    /// included.
    fn new(range: &Range<f64>, count: usize) -> Self {
        let step = if count > 1 {
            (range.end - range.start) / to_f64(count - 1)
        } else {
            0.0
        };
        Self {
            start: range.start,
            step,
            count,
        }
    }
    /// The coordinate of the point with the given index.
    fn at(&self, index: usize) -> f64 {
        self.step.mul_add(to_f64(index), self.start)
    }
    /// The indices of the points from `from` to `to`, both included.
    fn within(&self, from: f64, to: f64) -> Range<usize> {
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
        // A point on the border of the range may be off by rounding; which side of an edge it
        // lies on is decided when the triangle is drawn.
        const ROUNDING: f64 = 1e-9;
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
    /// * `x` - the range of the grid along x, in meters, both ends included.
    /// * `y` - the range of the grid along y, in meters, both ends included.
    /// * `nr_of_points` - the number of points along x and y.
    #[must_use]
    pub fn new(x: &Range<f64>, y: &Range<f64>, nr_of_points: (usize, usize)) -> Self {
        Self {
            x: Axis::new(x, nr_of_points.0),
            y: Axis::new(y, nr_of_points.1),
            values: DMatrix::zeros(nr_of_points.1, nr_of_points.0),
        }
    }
    /// Adds a triangle to the grid, linearly between the values at its corners.
    ///
    /// A point on an edge two triangles share is added once only: it belongs to the triangle for
    /// which the edge is a top or left one.
    fn add_triangle(&mut self, mut corners: [Point2<f64>; 3], mut values: [f64; 3]) {
        let twice_area = 2.0 * signed_area(corners);
        match twice_area.partial_cmp(&0.0) {
            Some(Ordering::Greater) => {}
            Some(Ordering::Less) => {
                corners.swap(1, 2);
                values.swap(1, 2);
            }
            _ => return,
        }
        let per_area = twice_area.abs().recip();
        let [p0, p1, p2] = corners;
        // each edge lies opposite the corner of the same index
        let edges = [(p1, p2), (p2, p0), (p0, p1)];
        let top_left = edges.map(|(a, b)| is_top_left(b - a));
        let (lower, upper) = (p0.inf(&p1).inf(&p2), p0.sup(&p1).sup(&p2));
        for column in self.x.within(lower.x, upper.x) {
            for row in self.y.within(lower.y, upper.y) {
                let point = Point2::new(self.x.at(column), self.y.at(row));
                let weights = edges.map(|(a, b)| edge_weight(a, b, point));
                let covered = weights
                    .iter()
                    .zip(top_left)
                    .all(|(w, top_left)| *w > 0.0 || (top_left && *w >= 0.0));
                if covered && let Some(cell) = self.values.get_mut((row, column)) {
                    *cell += weights.iter().zip(values).map(|(w, v)| w * v).sum::<f64>() * per_area;
                }
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
    use approx::assert_relative_eq;

    /// A grid of 1 m spacing from -2 m to 2 m in x and y.
    fn unit_grid() -> FluenceGrid {
        FluenceGrid::new(&(-2.0..2.0), &(-2.0..2.0), (5, 5))
    }
    /// The two triangles of a square cover the point on their shared diagonal once, and of the
    /// points on the border of the square those on its bottom and left edges: the others belong
    /// to the squares beyond.
    #[test]
    fn two_triangles_of_a_square_cover_each_point_once() {
        let mut grid = unit_grid();
        let (a, b, c, d) = (
            Point2::new(-1.0, -1.0),
            Point2::new(1.0, -1.0),
            Point2::new(1.0, 1.0),
            Point2::new(-1.0, 1.0),
        );
        grid.add_triangle([a, b, c], [1.0; 3]);
        grid.add_triangle([a, c, d], [1.0; 3]);
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
                let (a, b, c, d) = (
                    Point2::new(x, y),
                    Point2::new(x + 1.0, y),
                    Point2::new(x + 1.0, y + 1.0),
                    Point2::new(x, y + 1.0),
                );
                grid.add_triangle([a, b, c], [1.0; 3]);
                grid.add_triangle([c, d, a], [1.0; 3]);
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
    /// Draws the triangles of a margin of the given corners and values onto a grid of 1 m
    /// spacing from 0 m to 4 m in x and from -4 m to 0 m in y.
    fn draw_margin(corners: [Point2<f64>; 4], values: [f64; 2]) -> DMatrix<f64> {
        let mut grid = FluenceGrid::new(&(0.0..4.0), &(-4.0..0.0), (5, 5));
        for (corners, values) in margin_triangles(corners, values).into_iter().flatten() {
            grid.add_triangle(corners, values);
        }
        grid.values
    }
    /// A margin carries the values of its edge across.
    #[test]
    fn a_margin_carries_the_values_of_its_edge_across() {
        // a margin 2 deep below the edge from (0, 0) to (4, 0)
        let values = draw_margin(
            [
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 0.0),
                Point2::new(4.0, -2.0),
                Point2::new(0.0, -2.0),
            ],
            [1.0, 3.0],
        );
        // row 3 lies at y = -1 within the margin, row 1 at y = -3 beyond it
        assert_relative_eq!(values[(3, 1)], 1.5, epsilon = 1e-12);
        assert_relative_eq!(values[(3, 2)], 2.0, epsilon = 1e-12);
        assert!(values.row(1).iter().all(|v| *v == 0.0), "{values}");
    }
    /// A margin that closes to a triangle carries the mean of the values of its edge to its tip:
    /// below the middle of the edge it shows that mean all the way down.
    #[test]
    fn a_margin_closing_to_a_tip_carries_the_mean_there() {
        let tip = Point2::new(2.0, -4.0);
        let values = draw_margin(
            [Point2::new(0.0, 0.0), Point2::new(4.0, 0.0), tip, tip],
            [1.0, 3.0],
        );
        // rows 1 and 3 lie at y = -3 and y = -1, column 2 at x = 2, column 1 at x = 1
        assert_relative_eq!(values[(1, 2)], 2.0, epsilon = 1e-12);
        assert_relative_eq!(values[(3, 2)], 2.0, epsilon = 1e-12);
        assert_relative_eq!(values[(3, 1)], 1.5, epsilon = 1e-12);
    }
}
