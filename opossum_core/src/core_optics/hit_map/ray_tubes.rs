//! Fluence maps that follow the ray tubes of the sources (see [`RayMesh`](crate::light::RayMesh)).
//!
//! A tube is alive on a surface if all three of its corners arrived there. The map is drawn over
//! the alive tubes only, so where a bore or an edge cut the light away, it stays dark. Each edge
//! of the alive tubes that no alive tube continues gets a margin of half a tube height: the light
//! ends somewhere inside the torn tube, halfway on average (see the handbook, "Fluence").
use std::{cmp::Ordering, ops::Range};

use nalgebra::{DMatrix, Point2, Vector2};
use spade::{
    DelaunayTriangulation, Triangulation,
    handles::{FaceHandle, InnerTag, VertexHandle},
};
use uom::si::f64::Length;

use super::fluence_estimator::FluenceEstimator;
use crate::{
    J_per_m2,
    light::ray_mesh::RayVertex,
    meter,
    nodes::fluence_detector::fluence_data::FluenceData,
    utils::{to_f64, try_f64_to_usize},
};

/// A ray tube: an inner face of the triangulation of a sheet.
type Tube<'a> = FaceHandle<'a, InnerTag, RayVertex, (), (), ()>;

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
        let mut arrivals = vec![None; sheet.num_vertices()];
        for vertex in sheet.vertices() {
            if let Some(arrival) = arrivals.get_mut(vertex.index()) {
                *arrival = arrival_of(vertex.data());
            }
        }
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
    /// The energy of a corner spreads over the area it stands for at its source, magnified by
    /// as much as its alive tubes grew on the way: `E / A_src · D_src / D_surface`, where `D` is a
    /// third of the area of the alive tubes around the corner. A tube torn away counts on
    /// neither side, so a corner at a cut shows the fluence of the light that is there.
    ///
    /// # Returns
    ///
    /// One value per vertex of the sheet, `None` for a corner without an alive tube.
    pub fn fluences_from_energies(&self) -> Vec<Option<f64>> {
        let mut at_source = vec![0.0; self.arrivals.len()];
        let mut on_surface = vec![0.0; self.arrivals.len()];
        for (tube, points) in self.alive_tubes() {
            let (source, surface) = (tube.area() / 3.0, signed_area(points).abs() / 3.0);
            for corner in tube.vertices() {
                if let (Some(s), Some(d)) = (
                    at_source.get_mut(corner.index()),
                    on_surface.get_mut(corner.index()),
                ) {
                    *s += source;
                    *d += surface;
                }
            }
        }
        self.sheet
            .vertices()
            .map(|corner| {
                let arrival = self.arrival(corner)?;
                let source = *at_source.get(corner.index())?;
                let surface = *on_surface.get(corner.index())?;
                let cell = source_cell_area(corner);
                (surface > 0.0 && cell > 0.0).then(|| arrival.value / cell * source / surface)
            })
            .collect()
    }
    /// Hands each triangle this sheet covers the surface with to `draw`: its alive tubes, each
    /// with the values of its corners, and at each edge no alive tube continues a margin of half
    /// a tube height, carrying the values of the edge across.
    ///
    /// # Arguments
    ///
    /// * `values` - the value of each corner, indexed like the vertices of the sheet.
    /// * `draw` - what to do with each triangle.
    pub fn for_each_triangle(&self, values: &[Option<f64>], mut draw: impl FnMut(Triangle)) {
        let value_of = |v: VertexHandle<'_, RayVertex>| values.get(v.index()).copied().flatten();
        for (tube, points) in self.alive_tubes() {
            let [Some(fa), Some(fb), Some(fc)] = tube.vertices().map(value_of) else {
                continue;
            };
            draw((points, [fa, fb, fc]));
            let area = signed_area(points).abs();
            for edge in tube.adjacent_edges() {
                if edge
                    .rev()
                    .face()
                    .as_inner()
                    .is_some_and(|next| self.alive(next))
                {
                    continue;
                }
                let third = edge.opposite_vertex().and_then(|v| self.arrival(v));
                if let (Some(a), Some(b), Some(c), Some(fa), Some(fb)) = (
                    self.arrival(edge.from()),
                    self.arrival(edge.to()),
                    third,
                    value_of(edge.from()),
                    value_of(edge.to()),
                ) {
                    margin_triangles([a.position, b.position], c.position, area, [fa, fb])
                        .into_iter()
                        .flatten()
                        .for_each(&mut draw);
                }
            }
        }
    }
    /// Returns the smallest box around everything this sheet draws with the given values, as its
    /// lower and upper corner, `None` if it draws nothing.
    ///
    /// # Arguments
    ///
    /// * `values` - the value of each corner, indexed like the vertices of the sheet.
    pub fn extent(&self, values: &[Option<f64>]) -> Option<(Point2<f64>, Point2<f64>)> {
        let mut extent: Option<(Point2<f64>, Point2<f64>)> = None;
        self.for_each_triangle(values, |(corners, _)| {
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
/// Returns the margin beyond an edge of a tube: a strip of half the tube height, running out from
/// the edge on the side away from the third corner, as two triangles carrying the values of the
/// edge across; `None` for an edge of no length.
///
/// # Arguments
///
/// * `edge` - the two corners of the edge.
/// * `third` - the third corner of the tube.
/// * `tube_area` - the area of the tube on the surface.
/// * `values` - the values at the two corners of the edge.
fn margin_triangles(
    [a, b]: [Point2<f64>; 2],
    third: Point2<f64>,
    tube_area: f64,
    [fa, fb]: [f64; 2],
) -> Option<[Triangle; 2]> {
    let along = b - a;
    let length = along.norm();
    if length <= 0.0 {
        return None;
    }
    let mut outward = Vector2::new(along.y, -along.x) / length;
    if outward.dot(&(third - a)) > 0.0 {
        outward = -outward;
    }
    let across = outward * (tube_area / length);
    let (a_out, b_out) = (a + across, b + across);
    Some([
        ([a, b, b_out], [fa, fb, fb]),
        ([a, b_out, a_out], [fa, fb, fa]),
    ])
}
/// Draws sheets of ray tubes, each with the values of its corners, into one fluence map.
///
/// # Arguments
///
/// * `sheets` - the sheets, each with the fluence (in J/m²) at each of its corners.
/// * `nr_of_points` - the number of points of the map along x and y.
/// * `x`, `y` - the ranges the map covers; if `None`, it covers all the sheets draw.
/// * `otherwise` - the ranges the map covers if none are given and the sheets draw nothing.
/// * `estimator` - the estimator the map stands for.
pub fn draw_map(
    sheets: &[(ArrivedSheet<'_>, Vec<Option<f64>>)],
    nr_of_points: (usize, usize),
    x: Option<&Range<Length>>,
    y: Option<&Range<Length>>,
    otherwise: (Range<Length>, Range<Length>),
    estimator: FluenceEstimator,
) -> FluenceData {
    let drawn = sheets
        .iter()
        .filter_map(|(sheet, values)| sheet.extent(values))
        .reduce(|(lower, upper), (l, u)| (lower.inf(&l), upper.sup(&u)));
    let (x, y) = match (x, y, drawn) {
        (Some(x), Some(y), _) => (x.clone(), y.clone()),
        (x, y, Some((lower, upper))) => (
            x.cloned().unwrap_or(meter!(lower.x)..meter!(upper.x)),
            y.cloned().unwrap_or(meter!(lower.y)..meter!(upper.y)),
        ),
        (x, y, None) => (
            x.cloned().unwrap_or(otherwise.0),
            y.cloned().unwrap_or(otherwise.1),
        ),
    };
    let mut grid = FluenceGrid::new(
        &(x.start.value..x.end.value),
        &(y.start.value..y.end.value),
        nr_of_points,
    );
    for (sheet, values) in sheets {
        sheet.for_each_triangle(values, |(corners, values)| {
            grid.add_triangle(corners, values)
        });
    }
    FluenceData::new(
        grid.values.map(|fluence| J_per_m2!(fluence)),
        x,
        y,
        estimator,
    )
}
/// The signed area of a triangle: positive if its corners run counterclockwise.
fn signed_area([a, b, c]: [Point2<f64>; 3]) -> f64 {
    cross(b - a, c - a) / 2.0
}
/// The z component of the cross product of two vectors of the plane.
fn cross(u: Vector2<f64>, v: Vector2<f64>) -> f64 {
    u.x.mul_add(v.y, -u.y * v.x)
}
/// The area a corner stands for at its source: its Voronoi cell, and beyond each outer edge of
/// its sheet half of the margin of half a tube height there.
///
/// The Voronoi cell is summed from the tubes around the corner, each contributing the part of
/// itself that is closer to the corner than to its other corners. That holds however the tubes of
/// a regular grid are cut into triangles, which a third of their areas does not.
fn source_cell_area(corner: VertexHandle<'_, RayVertex>) -> f64 {
    let at = |v: VertexHandle<'_, RayVertex>| Point2::new(v.position().x, v.position().y);
    corner
        .out_edges()
        .map(|edge| {
            // Every tube around the corner lies left of one of its edges, every outer edge
            // of the sheet through it is one of them.
            let (left, right) = (edge.face().as_inner(), edge.rev().face().as_inner());
            let inside = left.map_or(0.0, |tube| {
                let mut others = tube
                    .vertices()
                    .into_iter()
                    .filter(|v| v.index() != corner.index())
                    .map(at);
                match (others.next(), others.next()) {
                    (Some(q), Some(r)) => voronoi_part(at(corner), q, r),
                    _ => 0.0,
                }
            });
            let margin = match (left, right) {
                (Some(tube), None) | (None, Some(tube)) => tube.area() / 2.0,
                _ => 0.0,
            };
            inside + margin
        })
        .sum()
}
/// The part of the triangle `p q r` that lies closer to `p` than to `q` and `r`, signed: negative
/// beyond an obtuse angle, where the parts of the neighbors make up for it.
fn voronoi_part(p: Point2<f64>, q: Point2<f64>, r: Point2<f64>) -> f64 {
    let twice_area = cross(q - p, r - p).abs();
    if twice_area <= 0.0 {
        return 0.0;
    }
    let (pq, pr) = ((p - q).norm_squared(), (p - r).norm_squared());
    pq.mul_add((p - r).dot(&(q - r)), pr * (p - q).dot(&(r - q))) / (8.0 * twice_area)
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
            (Some(first), Some(last)) if first <= last && first < self.count => first..last + 1,
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
        if twice_area.partial_cmp(&0.0) != Some(Ordering::Greater) {
            if twice_area.partial_cmp(&0.0) != Some(Ordering::Less) {
                return;
            }
            corners.swap(1, 2);
            values.swap(1, 2);
        }
        let twice_area = twice_area.abs();
        let [p0, p1, p2] = corners;
        let (min_x, max_x) = (p0.x.min(p1.x).min(p2.x), p0.x.max(p1.x).max(p2.x));
        let (min_y, max_y) = (p0.y.min(p1.y).min(p2.y), p0.y.max(p1.y).max(p2.y));
        for column in self.x.within(min_x, max_x) {
            for row in self.y.within(min_y, max_y) {
                let point = Point2::new(self.x.at(column), self.y.at(row));
                let weights = [(p1, p2), (p2, p0), (p0, p1)].map(|(a, b)| edge_weight(a, b, point));
                let covered =
                    weights
                        .iter()
                        .zip([(p1, p2), (p2, p0), (p0, p1)])
                        .all(|(w, (a, b))| match w.partial_cmp(&0.0) {
                            Some(Ordering::Greater) => true,
                            Some(Ordering::Equal) => is_top_left(b - a),
                            _ => false,
                        });
                if covered && let Some(cell) = self.values.get_mut((row, column)) {
                    *cell +=
                        weights.iter().zip(values).map(|(w, v)| w * v).sum::<f64>() / twice_area;
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
        cross(b - a, point - a)
    } else {
        -cross(a - b, point - b)
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
    /// The margin runs half the tube height beyond the edge, away from the third corner, and
    /// carries the values of the edge across.
    #[test]
    fn a_margin_runs_half_a_tube_height_out() {
        let mut grid = FluenceGrid::new(&(0.0..4.0), &(-4.0..0.0), (5, 5));
        // a tube of height 4 above the edge from (0, 0) to (4, 0): its margin reaches 2 below it
        let (a, b, third) = (
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(0.0, 4.0),
        );
        for (corners, values) in margin_triangles([a, b], third, 8.0, [1.0, 3.0])
            .into_iter()
            .flatten()
        {
            grid.add_triangle(corners, values);
        }
        let values = grid.values;
        // row 3 lies at y = -1 within the margin, row 1 at y = -3 beyond it
        assert_relative_eq!(values[(3, 1)], 1.5, epsilon = 1e-12);
        assert_relative_eq!(values[(3, 2)], 2.0, epsilon = 1e-12);
        assert!(values.row(1).iter().all(|v| *v == 0.0), "{values}");
    }
    /// A Voronoi part of a right isosceles triangle is a quarter of its square at the right angle
    /// and an eighth at each of the other corners, so the parts make up the triangle.
    #[test]
    fn the_voronoi_parts_of_a_triangle_make_it_up() {
        let (p, q, r) = (
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 1.0),
        );
        assert_relative_eq!(voronoi_part(p, q, r), 0.25);
        assert_relative_eq!(voronoi_part(q, r, p), 0.125);
        assert_relative_eq!(voronoi_part(r, p, q), 0.125);
    }
}
