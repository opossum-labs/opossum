//! Triangulation of the area an [`Outline`](super::Outline) encloses.

use super::{doubled_signed_area, ring_perimeter};
use crate::{
    error::{OpmResult, OpossumError},
    meter,
    utils::math_utils::{to_f64, try_f64_to_usize},
};
use nalgebra::Point2;
use spade::{
    ConstrainedDelaunayTriangulation, Point2 as SpadePoint, RefinementParameters, Triangulation,
    handles::{FixedFaceHandle, InnerTag},
};
use std::{collections::HashSet, ops::Range};
use uom::si::f64::Length;

/// A triangulation of the area an [`Outline`](super::Outline) encloses, in the plane of the
/// outline.
///
/// Where [`OutlineShape::outline_points`](super::OutlineShape::outline_points) gives only the rim,
/// this fills what the rims enclose: triangles covering the whole area, with points inside it and
/// not just along its edge. That is what a curved face needs to be laid onto — sampled at the rim
/// alone it would be flat.
///
/// The points of its rims come first, so a body built from two such faces can join them along
/// exactly those points; see [`rims`](OutlineMesh::rims).
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineMesh {
    points: Vec<Point2<Length>>,
    triangles: Vec<[u32; 3]>,
    rims: Vec<Range<usize>>,
}

/// How much room the refinement gets beyond the points it is expected to need.
///
/// Refinement can fail to terminate, so it has to be capped; the cap must still be generous enough
/// that a mesh which is merely awkward does not hit it. See
/// [`RefinementParameters::with_max_additional_vertices`].
const REFINEMENT_HEADROOM: f64 = 4.0;

/// How much of the triangulated area the rims and the outline may disagree about, as a fraction.
///
/// The points along a straight rim are no longer exactly in line once a bore is turned, which
/// leaves triangles without area between them whose centre lies on the very edge. A rim out of
/// place misplaces at least one whole triangle, many orders of magnitude more.
const ROUNDING_SLIVERS: f64 = 1e-9;

