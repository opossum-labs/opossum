//! Fluence maps that follow the ray tubes of the sources (see [`RayMesh`](crate::light::RayMesh)).
//!
//! A tube is alive on a surface if all three of its corners arrived there. The map is drawn over
//! the alive tubes only, so where a bore or an edge cut the light away, it stays dark. Each edge
//! of the alive tubes that no alive tube continues gets a margin of half a tube height: the light
//! ends somewhere inside the torn tube, halfway on average (see the handbook, "Fluence").
use std::ops::Range;

use log::warn;
use nalgebra::{DMatrix, DVector, Matrix2, Point2, Vector2};
use spade::{DelaunayTriangulation, Triangulation, handles::VertexHandle};
use uom::si::f64::{Energy, Length};

use super::fluence_estimator::FluenceEstimator;
use crate::{
    J_per_m2,
    error::{OpmResult, OpossumError},
    joule,
    light::ray_mesh::{RayVertex, Tube, margin, spans},
    meter,
    nodes::fluence_detector::{Fluence, fluence_data::FluenceData, fluence_grid::FluenceGrid},
    square_meter,
    utils::{math_utils::signed_area, to_f64},
};

/// Where a corner of the ray tubes arrived on a surface and the energy its rays brought there.
#[derive(Clone, Copy, Debug)]
pub struct Arrival {
    /// Where the corner arrived, in the coordinates of the surface, in meters.
    pub position: Point2<f64>,
    /// The summed energy of the rays of the corner.
    pub energy: Energy,
}

