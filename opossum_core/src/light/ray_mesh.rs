//! The ray tubes a light source spans: the neighborhood of its rays, fixed where it is simple.
//!
//! A source sends out its rays in sheets. A collimated beam is one sheet of rays side by side, a
//! point source one sheet of directions, and every wavelength is a sheet of its own. Each sheet is
//! triangulated once, where the source sends it out; its triangles are the ray tubes. A fluence
//! map follows a tube only where all three of its rays arrived (see the handbook, "Fluence").
use std::{fmt, sync::Arc};

use log::warn;
use nalgebra::Vector3;
use spade::{DelaunayTriangulation, HasPosition, Point2 as SheetPoint, Triangulation};

use crate::{
    error::{OpmResult, OpossumError},
    light::{Ray, Rays},
};

/// A corner of the ray tubes of a sheet: where one ray starts, or several identical ones.
#[derive(Debug, Clone)]
pub struct RayVertex {
    /// Where the rays start, in the coordinates of their sheet.
    position: SheetPoint<f64>,
    /// The indices of the rays in the bundle the mesh was built from.
    rays: Vec<u32>,
}
impl RayVertex {
    /// Returns the indices of the rays starting at this corner, in the bundle the mesh was built
    /// from.
    #[must_use]
    pub fn rays(&self) -> &[u32] {
        &self.rays
    }
}
impl HasPosition for RayVertex {
    type Scalar = f64;

    fn position(&self) -> SheetPoint<f64> {
        self.position
    }
}