impl OutlineMesh {
    /// Triangulate the area the given rims enclose.
    ///
    /// The rims are held fixed; the inside is then filled with further points until no triangle is
    /// larger than an equilateral one whose side is the mean spacing of the points on the most
    /// coarsely sampled rim. Holding the rims fixed is what lets two faces meshed over the same
    /// outline be joined along them afterwards — at the price that triangles touching a rim may
    /// come out somewhat larger than that, since a point dividing them would have to encroach on
    /// the rim.
    ///
    /// The result is reproducible: the same rims always yield the same mesh, which is what lets two
    /// identical optics share one mesh when they are exported.
    ///
    /// # Arguments
    ///
    /// - `rims`: the closed edges of the area, each walked with the area on its left: the edge
    ///   counter-clockwise, a bore clockwise
    /// - `inside`: whether a point of the plane lies in the area
    ///
    /// # Returns
    ///
    /// The triangulated area, with its points in the plane of the rims.
    ///
    /// # Errors
    ///
    /// This function returns an error if a point is visited twice, if the rims cross, if the
    /// triangles inside the rims disagree with `inside`, or if the area could not be filled within
    /// the point budget.
    pub(super) fn from_rims(
        rims: &[Vec<Point2<Length>>],
        inside: impl Fn(Point2<Length>) -> bool,
    ) -> OpmResult<Self> {
        // `spade` removes duplicate points before it triangulates, which would shift every index
        // behind the duplicate and quietly mislabel the rims. Rims visiting a point twice pinch the
        // area off there anyway.
        let mut seen = HashSet::new();
        for point in rims.iter().flatten() {
            if !seen.insert((point.x.value.to_bits(), point.y.value.to_bits())) {
                return Err(OpossumError::Other(
                    "the outline visits the same point twice and does not enclose an area".into(),
                ));
            }
        }
        // Rim after rim, which makes each of them a range of the points.
        let outline = rims.concat();
        let mut ranges = Vec::with_capacity(rims.len());
        for rim in rims {
            let start = ranges.last().map_or(0, |range: &Range<usize>| range.end);
            ranges.push(start..start + rim.len());
        }
        let vertices = outline
            .iter()
            .map(|point| SpadePoint::new(point.x.value, point.y.value))
            .collect::<Vec<_>>();
        let edges = ranges
            .iter()
            .flat_map(|rim| {
                rim.clone()
                    .map(|from| [from, rim.start + (from + 1 - rim.start) % rim.len()])
            })
            .collect::<Vec<_>>();
        let mut crossing = None;
        let mut triangulation: ConstrainedDelaunayTriangulation<SpadePoint<f64>> =
            ConstrainedDelaunayTriangulation::try_bulk_load_cdt(vertices, edges, |edge| {
                crossing.get_or_insert(edge);
            })
            .map_err(|e| {
                OpossumError::Other(format!("the outline could not be triangulated: {e}"))
            })?;
        if let Some([from, to]) = crossing {
            return Err(OpossumError::Other(format!(
                "the rims of the outline cross between their points {from} and {to}, so what \
                 they enclose is undefined"
            )));
        }
        // A curved face lifted onto this mesh has to be sampled inside, not only along its rim, so
        // the triangles are held to the size the rim is already sampled at. The most coarsely
        // sampled rim sets that size; a finer one only makes its own surroundings finer.
        let mean_spacing = rims
            .iter()
            .map(|rim| ring_perimeter(rim) / to_f64(rim.len()))
            .fold(0.0, f64::max);
        let max_area = 3.0_f64.sqrt() / 4. * mean_spacing * mean_spacing;
        // A rim walked clockwise takes its area away: the sum is what the rims leave between them.
        let area = rims
            .iter()
            .map(|rim| doubled_signed_area(rim))
            .sum::<f64>()
            .abs()
            / 2.;
        let refinement = triangulation.refine(
            RefinementParameters::<f64>::default()
                .with_max_allowed_area(max_area)
                .with_max_additional_vertices(
                    try_f64_to_usize((REFINEMENT_HEADROOM * area / max_area).ceil())
                        .unwrap_or(usize::MAX)
                        .saturating_add(outline.len()),
                )
                // Without this the rim would be split and the outline points would no longer be
                // the boundary of the mesh, which is the one thing two faces have to agree on.
                .keep_constraint_edges()
                // Only the inside is refined; what lies between the rim and the convex hull is not
                // part of the area and is dropped below.
                .exclude_outer_faces(true),
        );
        if !refinement.refinement_complete {
            return Err(OpossumError::Other(
                "the area enclosed by the outline could not be filled with small enough triangles"
                    .into(),
            ));
        }
        let points = triangulation
            .vertices()
            .map(|vertex| {
                let position = vertex.position();
                Point2::new(meter!(position.x), meter!(position.y))
            })
            .collect::<Vec<_>>();
        // The outline was loaded first and the refinement only appends, so the outline points keep
        // the indices they went in with — which is what makes the rim a plain prefix at all.
        // Checked rather than assumed, so a change in `spade` cannot silently mislabel it.
        for (index, point) in outline.iter().enumerate() {
            if points.get(index) != Some(point) {
                return Err(OpossumError::Other(
                    "the triangulation moved the outline points away from their indices".into(),
                ));
            }
        }
        let in_area =
            |point: SpadePoint<f64>| inside(Point2::new(meter!(point.x), meter!(point.y)));
        Ok(Self {
            points,
            triangles: inner_triangles(&triangulation, &refinement.excluded_faces, in_area)?,
            rims: ranges,
        })
    }
    /// The points of the mesh, in the plane of the outline.
    ///
    /// The leading ones are the [`rims`](OutlineMesh::rims), rim after rim in the order each is
    /// walked; the rest fill the inside.
    #[must_use]
    pub fn points(&self) -> &[Point2<Length>] {
        &self.points
    }
    /// The triangles of the mesh as index triples into [`points`](OutlineMesh::points).
    ///
    /// Every triple is ordered counter-clockwise, seen from +z.
    #[must_use]
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
    /// The rims of the mesh, each as the range of the [`points`](OutlineMesh::points) it runs
    /// through.
    ///
    /// A rim is a closed edge of the area, walked with the area on its left. Anything that has to
    /// meet this mesh along its edge — the lateral face of a body, or a neighbouring face — walks
    /// these ranges instead of comparing coordinates.
    #[must_use]
    pub fn rims(&self) -> &[Range<usize>] {
        &self.rims
    }
}

