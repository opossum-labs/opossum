//! The ray tubes a light source spans: the neighborhood of its rays, fixed where it is simple.
//!
//! A source sends out its rays in sheets. A collimated beam is one sheet of rays side by side, a
//! point source one sheet of directions, and every wavelength is a sheet of its own. Each sheet is
//! triangulated once, where the source sends it out; its triangles are the ray tubes. A fluence
//! map follows a tube only where all three of its rays arrived (see the handbook, "Fluence").
use std::{cmp::Ordering, fmt, sync::Arc};

use itertools::Itertools;
use log::warn;
use nalgebra::{Point2, Vector2, Vector3};
use spade::{
    DelaunayTriangulation, HasPosition, Point2 as SheetPoint, Triangulation,
    handles::{DirectedEdgeHandle, FaceHandle, InnerTag, VertexHandle},
};

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
    /// The area the corner stands for at its source (see [`source_cell_area`]).
    cell_area: f64,
}
impl RayVertex {
    /// Returns the indices of the rays starting at this corner, in the bundle the mesh was built
    /// from.
    #[must_use]
    pub fn rays(&self) -> &[u32] {
        &self.rays
    }
    /// Returns the area this corner stands for at its source, in the coordinates of its sheet:
    /// its Voronoi cell, and beyond each outer edge of the sheet half of the margin of half a tube
    /// height there.
    #[must_use]
    pub const fn cell_area(&self) -> f64 {
        self.cell_area
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
                let fanned_out = !start
                    .iter()
                    .map(|(_, ray)| ray.direction().normalize())
                    .all_equal();
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
        let corners = rays
            .iter()
            .filter_map(|(index, ray)| {
                let direction = ray.direction().normalize();
                let along = direction.dot(&axis);
                (along > 0.0).then(|| {
                    let point = direction / along;
                    (SheetPoint::new(point.dot(&u), point.dot(&v)), *index)
                })
            })
            .collect::<Vec<_>>();
        if corners.len() < rays.len() {
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
/// Triangulates the given corners of a sheet, each with the index of its ray, and gives each
/// corner the area it stands for (see [`source_cell_area`]). Corners at the same place become one.
fn triangulate(
    corners: impl IntoIterator<Item = (SheetPoint<f64>, u32)>,
) -> OpmResult<DelaunayTriangulation<RayVertex>> {
    let order = |(a, _): &(SheetPoint<f64>, u32), (b, _): &(SheetPoint<f64>, u32)| {
        a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y))
    };
    let mut corners = corners.into_iter().collect::<Vec<_>>();
    corners.sort_by(order);
    let mut sheet = DelaunayTriangulation::new();
    for same in corners.chunk_by(|a, b| order(a, b) == Ordering::Equal) {
        let Some((position, _)) = same.first() else {
            continue;
        };
        let vertex = RayVertex {
            position: *position,
            rays: same.iter().map(|(_, index)| *index).collect(),
            cell_area: 0.0,
        };
        sheet.insert(vertex).map_err(|e| {
            OpossumError::Other(format!("cannot span the ray tubes of a source: {e}"))
        })?;
    }
    let cell_areas = sheet
        .vertices()
        .map(|corner| (corner.fix(), source_cell_area(corner)))
        .collect::<Vec<_>>();
    for (corner, area) in cell_areas {
        sheet.vertex_data_mut(corner).cell_area = area;
    }
    Ok(sheet)
}
/// The area a corner stands for at its source: its Voronoi cell, and beyond each outer edge of
/// its sheet the half of the margin there on its side, cut square through the middle of the
/// edge, so that the piece where the margins of two edges meet belongs to the corner between them.
///
/// The Voronoi cell is summed from the tubes around the corner, each contributing the part of
/// itself that is closer to the corner than to its other corners. That holds however the tubes of
/// a regular grid are cut into triangles, which a third of their areas does not.
fn source_cell_area(corner: VertexHandle<'_, RayVertex>) -> f64 {
    let position = |v: VertexHandle<'_, RayVertex>| Point2::new(v.position().x, v.position().y);
    let at = |v: VertexHandle<'_, RayVertex>| Some(position(v));
    let all = |_: Tube<'_>| true;
    corner
        .out_edges()
        .map(|edge| {
            // Every tube around the corner lies left of one of its edges, every outer edge
            // of the sheet through it is one of them.
            let inside = edge.opposite_vertex().map_or(0.0, |third| {
                voronoi_part(position(corner), position(edge.to()), position(third))
            });
            let margin = if let Some([start, end, end_out, start_out]) = margin(edge, &all, &at) {
                let (middle, middle_out) = halfway([start, end, end_out, start_out]);
                quadrilateral_area([start, middle, middle_out, start_out])
            } else if let Some([start, end, end_out, start_out]) = margin(edge.rev(), &all, &at) {
                let (middle, middle_out) = halfway([start, end, end_out, start_out]);
                quadrilateral_area([middle, end, end_out, middle_out])
            } else {
                0.0
            };
            inside + margin
        })
        .sum()
}
/// Returns where a cut square through the middle of the edge of a margin (see [`margin`]) meets
/// the edge and the far side of the margin.
fn halfway([start, end, end_out, start_out]: [Point2<f64>; 4]) -> (Point2<f64>, Point2<f64>) {
    let middle = start + (end - start) / 2.0;
    let far = end_out - start_out;
    let length_squared = far.norm_squared();
    if length_squared <= 0.0 {
        return (middle, end_out);
    }
    (
        middle,
        start_out + far * ((middle - start_out).dot(&far) / length_squared),
    )
}
/// The area of a quadrilateral whose sides do not cross, from its corners in order.
fn quadrilateral_area([a, b, c, d]: [Point2<f64>; 4]) -> f64 {
    (c - a).perp(&(d - b)).abs() / 2.0
}
/// A ray tube: an inner face of the triangulation of a sheet.
pub(crate) type Tube<'a> = FaceHandle<'a, InnerTag, RayVertex, (), (), ()>;
/// An edge between two corners of the ray tubes of a sheet, running one way.
pub(crate) type TubeEdge<'a> = DirectedEdgeHandle<'a, RayVertex, (), (), ()>;
/// Returns the margin beyond the given edge of the alive tubes of a sheet, if no alive tube
/// continues it: a strip half as wide as its tube is high, running out from the edge away from
/// the tube. Where the margins of two edges meet at a corner, both end on the bisector there, so
/// they neither overlap nor leave a gap; where the edge turns by more than 150°, the margin ends
/// square to it (see the handbook, "Fluence").
///
/// # Arguments
///
/// * `edge` - an edge with an alive tube on its left.
/// * `alive` - whether a tube is alive.
/// * `at` - where a corner lies, `None` if it lies nowhere.
///
/// # Returns
///
/// The corners of the margin: the start and the end of the edge, then the points beyond its end
/// and beyond its start, which are one point where the margin closes to a triangle. `None` if an
/// alive tube continues the edge, if the edge has no length or if one of its corners lies nowhere.
pub(crate) fn margin(
    edge: TubeEdge<'_>,
    alive: &impl Fn(Tube<'_>) -> bool,
    at: &impl Fn(VertexHandle<'_, RayVertex>) -> Option<Point2<f64>>,
) -> Option<[Point2<f64>; 4]> {
    let is_alive = |side: TubeEdge<'_>| side.face().as_inner().is_some_and(alive);
    if !is_alive(edge) || is_alive(edge.rev()) {
        return None;
    }
    let (start, end) = (at(edge.from())?, at(edge.to())?);
    let (normal, width) = outward(edge, at)?;
    let normal_of = |other: TubeEdge<'_>| outward(other, at).map(|(normal, _)| normal);
    let start_out = miter(
        start,
        normal,
        previous_edge(edge, alive).and_then(normal_of),
        width,
    );
    let end_out = miter(
        end,
        normal,
        next_edge(edge, alive).and_then(normal_of),
        width,
    );
    let (to_start_out, to_end_out) = (start_out - start, end_out - end);
    if (end_out - start_out).dot(&(end - start)) <= 0.0 {
        // the ends cross before the margin is as wide: it closes where they meet
        let across = to_start_out.perp(&to_end_out);
        let meet = start + to_start_out * ((end - start).perp(&to_end_out) / across);
        return (across != 0.0).then_some([start, end, meet, meet]);
    }
    Some([start, end, end_out, start_out])
}
/// Returns the direction away from the tube on the left of the given edge, square to the edge,
/// and half the height of that tube above the edge, which is how wide its margin is; `None` if
/// the edge has no length or a corner lies nowhere.
fn outward(
    edge: TubeEdge<'_>,
    at: &impl Fn(VertexHandle<'_, RayVertex>) -> Option<Point2<f64>>,
) -> Option<(Vector2<f64>, f64)> {
    let (start, end, third) = (
        at(edge.from())?,
        at(edge.to())?,
        at(edge.opposite_vertex()?)?,
    );
    let along = end - start;
    let length = along.norm();
    if length <= 0.0 {
        return None;
    }
    let normal = Vector2::new(along.y, -along.x) / length;
    let height = normal.dot(&(third - start));
    Some((
        if height > 0.0 { -normal } else { normal },
        height.abs() / 2.0,
    ))
}
/// Returns where a margin `width` wide in the direction `normal` ends at the given corner: on
/// the bisector with the margin of the `other` edge there, or square to its own edge if there is
/// no other edge or the edge turns by more than 150°, where the end would reach out more than
/// four times as far as the margin is wide.
fn miter(
    corner: Point2<f64>,
    normal: Vector2<f64>,
    other: Option<Vector2<f64>>,
    width: f64,
) -> Point2<f64> {
    // the cosine of half the sharpest turn: 75°
    const SHARPEST: f64 = 0.258_819_045_102_520_8;
    let bisector = other
        .and_then(|other| (normal + other).try_normalize(f64::EPSILON))
        .filter(|bisector| bisector.dot(&normal) >= SHARPEST)
        .unwrap_or(normal);
    corner + bisector * (width / bisector.dot(&normal))
}
/// Returns the edge of the alive tubes that follows the given one at its end, found by turning
/// around that corner through the alive tubes: the first edge with an alive tube on its left and
/// none on its right.
fn next_edge<'a>(edge: TubeEdge<'a>, alive: &impl Fn(Tube<'_>) -> bool) -> Option<TubeEdge<'a>> {
    // the tube on the left of the given edge lies left of this one
    let mut next = edge.rev().cw();
    for _ in 0..edge.to().out_edges().count() {
        let turned = next.cw();
        if !turned.face().as_inner().is_some_and(alive) {
            return Some(next);
        }
        next = turned;
    }
    None
}
/// Returns the edge of the alive tubes that the given one follows at its start, found by turning
/// around that corner through the alive tubes the other way.
fn previous_edge<'a>(
    edge: TubeEdge<'a>,
    alive: &impl Fn(Tube<'_>) -> bool,
) -> Option<TubeEdge<'a>> {
    // the tube on the left of the given edge lies right of this one
    let mut back = edge.ccw();
    for _ in 0..edge.from().out_edges().count() {
        if !back.face().as_inner().is_some_and(alive) {
            return Some(back.rev());
        }
        back = back.ccw();
    }
    None
}
/// The part of the triangle `p q r` that lies closer to `p` than to `q` and `r`, signed: negative
/// beyond an obtuse angle, where the parts of the neighbors make up for it.
fn voronoi_part(p: Point2<f64>, q: Point2<f64>, r: Point2<f64>) -> f64 {
    let twice_area = (q - p).perp(&(r - p)).abs();
    if twice_area <= 0.0 {
        return 0.0;
    }
    let (pq, pr) = ((p - q).norm_squared(), (p - r).norm_squared());
    pq.mul_add((p - r).dot(&(q - r)), pr * (p - q).dot(&(r - q))) / (8.0 * twice_area)
}
impl PartialEq for RayMesh {
    /// Two meshes are equal if their sheets have the same corners, in the same places and order,
    /// and the same tubes between them.
    fn eq(&self, other: &Self) -> bool {
        let same_sheet = |a: &DelaunayTriangulation<RayVertex>,
                          b: &DelaunayTriangulation<RayVertex>| {
            a.num_vertices() == b.num_vertices()
                && a.num_inner_faces() == b.num_inner_faces()
                && a.vertices().zip(b.vertices()).all(|(va, vb)| {
                    let (pa, pb) = (va.position(), vb.position());
                    pa.x.total_cmp(&pb.x).is_eq()
                        && pa.y.total_cmp(&pb.y).is_eq()
                        && va.data().rays() == vb.data().rays()
                })
                && a.inner_faces().zip(b.inner_faces()).all(|(fa, fb)| {
                    fa.vertices().map(|v| v.index()) == fb.vertices().map(|v| v.index())
                })
        };
        self.nr_of_rays == other.nr_of_rays
            && self.sheets.len() == other.sheets.len()
            && self
                .sheets
                .iter()
                .zip(&other.sheets)
                .all(|(a, b)| same_sheet(a, b))
    }
}
/// Positions in a mesh are finite, as spade inserts no others, so equality is reflexive.
impl Eq for RayMesh {}
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
    /// Two rays stand at the same place if they have the same index in equal meshes; a shared
    /// mesh is equal without being compared.
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.mesh == other.mesh
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
        distributions::{
            energy::UniformDist,
            position::{Grid, Hexapolar},
            spectral::LaserLines,
        },
        joule, millimeter, nanometer,
        nodes::test_helper::helper::rays_along_z_from,
        utils::{geom_transformation::Isometry, test_helper::helper::assert_warned},
    };

    use crate::utils::test_helper::helper::hexapolar_bundle as hexapolar;
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
        let starts = (0..5)
            .map(|y| millimeter!(0.0, f64::from(y), 0.0))
            .collect::<Vec<_>>();
        let rays = rays_along_z_from(&starts, nanometer!(1000.0))?;
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
    /// A Voronoi part of a right isosceles triangle is a quarter of its square at the right angle
    /// and an eighth at each of the other corners, so the parts make up the triangle.
    #[test]
    fn the_voronoi_parts_of_a_triangle_make_it_up() {
        let (p, q, r) = (
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 1.0),
        );
        approx::assert_relative_eq!(voronoi_part(p, q, r), 0.25);
        approx::assert_relative_eq!(voronoi_part(q, r, p), 0.125);
        approx::assert_relative_eq!(voronoi_part(r, p, q), 0.125);
    }
    /// The corners of a square grid of 1 mm stand for a square of 1 mm² each: inside, on its
    /// edges, where the margin makes up the missing half, and at its corners, where the margins
    /// of the two sides meet on the bisector and make up the missing quarter.
    #[test]
    fn the_corners_of_a_grid_stand_for_their_squares() -> OpmResult<()> {
        let rays = Rays::new_uniform_collimated(
            nanometer!(1000.0),
            joule!(1.0),
            &Grid::new(millimeter!(2.0, 2.0), nalgebra::Point2::new(3, 3))?,
        )?;
        let mesh = RayMesh::new(&rays)?;
        let areas = only_sheet(&mesh)
            .vertices()
            .map(|v| v.data().cell_area() * 1e6)
            .collect::<Vec<_>>();
        assert_eq!(areas.len(), 9);
        for area in areas {
            approx::assert_relative_eq!(area, 1.0, max_relative = 1e-9);
        }
        Ok(())
    }
    /// The margins around a single dead tube are wider than its corners leave room for: each
    /// closes to a triangle whose tip lies where the bisectors of the corners meet, and together
    /// they fill the tube without overlapping.
    #[test]
    fn the_margins_around_a_dead_tube_fill_it() -> OpmResult<()> {
        let mesh = RayMesh::new(&hexapolar(2)?)?;
        let sheet = only_sheet(&mesh);
        let center = |tube: &Tube<'_>| {
            tube.vertices()
                .iter()
                .any(|v| v.position().x.hypot(v.position().y) < 1e-12)
        };
        let dead = sheet
            .inner_faces()
            .find(center)
            .ok_or_else(|| OpossumError::Other("no tube at the center".into()))?;
        let alive = |tube: Tube<'_>| tube.fix() != dead.fix();
        let at = |v: VertexHandle<'_, RayVertex>| Some(Point2::new(v.position().x, v.position().y));
        let mut area = 0.0;
        for edge in dead.adjacent_edges() {
            let [start, end, end_out, start_out] = margin(edge.rev(), &alive, &at)
                .ok_or_else(|| OpossumError::Other("no margin".into()))?;
            assert_eq!(end_out, start_out);
            area += (end - start).perp(&(end_out - start)).abs() / 2.0;
        }
        approx::assert_relative_eq!(area, dead.area(), max_relative = 1e-9);
        Ok(())
    }
    /// A margin ends on the bisector with the margin of the next edge, and square to its own edge
    /// where the edge turns by more than 150°.
    #[test]
    fn a_margin_ends_on_the_bisector_unless_the_edge_turns_sharply() {
        let (corner, down) = (Point2::new(0.0, 0.0), Vector2::new(0.0, -1.0));
        // the normal of the next edge, where the edge turns by the given angle in degrees
        let turned = |angle: f64| {
            let angle = angle.to_radians();
            Vector2::new(-angle.sin(), -angle.cos())
        };
        approx::assert_relative_eq!(
            miter(corner, down, Some(turned(90.0)), 1.0),
            Point2::new(-1.0, -1.0),
            epsilon = 1e-12
        );
        approx::assert_relative_eq!(
            miter(corner, down, Some(turned(120.0)), 1.0),
            Point2::new(-(3.0_f64.sqrt()), -1.0),
            epsilon = 1e-12
        );
        approx::assert_relative_eq!(
            miter(corner, down, Some(turned(160.0)), 1.0),
            Point2::new(0.0, -1.0),
            epsilon = 1e-12
        );
        approx::assert_relative_eq!(miter(corner, down, None, 1.0), Point2::new(0.0, -1.0));
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
