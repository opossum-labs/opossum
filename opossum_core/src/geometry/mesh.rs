//! Triangle meshes of geometric surfaces.

use super::geo_surface::{GeoSurfaceRef, SurfacePlacement};
use crate::{
    apertures::ApertureMesh,
    error::OpmResult,
    utils::{LockExt, geom_transformation::Isometry},
};
use nalgebra::{Point3, Vector3};
use uom::si::f64::Length;

/// A triangle mesh of one piece of surface, with a normal at every point.
///
/// Usually that piece is a [`GeoSurface`](super::geo_surface::GeoSurface) laid over an aperture;
/// the lateral face closing a body between two of them is the one exception.
///
/// The mesh lives in the frame it was built in — that of an optical node — and not in global
/// coordinates. Moving the node therefore leaves it untouched: only the node's own placement has to
/// be passed on again, which is what lets a changed setup be redrawn without recomputing any
/// geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceMesh {
    points: Vec<Point3<Length>>,
    normals: Vec<Vector3<f64>>,
    triangles: Vec<[u32; 3]>,
}

impl SurfaceMesh {
    /// Assemble a mesh from points, the normal at each of them, and the triangles between them.
    ///
    /// Only the lateral face closing a body is built this way — it belongs to no
    /// [`GeoSurface`](super::geo_surface::GeoSurface) and has no aperture to be laid over. Every
    /// other mesh comes from [`GeoSurfaceRef::mesh_over`], which fills these in itself.
    pub(super) const fn new(
        points: Vec<Point3<Length>>,
        normals: Vec<Vector3<f64>>,
        triangles: Vec<[u32; 3]>,
    ) -> Self {
        Self {
            points,
            normals,
            triangles,
        }
    }
    /// The points of the mesh, in the frame it was built in.
    ///
    /// Point `i` lies above point `i` of the [`ApertureMesh`] the surface was meshed over, so the
    /// aperture's [`rim_len`](ApertureMesh::rim_len) delimits the rim of this mesh as well.
    #[must_use]
    pub fn points(&self) -> &[Point3<Length>] {
        &self.points
    }
    /// The unit normal at every point, in the same order and the same frame.
    #[must_use]
    pub fn normals(&self) -> &[Vector3<f64>] {
        &self.normals
    }
    /// The triangles of the mesh as index triples into [`points`](SurfaceMesh::points).
    ///
    /// Every triple is ordered counter-clockwise when seen from the side the normals point to.
    #[must_use]
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.triangles
    }
    /// Turn the mesh around, so that it faces the other way.
    ///
    /// A body is bounded from below by one surface and from above by another, and what is outside
    /// is on opposite sides of the two. Since a surface is always meshed facing +z, one of them has
    /// to be turned over. Both the normals and the winding of the triangles are reversed, so the
    /// two keep agreeing on which side is out.
    #[must_use]
    pub fn flipped(mut self) -> Self {
        for normal in &mut self.normals {
            *normal = -*normal;
        }
        for triangle in &mut self.triangles {
            triangle.swap(1, 2);
        }
        self
    }
}