/// A sheet of ray tubes, with the corners that arrived on a surface and the fluence they show.
pub struct ArrivedSheet<'a> {
    sheet: &'a DelaunayTriangulation<RayVertex>,
    /// The arrival of each corner, indexed like the vertices of the sheet.
    arrivals: Vec<Option<Arrival>>,
    /// The fluence each corner shows (see [`ArrivedSheet::fluences_from_energies`]), indexed like
    /// the vertices of the sheet.
    fluences: Vec<Option<Fluence>>,
}
impl<'a> ArrivedSheet<'a> {
    /// Creates a new [`ArrivedSheet`], with the fluence each of its corners shows.
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
        let mut arrived = Self {
            sheet,
            arrivals,
            fluences: Vec::new(),
        };
        arrived.fluences = arrived.fluences_from_energies();
        arrived
    }
    /// Returns where the given corner arrived, if it did.
    fn arrival(&self, vertex: VertexHandle<'_, RayVertex>) -> Option<Arrival> {
        self.arrivals.get(vertex.index()).copied().flatten()
    }
    /// Returns the fluence the given corner shows, if it shows one.
    fn fluence(&self, vertex: VertexHandle<'_, RayVertex>) -> Option<Fluence> {
        self.fluences.get(vertex.index()).copied().flatten()
    }
    /// Returns where the three corners of the given tube arrived, if all of them did and the tube
    /// spans an area (see [`spans`]).
    fn positions(&self, tube: Tube<'_>) -> Option<[Point2<f64>; 3]> {
        if !spans(tube) {
            return None;
        }
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
    /// Returns the fluence each corner of this sheet shows from the energy it brought.
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
    fn fluences_from_energies(&self) -> Vec<Option<Fluence>> {
        // the area of the alive tubes around each corner, at the source and on the surface
        let mut around = vec![(0.0, 0.0); self.arrivals.len()];
        for (tube, points) in self.alive_tubes() {
            let (tube_at_source, tube_on_surface) = (tube.area(), signed_area(points).abs());
            for corner in tube.vertices() {
                if let Some((at_source, on_surface)) = around.get_mut(corner.index()) {
                    *at_source += tube_at_source;
                    *on_surface += tube_on_surface;
                }
            }
        }
        self.sheet
            .vertices()
            .zip(&self.arrivals)
            .zip(around)
            .map(|((corner, arrival), (at_source, on_surface))| {
                let (arrival, cell) = ((*arrival)?, corner.data().cell_area());
                (on_surface > 0.0 && cell > 0.0)
                    .then(|| arrival.energy / cell * at_source / square_meter!(on_surface))
            })
            .collect()
    }
    /// Returns the curvature of the fluence at each corner of this sheet: the Hessian `H` of
    /// `Δ = g·d + ½ dᵀ H d`, fitted by least squares to the steps `d` to the neighbors the corner
    /// shares an alive tube with and the changes `Δ` of the fluence along them (see the handbook,
    /// "Fluence uncertainty", which also states when the fit is exact).
    ///
    /// # Returns
    ///
    /// One curvature per vertex of the sheet, in J/m⁴, `None` for a corner without a fluence or
    /// whose steps do not fix all five coefficients of the fit, as with fewer than five neighbors.
    pub fn curvatures(&self) -> Vec<Option<Matrix2<f64>>> {
        let alive = |tube: Option<Tube<'_>>| tube.is_some_and(|tube| self.alive(tube));
        self.sheet
            .vertices()
            .map(|corner| {
                let (at, value) = (self.arrival(corner)?.position, self.fluence(corner)?);
                let steps = corner
                    .out_edges()
                    .filter(|edge| {
                        alive(edge.face().as_inner()) || alive(edge.rev().face().as_inner())
                    })
                    .filter_map(|edge| {
                        let neighbor = edge.to();
                        Some((
                            self.arrival(neighbor)?.position - at,
                            (self.fluence(neighbor)? - value).value,
                        ))
                    })
                    .collect::<Vec<_>>();
                fitted_curvature(&steps)
            })
            .collect()
    }
    /// Returns, for each alive tube of this sheet with a corner that tells the curvature of the
    /// fluence (see [`ArrivedSheet::curvatures`]), its corners on the surface and the curvature
    /// along each of its edges, `eᵀ H e`, the edge opposite the corner of the same index. `H` is
    /// the mean of the curvatures its corners tell (see the handbook, "Fluence uncertainty").
    pub fn deviations(&self) -> Vec<([Point2<f64>; 3], [Fluence; 3])> {
        let curvatures = self.curvatures();
        self.alive_tubes()
            .filter_map(|(tube, points)| {
                let known = tube
                    .vertices()
                    .iter()
                    .filter_map(|v| curvatures.get(v.index()).copied().flatten())
                    .collect::<Vec<_>>();
                if known.is_empty() {
                    return None;
                }
                let curvature =
                    known.iter().fold(Matrix2::zeros(), |sum, h| sum + h) / to_f64(known.len());
                let along = |edge: Vector2<f64>| J_per_m2!(edge.dot(&(curvature * edge)));
                let [p0, p1, p2] = points;
                Some((points, [along(p2 - p1), along(p0 - p2), along(p1 - p0)]))
            })
            .collect()
    }
    /// Hands each triangle this sheet covers the surface with to `draw`: its alive tubes, each
    /// with the fluences of its corners, and at each edge no alive tube continues a margin of
    /// half a tube height, carrying the fluences of the edge across.
    ///
    /// # Arguments
    ///
    /// * `draw` - what to do with each triangle, told whether it lies in a margin.
    pub fn for_each_triangle(&self, mut draw: impl FnMut(Triangle, bool)) {
        for (tube, points) in self.alive_tubes() {
            let [Some(fa), Some(fb), Some(fc)] = tube.vertices().map(|v| self.fluence(v)) else {
                continue;
            };
            draw((points, [fa, fb, fc]), false);
            let alive = |tube: Tube<'_>| self.alive(tube);
            let at = |v: VertexHandle<'_, RayVertex>| self.arrival(v).map(|a| a.position);
            for edge in tube.adjacent_edges() {
                if let (Some(corners), Some(fa), Some(fb)) = (
                    margin(edge, &alive, &at),
                    self.fluence(edge.from()),
                    self.fluence(edge.to()),
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
    /// Returns the smallest box around everything this sheet draws, as its lower and upper
    /// corner, `None` if it draws nothing.
    pub fn extent(&self) -> Option<(Point2<f64>, Point2<f64>)> {
        let mut extent: Option<(Point2<f64>, Point2<f64>)> = None;
        self.for_each_triangle(|(corners, _), _| {
            for corner in corners {
                extent = Some(extent.map_or((corner, corner), |(lower, upper)| {
                    (lower.inf(&corner), upper.sup(&corner))
                }));
            }
        });
        extent
    }
}
/// A triangle to draw: its corners on the surface and the fluences there.
pub type Triangle = ([Point2<f64>; 3], [Fluence; 3]);
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
    [fs, fe]: [Fluence; 2],
) -> [Option<Triangle>; 2] {
    if end_out == start_out {
        return [
            Some(([start, end, end_out], [fs, fe, (fs + fe) / 2.0])),
            None,
        ];
    }
    [
        Some(([start, end, end_out], [fs, fe, fe])),
        Some(([start, end_out, start_out], [fs, fe, fs])),
    ]
}
/// Draws sheets of ray tubes, each with the fluences of its corners, into one fluence map.
///
/// Where tubes overlap, their fluences add up. If the light of a sheet folds over itself (see
/// [`ArrivedSheet::folds`]), a warning says that the map is unreliable near the fold. The map
/// tells the energy it draws in the tubes and in the margins (see
/// [`FluenceData::with_drawn_energy`]), summed over the triangles exactly rather than over the
/// points of the map.
///
/// # Arguments
///
/// * `sheets` - the sheets to draw.
/// * `nr_of_points` - the number of points of the map along x and y.
/// * `x`, `y` - the ranges the map covers; if `None`, it covers all the sheets draw.
/// * `estimator` - the estimator the map stands for.
///
/// # Errors
///
/// This function returns an error if no tube arrived complete, so that the sheets draw nothing.
pub fn draw_map(
    sheets: &[ArrivedSheet<'_>],
    nr_of_points: (usize, usize),
    x: Option<&Range<Length>>,
    y: Option<&Range<Length>>,
    estimator: FluenceEstimator,
) -> OpmResult<FluenceData> {
    let (lower, upper) = sheets
        .iter()
        .filter_map(ArrivedSheet::extent)
        .reduce(|(lower, upper), (l, u)| (lower.inf(&l), upper.sup(&u)))
        .ok_or_else(|| {
            OpossumError::Analysis(
                "no ray tube arrived complete on this surface, so the light shows no fluence map"
                    .into(),
            )
        })?;
    if sheets.iter().any(ArrivedSheet::folds) {
        warn!(
            "light folds over itself on this surface (caustic): the fluence near the fold is unreliable"
        );
    }
    // without a given range, the map is square and reaches 5 % beyond the light along its longer
    // side, so the edges of the light lie inside it rather than on its outermost points
    let (side, center) = (
        1.05 * (upper - lower).max(),
        nalgebra::center(&lower, &upper),
    );
    let around = |middle: f64| meter!(middle - side / 2.0)..meter!(middle + side / 2.0);
    let x = x.cloned().unwrap_or_else(|| around(center.x));
    let y = y.cloned().unwrap_or_else(|| around(center.y));
    let mut grid = FluenceGrid::new(&x, &y, nr_of_points);
    // the energy of the tubes and of the margins: a linear triangle holds its area times its mean
    let (mut in_tubes, mut in_margins) = (joule!(0.0), joule!(0.0));
    for sheet in sheets {
        sheet.for_each_triangle(|(corners, values), in_margin| {
            let energy = square_meter!(signed_area(corners).abs())
                * values.into_iter().sum::<Fluence>()
                / 3.0;
            if in_margin {
                in_margins += energy;
            } else {
                in_tubes += energy;
            }
            grid.add_triangle(corners, values);
        });
    }
    Ok(FluenceData::new(grid.into_values(), x, y, estimator)
        .with_drawn_energy(in_tubes, in_margins))
}
/// Returns the given map with the estimated deviation of the fluence from it at each of its
/// points, drawn from sheets of ray tubes (see [`ArrivedSheet::deviations`] and the handbook,
/// "Fluence uncertainty"). Overlapping tubes add up as in the map.
///
/// # Arguments
///
/// * `sheets` - the sheets the map was drawn from.
/// * `map` - the map, whose points the deviation is drawn onto.
#[must_use]
pub fn draw_deviation(sheets: &[ArrivedSheet<'_>], map: FluenceData) -> FluenceData {
    let mut grid = FluenceGrid::new(map.x_range(), map.y_range(), (map.len_x(), map.len_y()));
    for sheet in sheets {
        for (corners, along_edges) in sheet.deviations() {
            grid.add_deviation(corners, along_edges);
        }
    }
    map.with_deviation(grid.into_values())
}
/// Fits `Δ = g·d + ½ dᵀ H d` by least squares to the given steps `d` from a corner to its
/// neighbors and the changes `Δ` of the value along them, and returns `H`; `None` if the steps
/// do not fix all five coefficients, its singular values spreading by more than 10¹⁰ (see the
/// handbook, "Fluence uncertainty").
fn fitted_curvature(steps: &[(Vector2<f64>, f64)]) -> Option<Matrix2<f64>> {
    // the steps measured in the longest of them, so that the columns of the fit are alike in size
    let scale = steps.iter().map(|(d, _)| d.norm()).fold(0.0, f64::max);
    if steps.len() < 5 || scale <= 0.0 {
        return None;
    }
    let rows = steps.iter().flat_map(|(d, _)| {
        let d = d / scale;
        [d.x, d.y, d.x * d.x / 2.0, d.x * d.y, d.y * d.y / 2.0]
    });
    let fit = DMatrix::from_row_iterator(steps.len(), 5, rows);
    let changes = DVector::from_iterator(steps.len(), steps.iter().map(|(_, change)| *change));
    let svd = fit.svd(true, true);
    if svd.singular_values.min() <= 1e-10 * svd.singular_values.max() {
        return None;
    }
    let coefficients = svd.solve(&changes, 0.0).ok()?;
    let &[_, _, xx, xy, yy] = coefficients.as_slice() else {
        return None;
    };
    // a curvature measured in steps of `scale` is `scale²` times that measured in meters
    Some(Matrix2::new(xx, xy, xy, yy) / (scale * scale))
}

#[cfg(test)]
mod test {
    use super::*;
    use approx::assert_relative_eq;

    /// Draws the triangles of a margin of the given corners and values onto a grid of 1 m
    /// spacing from 0 m to 4 m in x and from -4 m to 0 m in y.
    fn draw_margin(corners: [Point2<f64>; 4], values: [f64; 2]) -> DMatrix<f64> {
        let mut grid = FluenceGrid::new(
            &(meter!(0.0)..meter!(4.0)),
            &(meter!(-4.0)..meter!(0.0)),
            (5, 5),
        );
        let values = values.map(|value| J_per_m2!(value));
        for (corners, values) in margin_triangles(corners, values).into_iter().flatten() {
            grid.add_triangle(corners, values);
        }
        grid.into_values().map(|fluence| fluence.value)
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