/// Collect the triangles a refined triangulation covers the inside with.
///
/// What counts as inside is read off the rims, a bore being outside again. That is the area of the
/// outline only if its rims bound it, so the triangles are held against the outline itself (see
/// [`ROUNDING_SLIVERS`]).
///
/// # Arguments
///
/// - `triangulation`: the refined triangulation
/// - `excluded`: the faces the refinement marked as lying outside the constrained rims
/// - `in_area`: whether a point lies in the area of the outline
///
/// # Returns
///
/// The triangles as index triples, each wound counter-clockwise.
///
/// # Errors
///
/// This function returns an error if triangles inside the rims lie outside the area or the other
/// way round, or if the triangulation holds more points than can be indexed.
fn inner_triangles(
    triangulation: &ConstrainedDelaunayTriangulation<SpadePoint<f64>>,
    excluded: &[FixedFaceHandle<InnerTag>],
    in_area: impl Fn(SpadePoint<f64>) -> bool,
) -> OpmResult<Vec<[u32; 3]>> {
    let outside = excluded.iter().collect::<HashSet<_>>();
    let mut triangles = Vec::with_capacity(triangulation.num_inner_faces() - outside.len());
    // The area inside the rims, and the area the rims and the outline disagree about.
    let (mut covered, mut disputed) = (0.0, 0.0);
    for face in triangulation.inner_faces() {
        let outside_the_rims = outside.contains(&face.fix());
        if outside_the_rims == in_area(face.center()) {
            disputed += face.area();
        }
        if outside_the_rims {
            continue;
        }
        covered += face.area();
        let mut corners = [0_u32; 3];
        // `vertices` hands the corners over counter-clockwise, so the triangles come out wound the
        // same way as the outline they were built from.
        for (corner, vertex) in corners.iter_mut().zip(face.vertices()) {
            *corner = u32::try_from(vertex.fix().index()).map_err(|_| {
                OpossumError::Other("the mesh has more points than can be indexed".into())
            })?;
        }
        triangles.push(corners);
    }
    if disputed > ROUNDING_SLIVERS * covered {
        return Err(OpossumError::Other(
            "the rims of the outline do not bound what it encloses".into(),
        ));
    }
    Ok(triangles)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Bore, CircleShape, Outline, PolygonShape, RectangleShape},
        degree, millimeter,
        utils::test_helper::helper::l_shape_corners,
    };
    use approx::{assert_abs_diff_eq, assert_relative_eq};
    use std::f64::consts::PI;

    /// How finely the test outlines are sampled along their rim.
    const SEGMENTS: usize = 48;

    /// The three corners of a mesh triangle.
    fn corners(mesh: &OutlineMesh, triangle: &[u32; 3]) -> [Point2<Length>; 3] {
        triangle.map(|index| mesh.points()[usize::try_from(index).unwrap()])
    }

    /// A five-pointed star, to cover an outline that dents inwards five times over.
    fn star() -> Vec<Point2<Length>> {
        (0..10)
            .map(|corner| {
                let angle = to_f64(corner) * PI / 5.;
                let radius = if corner % 2 == 0 { 10.0 } else { 4.0 };
                millimeter!(radius * angle.cos(), radius * angle.sin())
            })
            .collect()
    }

    /// One outline of every shape, convex and not, with and without bores, one of them turned.
    fn outlines() -> OpmResult<Vec<(&'static str, Outline)>> {
        Ok(vec![
            (
                "circle",
                Outline::new(CircleShape::new(millimeter!(12.5))?, Vec::new())?,
            ),
            (
                "rectangle",
                Outline::new(
                    RectangleShape::new(millimeter!(30.0), millimeter!(10.0))?,
                    Vec::new(),
                )?,
            ),
            (
                "L polygon",
                Outline::new(PolygonShape::new(l_shape_corners())?, Vec::new())?,
            ),
            (
                "star polygon",
                Outline::new(PolygonShape::new(star())?, Vec::new())?,
            ),
            (
                "ring",
                Outline::new(
                    CircleShape::new(millimeter!(12.5))?,
                    vec![Bore::new(
                        CircleShape::new(millimeter!(4.0))?,
                        millimeter!(0.0, 0.0),
                        degree!(0.0),
                    )],
                )?,
            ),
            (
                "plate with two bores",
                Outline::new(
                    RectangleShape::new(millimeter!(30.0), millimeter!(12.0))?,
                    vec![
                        Bore::new(
                            CircleShape::new(millimeter!(3.0))?,
                            millimeter!(-8.0, 0.0),
                            degree!(0.0),
                        ),
                        Bore::new(
                            RectangleShape::new(millimeter!(4.0), millimeter!(3.0))?,
                            millimeter!(8.0, 1.0),
                            degree!(30.0),
                        ),
                    ],
                )?,
            ),
        ])
    }

    /// The triangles have to cover exactly what the rims enclose. Comparing their total area with
    /// the area the rims leave between them catches gaps and overlap at once, and asking the
    /// outline about every centroid catches a triangle that strayed into a dent or a bore.
    #[test]
    fn the_mesh_covers_exactly_what_its_rims_enclose() -> OpmResult<()> {
        for (name, outline) in outlines()? {
            let mesh = outline.triangulate(SEGMENTS)?;
            // A rim around a bore runs clockwise and takes its area away.
            let enclosed = mesh
                .rims()
                .iter()
                .map(|rim| doubled_signed_area(&mesh.points()[rim.clone()]))
                .sum::<f64>();
            let covered = mesh
                .triangles()
                .iter()
                .map(|triangle| doubled_signed_area(&corners(&mesh, triangle)))
                .sum::<f64>();
            assert_relative_eq!(covered, enclosed, max_relative = 1e-12);
            for triangle in mesh.triangles() {
                let corners = corners(&mesh, triangle);
                assert!(
                    doubled_signed_area(&corners) > 0.0,
                    "a triangle of the {name} is not wound counter-clockwise"
                );
                let centroid = Point2::new(
                    (corners[0].x + corners[1].x + corners[2].x) / 3.,
                    (corners[0].y + corners[1].y + corners[2].y) / 3.,
                );
                assert!(
                    outline.contains(centroid),
                    "a triangle of the {name} lies outside it, centered on {centroid:?}"
                );
            }
        }
        Ok(())
    }

    /// The whole point of filling the inside is that a curved face laid onto the mesh is sampled
    /// there too, so no triangle may be coarser than the rim already is.
    ///
    /// Away from the rim that holds outright. Triangles *touching* the rim are the exception: the
    /// rim is held fixed so that two faces can be joined along it, which leaves the refinement
    /// nowhere to put a point that would encroach on it. Those are allowed to come out larger, but
    /// only by a bounded margin.
    #[test]
    fn no_triangle_is_larger_than_allowed() -> OpmResult<()> {
        for (name, outline) in outlines()? {
            let mesh = outline.triangulate(SEGMENTS)?;
            // The coarsest rim is the one the triangles are held to.
            let mean_spacing = mesh
                .rims()
                .iter()
                .map(|rim| ring_perimeter(&mesh.points()[rim.clone()]) / to_f64(rim.len()))
                .fold(0.0, f64::max);
            let max_area = 3.0_f64.sqrt() / 4. * mean_spacing * mean_spacing;
            for triangle in mesh.triangles() {
                let area = doubled_signed_area(&corners(&mesh, triangle)) / 2.;
                let touches_rim = triangle.iter().any(|index| {
                    mesh.rims()
                        .iter()
                        .any(|rim| rim.contains(&(*index as usize)))
                });
                let allowed = if touches_rim { 2. } else { 1. } * max_area;
                assert!(
                    area <= allowed,
                    "a triangle of the {name} covers {area} m², more than the {allowed} m² allowed \
                     {} the rim",
                    if touches_rim { "at" } else { "away from" }
                );
            }
        }
        Ok(())
    }

    /// Two faces meshed over the same outline are joined along their rims, and the lateral face
    /// between them faces away from whatever lies to the left of a rim. So every rim has to run
    /// along the edge of the area with the area on its left: stepping off the middle of each of its
    /// segments to the left lands in the outline, to the right outside it.
    #[test]
    fn every_rim_runs_along_the_edge_with_the_area_on_its_left() -> OpmResult<()> {
        // Further than a circle bulges out between two points of its rim, and less than any of the
        // shapes is wide.
        let step = millimeter!(0.1);
        for (name, outline) in outlines()? {
            let mesh = outline.triangulate(SEGMENTS)?;
            let mut rim_points = 0;
            for rim in mesh.rims() {
                assert_eq!(
                    rim.start, rim_points,
                    "the rims of the {name} do not come first"
                );
                rim_points = rim.end;
                let rim = &mesh.points()[rim.clone()];
                for (from, to) in rim.iter().zip(rim.iter().cycle().skip(1)) {
                    let along = (to - from).map(|c| c.value).normalize();
                    let beside = |side: f64| {
                        Point2::new(
                            (from.x + to.x) / 2. - step * (side * along.y),
                            (from.y + to.y) / 2. + step * (side * along.x),
                        )
                    };
                    assert!(
                        outline.contains(beside(1.0)),
                        "left of a rim of the {name} there is no area, at {from:?}"
                    );
                    assert!(
                        !outline.contains(beside(-1.0)),
                        "right of a rim of the {name} there is area, at {from:?}"
                    );
                }
            }
            assert!(
                mesh.points().len() > rim_points,
                "the {name} got no points inside it at all"
            );
            // An outline without bores has one rim, and that is its edge point for point.
            if outline.bores().is_empty() {
                assert_eq!(
                    mesh.points()[..rim_points],
                    outline.shape().outline_points(SEGMENTS)?,
                    "the rim of the {name}"
                );
            }
        }
        Ok(())
    }

    /// Two optics of the same kind share one mesh when they are exported, which only works if the
    /// same outline always triangulates the same way.
    #[test]
    fn the_same_outline_yields_the_same_mesh() -> OpmResult<()> {
        for ((_, one), (name, other)) in outlines()?.into_iter().zip(outlines()?) {
            assert_eq!(
                one.triangulate(SEGMENTS)?,
                other.triangulate(SEGMENTS)?,
                "two equal {name}s were triangulated differently"
            );
        }
        Ok(())
    }

    /// Rims that cross or visit a point twice do not separate an inside from an outside, so there
    /// is nothing to fill.
    #[test]
    fn rims_that_do_not_bound_an_area_cannot_be_triangulated() {
        let bowtie = vec![
            millimeter!(0.0, 0.0),
            millimeter!(3.0, 3.0),
            millimeter!(3.0, 0.0),
            millimeter!(0.0, 4.0),
        ];
        assert!(OutlineMesh::from_rims(&[bowtie], |_| true).is_err());
        let square = vec![
            millimeter!(0.0, 0.0),
            millimeter!(1.0, 0.0),
            millimeter!(1.0, 1.0),
            millimeter!(0.0, 1.0),
        ];
        assert!(OutlineMesh::from_rims(&[square.clone(), square], |_| true).is_err());
    }

    /// The mesh of a circle approaches its area from below as the rim is sampled more finely — a
    /// check that the triangles really tile the disc and not some other region.
    #[test]
    fn a_finer_circle_covers_more_of_its_disc() -> OpmResult<()> {
        let radius = millimeter!(12.5);
        let outline = Outline::new(CircleShape::new(radius)?, Vec::new())?;
        let covered = |segments: usize| -> OpmResult<f64> {
            let mesh = outline.triangulate(segments)?;
            Ok(mesh
                .triangles()
                .iter()
                .map(|triangle| doubled_signed_area(&corners(&mesh, triangle)) / 2.)
                .sum::<f64>())
        };
        let disc = PI * radius.value * radius.value;
        let (coarse, fine) = (covered(16)?, covered(256)?);
        assert!(coarse < fine && fine < disc);
        assert_abs_diff_eq!(fine, disc, epsilon = disc * 1e-3);
        Ok(())
    }
}