impl GeoSurfaceRef {
    /// Lay a flat [`ApertureMesh`] onto this surface.
    ///
    /// The aperture states *where* to sample — a triangulation of the area the surface is wanted
    /// over — and the surface answers where it lies above each of those positions and which way it
    /// faces there. Splitting it this way is what lets a lens mesh both of its surfaces over one
    /// aperture, and what will let a detector be drawn as a single surface over its own.
    ///
    /// Points and triangles come out in the order the aperture mesh had them, so
    /// [`ApertureMesh::rim_len`] delimits the rim of the result too.
    ///
    /// # Arguments
    ///
    /// - `aperture_mesh`: the triangulated area to lay onto the surface, in the xy plane of `frame`
    /// - `frame`: the frame to express the result in, usually the placement of an optical node
    ///
    /// # Returns
    ///
    /// The meshed surface, with all normals facing the +z of `frame`.
    ///
    /// # Errors
    ///
    /// This function returns an error if the surface cannot be locked, if it runs parallel to the
    /// frame's axis, if it does not reach as far out as the aperture, or if it is both curved and
    /// tilted against the frame.
    // The lock cannot be released any earlier: the placement borrows the surface for as long as it
    // lives, so the suggested `drop` does not compile.
    #[allow(clippy::significant_drop_tightening)]
    pub fn mesh_over(
        &self,
        aperture_mesh: &ApertureMesh,
        frame: &Isometry,
    ) -> OpmResult<SurfaceMesh> {
        // Everything the surface itself has to answer happens inside this block, so its lock is
        // released again before the mesh is assembled.
        let (points, normals) = {
            let surface = self.0.lock_opm()?;
            let placement = SurfacePlacement::new(&*surface, frame)?;
            let mut points = Vec::with_capacity(aperture_mesh.points().len());
            let mut normals = Vec::with_capacity(aperture_mesh.points().len());
            for transversal_position in aperture_mesh.points() {
                let (point, normal) = placement.above(transversal_position)?;
                points.push(point);
                normals.push(normal);
            }
            (points, normals)
        };
        Ok(SurfaceMesh {
            points,
            normals,
            triangles: aperture_mesh.triangles().to_vec(),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Aperture, ApertureType},
        degree,
        geometry::{Cylinder, Parabola, Plane, Sphere, geo_surface::GeoSurface},
        millimeter,
        utils::test_helper::test_helper::{placed, somewhere_else},
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::Point2;
    use std::sync::{Arc, Mutex};

    const SEGMENTS: usize = 24;

    fn clear_aperture(radius: f64) -> ApertureMesh {
        Aperture::new_circle(millimeter!(radius), ApertureType::Hole, None)
            .unwrap()
            .triangulate(SEGMENTS)
            .unwrap()
    }

    /// Place a surface turned against its node, the way a wedge does with its exit face.
    fn tilted<S: GeoSurface + 'static>(
        mut surface: S,
        node: &Isometry,
    ) -> OpmResult<GeoSurfaceRef> {
        surface.set_isometry(node.append(&Isometry::new(
            millimeter!(0.0, 0.0, 3.0),
            degree!(0.0, 8.0, 0.0),
        )?));
        Ok(GeoSurfaceRef(Arc::new(Mutex::new(surface))))
    }

    /// One of every surface a volume node builds, each sitting at its own offset within the node.
    fn surfaces_of_a_node(node: &Isometry) -> OpmResult<Vec<(&'static str, GeoSurfaceRef)>> {
        Ok(vec![
            ("plane", placed(Plane::default(), node, millimeter!(0.0))?),
            (
                "convex sphere",
                placed(
                    Sphere::new(millimeter!(50.0), Isometry::identity())?,
                    node,
                    millimeter!(0.0),
                )?,
            ),
            (
                "concave sphere",
                placed(
                    Sphere::new(millimeter!(-50.0), Isometry::identity())?,
                    node,
                    millimeter!(4.0),
                )?,
            ),
            (
                "cylinder",
                placed(
                    Cylinder::new(millimeter!(40.0), Isometry::identity())?,
                    node,
                    millimeter!(2.0),
                )?,
            ),
            (
                "parabola",
                placed(
                    Parabola::new(millimeter!(60.0), Isometry::identity())?,
                    node,
                    millimeter!(0.0),
                )?,
            ),
            ("tilted plane", tilted(Plane::default(), node)?),
        ])
    }

    /// Check everything [`GeoSurfaceRef::mesh_over`] promises.
    // The lock is held for as long as the surface is being asked about every meshed point at once.
    #[allow(clippy::significant_drop_tightening)]
    fn check_mesh_contract(
        name: &str,
        surface: &GeoSurfaceRef,
        frame: &Isometry,
        aperture: &ApertureMesh,
    ) -> OpmResult<()> {
        let mesh = surface.mesh_over(aperture, frame)?;
        assert_eq!(mesh.points().len(), aperture.points().len());
        assert_eq!(mesh.normals().len(), aperture.points().len());
        assert_eq!(mesh.triangles(), aperture.triangles());
        // Where the surface itself says it lies above each meshed point — asked through its own
        // placement rather than the one the mesh was built from, so this is an independent answer.
        let on_surface = {
            let surface = surface.0.lock_opm()?;
            let surface_iso = *surface.isometry();
            mesh.points()
                .iter()
                .map(|point| {
                    let local = surface_iso.inverse_transform_point(&frame.transform_point(point));
                    let sag = surface
                        .local_z_at(&Point2::new(local.x, local.y))
                        .expect("the surface reaches a point it was meshed at");
                    (local.z.value, sag.value)
                })
                .collect::<Vec<_>>()
        };
        for (meshed_at, surface_at) in on_surface {
            assert_abs_diff_eq!(meshed_at, surface_at, epsilon = 1e-12);
        }
        for normal in mesh.normals() {
            assert_abs_diff_eq!(normal.norm(), 1.0, epsilon = 1e-12);
            assert!(
                normal.z > 0.0,
                "a normal of the {name} does not face the +z of its frame: {normal:?}"
            );
        }
        for triangle in mesh.triangles() {
            let corner =
                |index: u32| mesh.points()[usize::try_from(index).unwrap()].map(|c| c.value);
            let [first, second, third] = [
                corner(triangle[0]),
                corner(triangle[1]),
                corner(triangle[2]),
            ];
            let normal = |index: u32| mesh.normals()[usize::try_from(index).unwrap()];
            let facing = (second - first).cross(&(third - first));
            let mean_normal =
                (normal(triangle[0]) + normal(triangle[1]) + normal(triangle[2])) / 3.;
            assert!(
                facing.dot(&mean_normal) > 0.0,
                "a triangle of the {name} is wound against the normals of its corners"
            );
        }
        Ok(())
    }

    #[test]
    fn a_meshed_surface_follows_the_contract() -> OpmResult<()> {
        let aperture = clear_aperture(10.0);
        for node in [Isometry::identity(), somewhere_else()?] {
            for (name, surface) in surfaces_of_a_node(&node)? {
                check_mesh_contract(name, &surface, &node, &aperture)?;
            }
        }
        Ok(())
    }

    /// The mesh is built in the node's own frame, so moving the node leaves it alone. That is what
    /// lets a changed setup be redrawn by resending placements instead of geometry.
    #[test]
    fn the_mesh_does_not_move_with_the_node() -> OpmResult<()> {
        let aperture = clear_aperture(10.0);
        let (here, there) = (Isometry::identity(), somewhere_else()?);
        for ((name, surface), (_, moved)) in surfaces_of_a_node(&here)?
            .into_iter()
            .zip(surfaces_of_a_node(&there)?)
        {
            let at_origin = surface.mesh_over(&aperture, &here)?;
            let elsewhere = moved.mesh_over(&aperture, &there)?;
            assert_eq!(at_origin.triangles(), elsewhere.triangles());
            for (before, after) in at_origin.points().iter().zip(elsewhere.points()) {
                for axis in 0..3 {
                    assert_abs_diff_eq!(before[axis].value, after[axis].value, epsilon = 1e-12);
                }
            }
            for (before, after) in at_origin.normals().iter().zip(elsewhere.normals()) {
                assert_abs_diff_eq!((before - after).norm(), 0.0, epsilon = 1e-12);
            }
            assert!(
                !at_origin.points().is_empty(),
                "the {name} was not meshed at all"
            );
        }
        Ok(())
    }

    #[test]
    fn flipping_turns_normals_and_winding_together() -> OpmResult<()> {
        let node = Isometry::identity();
        let surface = placed(
            Sphere::new(millimeter!(50.0), Isometry::identity())?,
            &node,
            millimeter!(0.0),
        )?;
        let mesh = surface.mesh_over(&clear_aperture(10.0), &node)?;
        let flipped = mesh.clone().flipped();
        assert_eq!(flipped.points(), mesh.points());
        for (before, after) in mesh.normals().iter().zip(flipped.normals()) {
            assert_eq!(*after, -before);
        }
        for (before, after) in mesh.triangles().iter().zip(flipped.triangles()) {
            assert_eq!(*after, [before[0], before[2], before[1]]);
        }
        assert_eq!(
            flipped.flipped(),
            mesh,
            "turning it over twice turns it back"
        );
        Ok(())
    }

    /// A sphere ends where its radius of curvature does. Asked beyond that it has no answer, and a
    /// mesh reaching further out must not be filled in with something made up.
    #[test]
    fn a_surface_too_small_for_the_aperture_cannot_be_meshed() -> OpmResult<()> {
        let node = Isometry::identity();
        let surface = placed(
            Sphere::new(millimeter!(5.0), Isometry::identity())?,
            &node,
            millimeter!(0.0),
        )?;
        assert!(surface.mesh_over(&clear_aperture(10.0), &node).is_err());
        assert!(surface.mesh_over(&clear_aperture(2.0), &node).is_ok());
        Ok(())
    }

    /// A surface standing on edge never crosses the frame's axis, so there is no point "above" any
    /// transversal position to mesh.
    #[test]
    fn a_surface_parallel_to_the_frame_axis_cannot_be_meshed() -> OpmResult<()> {
        let node = Isometry::identity();
        let mut on_edge = Plane::default();
        on_edge.set_isometry(Isometry::new(
            millimeter!(0.0, 0.0, 0.0),
            degree!(0.0, 90.0, 0.0),
        )?);
        assert!(
            GeoSurfaceRef(Arc::new(Mutex::new(on_edge)))
                .mesh_over(&clear_aperture(10.0), &node)
                .is_err()
        );
        Ok(())
    }

    /// Where a curved surface tilted against its frame is met cannot be worked out by walking along
    /// the frame's axis, so it is turned away — the same limit the extent of a body is subject to.
    #[test]
    fn a_curved_surface_tilted_against_the_frame_cannot_be_meshed() -> OpmResult<()> {
        let node = Isometry::identity();
        for surface in [
            tilted(Sphere::new(millimeter!(50.0), Isometry::identity())?, &node)?,
            tilted(
                Cylinder::new(millimeter!(40.0), Isometry::identity())?,
                &node,
            )?,
            tilted(
                Parabola::new(millimeter!(60.0), Isometry::identity())?,
                &node,
            )?,
        ] {
            assert!(surface.mesh_over(&clear_aperture(10.0), &node).is_err());
        }
        Ok(())
    }
}