/// The ray tubes of a bundle of rays: one triangulation per sheet of the bundle.
pub struct RayMesh {
    sheets: Vec<DelaunayTriangulation<RayVertex>>,
    /// The number of rays of the bundle the mesh was built from.
    nr_of_rays: usize,
}
impl RayMesh {
    /// Triangulates the sheets of the given bundle.
    ///
    /// The rays are grouped by wavelength first. Of one wavelength, the rays that start alone at
    /// their position, or together with rays of the same direction, form one sheet: it is
    /// triangulated by where they start, seen along their mean direction. Rays that start at one
    /// position in different directions form a sheet of their own, triangulated by their
    /// directions; a ray of it pointing back against the mean direction is left out. Identical
    /// rays share a corner. A sheet of fewer than three corners, or of corners on one line, has no
    /// tubes.
    ///
    /// # Arguments
    ///
    /// * `rays` - the bundle, as its source sends it out. All its rays count, valid or not.
    ///
    /// # Errors
    ///
    /// This function returns an error if the bundle holds more than `u32::MAX` rays or if a ray
    /// does not start at a finite position in a finite direction.
    pub fn new(rays: &Rays) -> OpmResult<Self> {
        if u32::try_from(rays.nr_of_rays(false)).is_err() {
            return Err(OpossumError::Other(
                "a bundle of more than u32::MAX rays spans no ray tubes".into(),
            ));
        }
        let mut indexed: Vec<(u32, &Ray)> = (0..).zip(rays.iter()).collect();
        indexed.sort_by(|(_, a), (_, b)| {
            let (pa, pb) = (a.position(), b.position());
            a.wavelength()
                .value
                .total_cmp(&b.wavelength().value)
                .then(pa.x.value.total_cmp(&pb.x.value))
                .then(pa.y.value.total_cmp(&pb.y.value))
                .then(pa.z.value.total_cmp(&pb.z.value))
        });
        let mut sheets = Vec::new();
        for wavelength in indexed.chunk_by(|(_, a), (_, b)| a.wavelength() == b.wavelength()) {
            let mut side_by_side = Vec::new();
            for start in wavelength.chunk_by(|(_, a), (_, b)| a.position() == b.position()) {
                let direction = |ray: &Ray| ray.direction().normalize();
                let fanned_out = start.split_first().is_some_and(|((_, first), rest)| {
                    rest.iter()
                        .any(|(_, ray)| direction(ray) != direction(first))
                });
                if fanned_out {
                    sheets.push(Self::fan(start)?);
                } else {
                    side_by_side.extend_from_slice(start);
                }
            }
            if !side_by_side.is_empty() {
                sheets.push(Self::side_by_side(&side_by_side)?);
            }
        }
        Ok(Self {
            sheets,
            nr_of_rays: rays.nr_of_rays(false),
        })
    }
    /// Returns the triangulations of the sheets of this mesh, whose inner faces are its ray tubes.
    #[must_use]
    pub fn sheets(&self) -> &[DelaunayTriangulation<RayVertex>] {
        &self.sheets
    }
    /// Returns the number of rays of the bundle this mesh was built from.
    #[must_use]
    pub const fn nr_of_rays(&self) -> usize {
        self.nr_of_rays
    }
    /// Triangulates rays side by side by where they start, seen along their mean direction.
    fn side_by_side(rays: &[(u32, &Ray)]) -> OpmResult<DelaunayTriangulation<RayVertex>> {
        let axis = mean_direction(rays);
        let (u, v) = across(&axis);
        triangulate(rays.iter().map(|(index, ray)| {
            let start = ray.position().map(|c| c.value).coords;
            (SheetPoint::new(start.dot(&u), start.dot(&v)), *index)
        }))
    }
    /// Triangulates rays fanning out from one point by their directions, projected onto a plane
    /// across their mean direction. A ray pointing back against it is left out.
    fn fan(rays: &[(u32, &Ray)]) -> OpmResult<DelaunayTriangulation<RayVertex>> {
        let axis = mean_direction(rays);
        let (u, v) = across(&axis);
        let mut backward = false;
        let corners = rays
            .iter()
            .filter_map(|(index, ray)| {
                let direction = ray.direction().normalize();
                let along = direction.dot(&axis);
                backward |= along <= 0.0;
                (along > 0.0).then(|| {
                    let point = direction / along;
                    (SheetPoint::new(point.dot(&u), point.dot(&v)), *index)
                })
            })
            .collect::<Vec<_>>();
        if backward {
            warn!("a ray pointing back against the rays of its point source spans no ray tubes");
        }
        triangulate(corners)
    }
}
/// The mean direction of the given rays, or the direction of the first one where they cancel out.
fn mean_direction(rays: &[(u32, &Ray)]) -> Vector3<f64> {
    let first = rays
        .first()
        .map_or_else(Vector3::z, |(_, ray)| ray.direction().normalize());
    rays.iter()
        .map(|(_, ray)| ray.direction().normalize())
        .sum::<Vector3<f64>>()
        .try_normalize(f64::EPSILON)
        .unwrap_or(first)
}
/// Two unit vectors across the given unit vector and across each other.
fn across(axis: &Vector3<f64>) -> (Vector3<f64>, Vector3<f64>) {
    let helper = if axis.x.abs() < 0.9 {
        Vector3::x()
    } else {
        Vector3::y()
    };
    let u = axis.cross(&helper).normalize();
    let v = axis.cross(&u);
    (u, v)
}
/// Triangulates the given corners of a sheet, each with the index of its ray. Corners at the same
/// place become one.
fn triangulate(
    corners: impl IntoIterator<Item = (SheetPoint<f64>, u32)>,
) -> OpmResult<DelaunayTriangulation<RayVertex>> {
    let mut corners = corners.into_iter().collect::<Vec<_>>();
    corners.sort_by(|(a, _), (b, _)| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let mut sheet = DelaunayTriangulation::new();
    for same in corners
        .chunk_by(|(a, _), (b, _)| a.x.total_cmp(&b.x).is_eq() && a.y.total_cmp(&b.y).is_eq())
    {
        let Some((position, _)) = same.first() else {
            continue;
        };
        let vertex = RayVertex {
            position: *position,
            rays: same.iter().map(|(_, index)| *index).collect(),
        };
        sheet.insert(vertex).map_err(|e| {
            OpossumError::Other(format!("cannot span the ray tubes of a source: {e}"))
        })?;
    }
    Ok(sheet)
}
impl PartialEq for RayMesh {
    /// Two meshes are equal if their sheets have the same corners, in the same places and order,
    /// and the same tubes between them.
    fn eq(&self, other: &Self) -> bool {
        let corners = |sheet: &DelaunayTriangulation<RayVertex>| {
            sheet
                .vertices()
                .map(|v| (v.position().x, v.position().y, v.data().rays.clone()))
                .collect::<Vec<_>>()
        };
        let tubes = |sheet: &DelaunayTriangulation<RayVertex>| {
            sheet
                .inner_faces()
                .map(|face| face.vertices().map(|v| v.index()))
                .collect::<Vec<_>>()
        };
        self.nr_of_rays == other.nr_of_rays
            && self.sheets.len() == other.sheets.len()
            && self
                .sheets
                .iter()
                .zip(&other.sheets)
                .all(|(a, b)| corners(a) == corners(b) && tubes(a) == tubes(b))
    }
}
impl fmt::Debug for RayMesh {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RayMesh")
            .field("sheets", &self.sheets.len())
            .field(
                "tubes",
                &self
                    .sheets
                    .iter()
                    .map(|sheet| sheet.num_inner_faces())
                    .sum::<usize>(),
            )
            .finish()
    }
}

