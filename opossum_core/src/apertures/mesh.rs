//! Triangulation of the area an [`Aperture`] encloses.

use super::{Aperture, doubled_signed_area, ring_perimeter};
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
use std::collections::HashSet;
use uom::si::f64::Length;

/// A triangulation of the area an [`Aperture`] encloses, in the aperture's own plane.
///
/// Where [`Aperture::outline_points`] gives only the rim, this fills what the rim encloses:
/// triangles covering the whole area, with points inside it and not just along its edge. That is
/// what a curved surface needs to be laid onto — sampled at the rim alone it would be flat.
///
/// The points of the outline come first, so a body built from two such surfaces can join them along
/// exactly those points; see [`rim_len`](ApertureMesh::rim_len).
#[derive(Debug, Clone, PartialEq)]
pub struct ApertureMesh {
    points: Vec<Point2<Length>>,
    triangles: Vec<[u32; 3]>,
    rim_len: usize,
}

impl ApertureMesh {
    /// The points of the mesh, in the plane of the aperture.
    ///
    /// The first [`rim_len`](ApertureMesh::rim_len) of them are the rim, in the order the outline
    /// walks it; the rest fill the inside.
    #[must_use]
    pub fn points(&self) -> &[Point2<Length>] {
        &self.points
    }
    /// The triangles of the mesh as index triples into [`points`](ApertureMesh::points).
    ///
    /// Every triple is ordered counter-clockwise, seen from +z.
    #[must_use]
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
    /// How many of the leading [`points`](ApertureMesh::points) came from the outline.
    ///
    /// These are exactly the points [`Aperture::outline_points`] returned, in the same order.
    /// Anything that has to meet this mesh along its edge — the lateral face of a body, or a
    /// neighbouring surface — walks that prefix instead of comparing coordinates.
    #[must_use]
    pub const fn rim_len(&self) -> usize {
        self.rim_len
    }
}

/// How much room the refinement gets beyond the points it is expected to need.
///
/// Refinement can fail to terminate, so it has to be capped; the cap must still be generous enough
/// that a mesh which is merely awkward does not hit it. See
/// [`RefinementParameters::with_max_additional_vertices`].
const REFINEMENT_HEADROOM: f64 = 4.0;

impl Aperture {
    /// Triangulate the area this [`Aperture`] encloses.
    ///
    /// The rim comes from [`outline_points`](Aperture::outline_points) and is held fixed; the
    /// inside is then filled with further points until no triangle is larger than an equilateral
    /// one whose side is the mean spacing of the outline points. Holding the rim fixed is what lets
    /// two surfaces meshed over the same aperture be joined along it afterwards — at the price that
    /// triangles touching the rim may come out somewhat larger than that, since a point dividing
    /// them would have to encroach on the rim.
    ///
    /// The result is reproducible: the same aperture and the same `segments` always yield the same
    /// mesh, which is what lets two identical optics share one mesh when they are exported.
    ///
    /// # Arguments
    ///
    /// - `segments`: the number of outline points to aim for, at least 3
    ///
    /// # Returns
    ///
    /// The triangulated area, with its points in the plane of the aperture.
    ///
    /// # Errors
    ///
    /// This function returns an error if the aperture does not enclose a region at all (see
    /// [`is_geometric_bound`](Aperture::is_geometric_bound)), if
    /// [`outline_points`](Aperture::outline_points) fails, if the outline crosses itself or visits
    /// a point twice, or if the area could not be filled within the point budget.
    pub fn triangulate(&self, segments: usize) -> OpmResult<ApertureMesh> {
        if !self.is_geometric_bound() {
            return Err(OpossumError::Other(format!(
                "a {} aperture of shape '{}' encloses no area to triangulate",
                self.aperture_type(),
                self.shape()
            )));
        }
        let outline = self.distinct_outline(segments)?;
        let vertices = outline
            .iter()
            .map(|point| SpadePoint::new(point.x.value, point.y.value))
            .collect::<Vec<_>>();
        let edges = (0..outline.len())
            .map(|from| [from, (from + 1) % outline.len()])
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
                "the outline crosses itself between its points {from} and {to}, so what it \
                 encloses is undefined"
            )));
        }
        // A curved surface lifted onto this mesh has to be sampled inside, not only along its rim,
        // so the triangles are held to the size the rim is already sampled at.
        let mean_spacing = ring_perimeter(&outline) / to_f64(outline.len());
        let max_area = 3.0_f64.sqrt() / 4. * mean_spacing * mean_spacing;
        let area = doubled_signed_area(&outline).abs() / 2.;
        let refinement = triangulation.refine(
            RefinementParameters::<f64>::default()
                .with_max_allowed_area(max_area)
                .with_max_additional_vertices(
                    try_f64_to_usize((REFINEMENT_HEADROOM * area / max_area).ceil())
                        .unwrap_or(usize::MAX)
                        .saturating_add(outline.len()),
                )
                // Without this the rim would be split and the outline points would no longer be
                // the boundary of the mesh, which is the one thing two surfaces have to agree on.
                .keep_constraint_edges()
                // Only the inside is refined; what lies between the rim and the convex hull is not
                // part of the aperture and is dropped below.
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
        Ok(ApertureMesh {
            points,
            triangles: inner_triangles(&triangulation, &refinement.excluded_faces)?,
            rim_len: outline.len(),
        })
    }
    /// The outline of this aperture, checked to visit no point twice.
    ///
    /// `spade` removes duplicate points before it triangulates, which would shift every index
    /// behind the duplicate and quietly mislabel the rim. An outline visiting a point twice pinches
    /// the area off there anyway.
    ///
    /// # Arguments
    ///
    /// - `segments`: the number of outline points to aim for
    ///
    /// # Returns
    ///
    /// The outline points, all distinct.
    ///
    /// # Errors
    ///
    /// This function returns an error if the outline cannot be walked, or if it visits a point
    /// twice.
    fn distinct_outline(&self, segments: usize) -> OpmResult<Vec<Point2<Length>>> {
        let outline = self.outline_points(segments)?;
        let mut seen = HashSet::with_capacity(outline.len());
        for point in &outline {
            if !seen.insert((point.x.value.to_bits(), point.y.value.to_bits())) {
                return Err(OpossumError::Other(
                    "the outline visits the same point twice and does not enclose an area".into(),
                ));
            }
        }
        Ok(outline)
    }
}