/// Where a ray stands in the ray tubes of its source: the mesh and the index of the ray in the
/// bundle the mesh was built from.
#[derive(Clone)]
pub struct RaySource {
    mesh: Arc<RayMesh>,
    index: u32,
}
impl RaySource {
    /// Creates a new [`RaySource`].
    ///
    /// # Arguments
    ///
    /// * `mesh` - the ray tubes of the bundle the ray was sent out in.
    /// * `index` - the index of the ray in that bundle.
    #[must_use]
    pub const fn new(mesh: Arc<RayMesh>, index: u32) -> Self {
        Self { mesh, index }
    }
    /// Returns the ray tubes of the bundle the ray was sent out in.
    #[must_use]
    pub const fn mesh(&self) -> &Arc<RayMesh> {
        &self.mesh
    }
    /// Returns the index of the ray in the bundle its mesh was built from.
    #[must_use]
    pub const fn index(&self) -> u32 {
        self.index
    }
}
impl PartialEq for RaySource {
    /// Two rays stand at the same place if they have the same index in equal meshes.
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && (Arc::ptr_eq(&self.mesh, &other.mesh) || self.mesh == other.mesh)
    }
}
impl fmt::Debug for RaySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RaySource")
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        degree,
        distributions::{energy::UniformDist, position::Hexapolar, spectral::LaserLines},
        joule, millimeter, nanometer,
        utils::{geom_transformation::Isometry, test_helper::helper::assert_warned},
    };

    /// A collimated bundle of 1000 nm along z from a hexapolar pattern of the given rings, 5 mm
    /// across.
    fn hexapolar(rings: u8) -> OpmResult<Rays> {
        Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Hexapolar::new(millimeter!(5.0), rings)?,
        )
    }
    /// The number of triangles of any triangulation of a pattern of `rings` hexapolar rings: its
    /// 1 + 3·k·(k+1) points span 2·n − h − 2 triangles, with its 6·k outermost points on the hull.
    const fn triangles_of_hexapolar(rings: usize) -> usize {
        let points = 1 + 3 * rings * (rings + 1);
        2 * points - 6 * rings - 2
    }
    /// The ray tubes of the only sheet of the given mesh.
    fn only_sheet(mesh: &RayMesh) -> &DelaunayTriangulation<RayVertex> {
        assert_eq!(mesh.sheets().len(), 1, "{mesh:?}");
        &mesh.sheets()[0]
    }
    /// The indices of the rays of every tube of the given sheet.
    fn rays_of_tubes(sheet: &DelaunayTriangulation<RayVertex>) -> Vec<Vec<u32>> {
        sheet
            .inner_faces()
            .map(|face| {
                face.vertices()
                    .iter()
                    .flat_map(|v| v.data().rays().to_vec())
                    .collect()
            })
            .collect()
    }

    /// A collimated beam is one sheet whose tubes span the pattern of its rays.
    #[test]
    fn a_collimated_beam_is_one_sheet_spanning_its_rays() -> OpmResult<()> {
        let mesh = RayMesh::new(&hexapolar(3)?)?;
        let sheet = only_sheet(&mesh);
        assert_eq!(sheet.num_vertices(), 37);
        assert_eq!(sheet.num_inner_faces(), triangles_of_hexapolar(3));
        Ok(())
    }
    /// Every wavelength is a sheet of its own: no tube joins rays of two wavelengths.
    #[test]
    fn each_wavelength_is_a_sheet_of_its_own() -> OpmResult<()> {
        let rays = Rays::new_collimated_with_spectrum(
            &LaserLines::new(vec![(nanometer!(1000.0), 1.0), (nanometer!(1064.0), 1.0)])?,
            &UniformDist::new(joule!(1.0))?,
            &Hexapolar::new(millimeter!(5.0), 3)?,
        )?;
        let mesh = RayMesh::new(&rays)?;
        assert_eq!(mesh.sheets().len(), 2);
        for sheet in mesh.sheets() {
            assert_eq!(sheet.num_inner_faces(), triangles_of_hexapolar(3));
            for tube in rays_of_tubes(sheet) {
                let wavelengths = tube
                    .iter()
                    .filter_map(|index| rays.iter().nth(*index as usize))
                    .map(Ray::wavelength)
                    .collect::<Vec<_>>();
                assert!(wavelengths.windows(2).all(|w| w[0] == w[1]), "{tube:?}");
            }
        }
        Ok(())
    }
    /// The rays of a point source all start at one point: they span their tubes by direction.
    #[test]
    fn a_point_source_spans_its_tubes_by_direction() -> OpmResult<()> {
        let rays = Rays::new_point_src_with_spectrum(
            &LaserLines::new(vec![(nanometer!(1000.0), 1.0)])?,
            &UniformDist::new(joule!(1.0))?,
            &Hexapolar::new(millimeter!(5.0), 3)?,
            millimeter!(100.0),
        )?;
        let mesh = RayMesh::new(&rays)?;
        assert_eq!(
            only_sheet(&mesh).num_inner_faces(),
            triangles_of_hexapolar(3)
        );
        Ok(())
    }
    /// Two point sources in one bundle, like two pixels of an image source, are two sheets: no
    /// tube joins rays of both.
    #[test]
    fn no_tube_joins_two_point_sources() -> OpmResult<()> {
        let cone_at = |x: f64| {
            Rays::new_hexapolar_point_source(
                millimeter!(x, 0.0, 0.0),
                degree!(10.0),
                2,
                nanometer!(1000.0),
                joule!(1.0),
            )
        };
        let mut rays = cone_at(0.0)?;
        rays.add_rays(&mut cone_at(1.0)?);
        let mesh = RayMesh::new(&rays)?;
        assert_eq!(mesh.sheets().len(), 2);
        for sheet in mesh.sheets() {
            assert_eq!(sheet.num_inner_faces(), triangles_of_hexapolar(2));
            for tube in rays_of_tubes(sheet) {
                let starts = tube
                    .iter()
                    .filter_map(|index| rays.iter().nth(*index as usize))
                    .map(Ray::position)
                    .collect::<Vec<_>>();
                assert!(starts.windows(2).all(|s| s[0] == s[1]), "{tube:?}");
            }
        }
        Ok(())
    }
    /// A ray of a point source pointing back against the others spans no tube, and that is said.
    #[test]
    fn a_ray_pointing_back_spans_no_tube() -> OpmResult<()> {
        let mut rays = Rays::new_hexapolar_point_source(
            millimeter!(0.0, 0.0, 0.0),
            degree!(10.0),
            2,
            nanometer!(1000.0),
            joule!(1.0),
        )?;
        rays.add_ray(Ray::new(
            millimeter!(0.0, 0.0, 0.0),
            -Vector3::z(),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        testing_logger::setup();
        let mesh = RayMesh::new(&rays)?;
        assert_warned("pointing back");
        assert_eq!(only_sheet(&mesh).num_vertices(), 19);
        Ok(())
    }
    /// Identical rays share a corner, so none of them is lost from the mesh.
    #[test]
    fn identical_rays_share_a_corner() -> OpmResult<()> {
        let mut rays = hexapolar(3)?;
        let copy = rays.iter().nth(5).cloned().expect("a ray to copy");
        rays.add_ray(copy);
        let mesh = RayMesh::new(&rays)?;
        let sheet = only_sheet(&mesh);
        assert_eq!(sheet.num_vertices(), 37);
        assert!(
            sheet.vertices().any(|v| v.data().rays() == [5, 37]),
            "the copy shares the corner of its original"
        );
        Ok(())
    }
    /// Fewer than three rays, or rays on one line, span no tubes, and that is no error.
    #[test]
    fn a_line_of_rays_spans_no_tubes() -> OpmResult<()> {
        let mut rays = Rays::default();
        for y in 0..5 {
            rays.add_ray(Ray::new_collimated(
                millimeter!(0.0, f64::from(y), 0.0),
                nanometer!(1000.0),
                joule!(1.0),
            )?);
        }
        assert_eq!(only_sheet(&RayMesh::new(&rays)?).num_inner_faces(), 0);
        Ok(())
    }
    /// A beam running along x spans the same tubes as one along z: the sheet is seen across
    /// whatever direction the rays run in.
    #[test]
    fn a_beam_along_x_spans_its_tubes_across_x() -> OpmResult<()> {
        let along_x = hexapolar(3)?.transformed_by_iso(&Isometry::new(
            millimeter!(0.0, 0.0, 0.0),
            degree!(0.0, 90.0, 0.0),
        )?);
        assert!(along_x.iter().all(|ray| ray.direction().x > 0.99));
        assert_eq!(
            only_sheet(&RayMesh::new(&along_x)?).num_inner_faces(),
            triangles_of_hexapolar(3)
        );
        Ok(())
    }
    /// A ray that starts at no finite position cannot be triangulated, and that is an error.
    #[test]
    fn a_ray_at_no_finite_position_is_an_error() -> OpmResult<()> {
        let mut rays = hexapolar(1)?;
        rays.add_ray(Ray::new_collimated(
            millimeter!(f64::NAN, 0.0, 0.0),
            nanometer!(1000.0),
            joule!(1.0),
        )?);
        let error = RayMesh::new(&rays).expect_err("a ray at no finite position");
        assert!(error.to_string().contains("ray tubes"), "{error}");
        Ok(())
    }
    /// The mesh of a bundle is the same each time it is built, and that of another bundle differs.
    #[test]
    fn a_bundle_spans_the_same_mesh_each_time() -> OpmResult<()> {
        let rays = hexapolar(3)?;
        assert_eq!(RayMesh::new(&rays)?, RayMesh::new(&rays)?);
        assert_ne!(RayMesh::new(&rays)?, RayMesh::new(&hexapolar(2)?)?);
        Ok(())
    }
    /// A ray keeps its place in the tubes when it is copied, and drops it when written to file.
    #[test]
    fn a_ray_keeps_its_source_when_copied_but_not_when_written() -> OpmResult<()> {
        let mut rays = hexapolar(1)?;
        rays.attach_mesh()?;
        let ray = rays.iter().nth(3).cloned().expect("a ray");
        let source = ray.source().expect("a place in the tubes");
        assert_eq!(
            source,
            &RaySource::new(Arc::new(RayMesh::new(&hexapolar(1)?)?), 3)
        );
        let written = ron::to_string(&ray).expect("a ray can be written");
        let read: Ray = ron::from_str(&written).expect("a written ray can be read");
        assert!(read.source().is_none());
        assert_eq!(read.position(), ray.position());
        Ok(())
    }
}