/// Collect the triangles a refined triangulation covers the inside with.
///
/// # Arguments
///
/// - `triangulation`: the refined triangulation
/// - `excluded`: the faces the refinement marked as lying outside the constrained outline
///
/// # Returns
///
/// The triangles as index triples, each wound counter-clockwise.
///
/// # Errors
///
/// This function returns an error if the triangulation holds more points than can be indexed.
fn inner_triangles(
    triangulation: &ConstrainedDelaunayTriangulation<SpadePoint<f64>>,
    excluded: &[FixedFaceHandle<InnerTag>],
) -> OpmResult<Vec<[u32; 3]>> {
    let outside = excluded.iter().collect::<HashSet<_>>();
    let mut triangles = Vec::with_capacity(triangulation.num_inner_faces() - outside.len());
    for face in triangulation.inner_faces() {
        if outside.contains(&face.fix()) {
            continue;
        }
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
    Ok(triangles)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::ApertureType, degree, millimeter,
        utils::test_helper::test_helper::l_shape_corners,
    };
    use approx::{assert_abs_diff_eq, assert_relative_eq};
    use nalgebra::Point3;
    use num_traits::Zero;
    use std::f64::consts::PI;

    /// How finely the test apertures are sampled along their rim.
    const SEGMENTS: usize = 48;

    /// The three corners of a mesh triangle.
    fn corners(mesh: &ApertureMesh, triangle: &[u32; 3]) -> [Point2<Length>; 3] {
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

    /// One aperture of every shape that can bound a region, convex and not, placed and turned.
    fn apertures_that_bound_a_region() -> OpmResult<Vec<(&'static str, Aperture)>> {
        Ok(vec![
            (
                "circle",
                Aperture::new_circle(millimeter!(12.5), ApertureType::Hole, None)?,
            ),
            (
                "rectangle",
                Aperture::new_rectangle(
                    millimeter!(30.0),
                    millimeter!(10.0),
                    ApertureType::Hole,
                    None,
                    None,
                )?,
            ),
            (
                "L polygon",
                Aperture::new_polygon(l_shape_corners(), ApertureType::Hole, None, None)?,
            ),
            (
                "star polygon",
                Aperture::new_polygon(star(), ApertureType::Hole, None, None)?,
            ),
            (
                "shifted and turned star",
                Aperture::new_polygon(
                    star(),
                    ApertureType::Hole,
                    Some(millimeter!(7.0, -4.0)),
                    Some(degree!(23.0)),
                )?,
            ),
        ])
    }

    /// The triangles have to cover exactly what the outline encloses. Comparing their total area
    /// with the area of the outline catches gaps and overlap at once, and asking the aperture about
    /// every centroid catches a triangle that strayed across a dent into the outside.
    #[test]
    fn the_mesh_covers_exactly_what_the_outline_encloses() -> OpmResult<()> {
        for (name, aperture) in apertures_that_bound_a_region()? {
            let outline = aperture.outline_points(SEGMENTS)?;
            let mesh = aperture.triangulate(SEGMENTS)?;
            let covered = mesh
                .triangles()
                .iter()
                .map(|triangle| doubled_signed_area(&corners(&mesh, triangle)))
                .sum::<f64>();
            assert_relative_eq!(covered, doubled_signed_area(&outline), max_relative = 1e-12);
            for triangle in mesh.triangles() {
                let corners = corners(&mesh, triangle);
                assert!(
                    doubled_signed_area(&corners) > 0.0,
                    "a triangle of the {name} is not wound counter-clockwise"
                );
                let centroid = Point3::new(
                    (corners[0].x + corners[1].x + corners[2].x) / 3.,
                    (corners[0].y + corners[1].y + corners[2].y) / 3.,
                    Length::zero(),
                );
                assert!(
                    aperture.apodize(&centroid) > 0.0,
                    "a triangle of the {name} lies outside it, centered on {centroid:?}"
                );
            }
        }
        Ok(())
    }

    /// The whole point of filling the inside is that a curved surface laid onto the mesh is sampled
    /// there too, so no triangle may be coarser than the rim already is.
    ///
    /// Away from the rim that holds outright. Triangles *touching* the rim are the exception: the
    /// rim is held fixed so that two surfaces can be joined along it, which leaves the refinement
    /// nowhere to put a point that would encroach on it. Those are allowed to come out larger, but
    /// only by a bounded margin — one growing without limit would mean the rim itself was sampled
    /// too coarsely.
    #[test]
    fn no_triangle_is_larger_than_allowed() -> OpmResult<()> {
        for (name, aperture) in apertures_that_bound_a_region()? {
            let outline = aperture.outline_points(SEGMENTS)?;
            let mesh = aperture.triangulate(SEGMENTS)?;
            let mean_spacing = ring_perimeter(&outline) / to_f64(outline.len());
            let max_area = 3.0_f64.sqrt() / 4. * mean_spacing * mean_spacing;
            for triangle in mesh.triangles() {
                let area = doubled_signed_area(&corners(&mesh, triangle)) / 2.;
                let touches_rim = triangle
                    .iter()
                    .any(|index| (*index as usize) < mesh.rim_len());
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

    /// Two surfaces meshed over the same aperture are joined along their rims, so the rim has to be
    /// exactly the outline — in the same places and in the same order.
    #[test]
    fn the_rim_is_the_outline() -> OpmResult<()> {
        for (name, aperture) in apertures_that_bound_a_region()? {
            let outline = aperture.outline_points(SEGMENTS)?;
            let mesh = aperture.triangulate(SEGMENTS)?;
            assert_eq!(mesh.rim_len(), outline.len());
            assert_eq!(
                &mesh.points()[..mesh.rim_len()],
                outline.as_slice(),
                "the rim of the {name} does not follow its outline"
            );
            assert!(
                mesh.points().len() > outline.len(),
                "the {name} got no points inside it at all"
            );
        }
        Ok(())
    }

    /// Two optics of the same kind share one mesh when they are exported, which only works if the
    /// same aperture always triangulates the same way.
    #[test]
    fn the_same_aperture_yields_the_same_mesh() -> OpmResult<()> {
        for ((_, one), (name, other)) in apertures_that_bound_a_region()?
            .into_iter()
            .zip(apertures_that_bound_a_region()?)
        {
            // Two separately built but equal apertures, which subsumes asking the same one twice.
            assert_eq!(
                one.triangulate(SEGMENTS)?,
                other.triangulate(SEGMENTS)?,
                "two equal {name}s were triangulated differently"
            );
        }
        Ok(())
    }

    /// An outline crossing itself does not separate an inside from an outside, so there is nothing
    /// to fill.
    #[test]
    fn a_self_crossing_outline_cannot_be_triangulated() -> OpmResult<()> {
        let bowtie = Aperture::new_polygon(
            vec![
                millimeter!(0.0, 0.0),
                millimeter!(3.0, 3.0),
                millimeter!(3.0, 0.0),
                millimeter!(0.0, 4.0),
            ],
            ApertureType::Hole,
            None,
            None,
        )?;
        assert!(bowtie.triangulate(SEGMENTS).is_err());
        Ok(())
    }

    #[test]
    fn an_aperture_that_bounds_no_region_cannot_be_triangulated() -> OpmResult<()> {
        // What an obstruction lets through is the unbounded outside of its shape.
        let obstruction = Aperture::new_circle(millimeter!(5.0), ApertureType::Obstruction, None)?;
        assert!(obstruction.triangulate(SEGMENTS).is_err());
        for shape in crate::apertures::ApertureShape::non_delimiting() {
            let aperture = Aperture::new(shape.clone(), ApertureType::Hole, None, None)?;
            assert!(
                aperture.triangulate(SEGMENTS).is_err(),
                "'{shape}' has no edge, so there is no area to fill"
            );
        }
        Ok(())
    }

    /// The mesh of a circle approaches its area from below as the rim is sampled more finely — a
    /// check that the triangles really tile the disc and not some other region.
    #[test]
    fn a_finer_circle_covers_more_of_its_disc() -> OpmResult<()> {
        let radius = millimeter!(12.5);
        let aperture = Aperture::new_circle(radius, ApertureType::Hole, None)?;
        let covered = |segments: usize| -> OpmResult<f64> {
            let mesh = aperture.triangulate(segments)?;
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
