#![warn(missing_docs)]
//! The shape of an optical component: its faces and where they end, in frames.
//!
//! A node's properties (curvatures, thickness, optic shape, ...) are what a user enters; the node
//! derives a [`Geometry`] from them (see
//! [`OpticNode::geometry`](crate::core_optics::OpticNode::geometry)), stated relative to the node.
//! Moved to where the node sits (see [`Geometry::moved`]), the same geometry is what rays are
//! traced against. The node type thereby acts as a preset: a lens is a [`Geometry::singlet`] whose
//! parameters are the lens's properties.
//!
//! A component is a single surface ([`Geometry::Surface`]) or one of the kinds of solid enclosing a
//! volume of material. Every kind is described in a file of its own and answers what
//! [`GeometryKind`] asks; [`Geometry::kind`] is the one place that tells the kinds apart.

use nalgebra::Point3;
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use uom::si::f64::{Angle, Length};

use crate::{
    apertures::Outline,
    error::OpmResult,
    geometry::{Assembly, Extruded, Face, ParametricSolid, Rim, SurfaceShape, body::Body},
    utils::geom_transformation::Isometry,
};

/// The shape of an optical component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Geometry {
    /// A single surface without thickness, such as a thin mirror, a grating or a detector.
    Surface(Box<SurfaceGeometry>),
    /// Two faces capping a profile extruded along an axis: lens, cylindric lens, wedge, plate.
    Extruded(Box<Extruded>),
    /// A solid bounded by several faces: the intersection of the inner sides of all of them.
    Parametric(ParametricSolid),
    /// Several solids, each with its own material, sharing interfaces.
    Assembly(Assembly),
}

impl Geometry {
    /// A spherical singlet lens: two spherical (or flat) faces a center thickness apart.
    ///
    /// # Arguments
    ///
    /// * `front_radius` - radius of curvature of the front face; infinite for a flat face.
    /// * `rear_radius` - radius of curvature of the rear face; infinite for a flat face.
    /// * `center_thickness` - distance between the two vertices.
    /// * `outline` - the transversal extent (optic shape).
    ///
    /// # Errors
    ///
    /// This function returns an error if the center thickness is not finite or if a face is curved
    /// too tightly to reach the edge of the outline.
    pub fn singlet(
        front_radius: Length,
        rear_radius: Length,
        center_thickness: Length,
        outline: Outline,
    ) -> OpmResult<Self> {
        Self::two_faced(
            SurfaceShape::spherical(front_radius)?,
            SurfaceShape::spherical(rear_radius)?,
            Isometry::new_along_z(center_thickness)?,
            outline,
        )
    }
    /// A cylindrical singlet lens: two cylindrical (or flat) faces a center thickness apart.
    ///
    /// # Arguments
    ///
    /// * `front_radius` - radius of curvature of the front face; infinite for a flat face.
    /// * `rear_radius` - radius of curvature of the rear face; infinite for a flat face.
    /// * `center_thickness` - distance between the two vertices.
    /// * `outline` - the transversal extent (optic shape).
    ///
    /// # Errors
    ///
    /// This function returns an error if the center thickness is not finite or if a face is curved
    /// too tightly to reach the edge of the outline.
    pub fn cylindrical_singlet(
        front_radius: Length,
        rear_radius: Length,
        center_thickness: Length,
        outline: Outline,
    ) -> OpmResult<Self> {
        Self::two_faced(
            SurfaceShape::cylindrical(front_radius)?,
            SurfaceShape::cylindrical(rear_radius)?,
            Isometry::new_along_z(center_thickness)?,
            outline,
        )
    }
    /// A wedge: two flat faces, the rear one tilted about the local x axis.
    ///
    /// # Arguments
    ///
    /// * `center_thickness` - distance between the two faces on the axis.
    /// * `wedge_angle` - tilt of the rear face.
    /// * `outline` - the transversal extent (optic shape).
    ///
    /// # Errors
    ///
    /// This function returns an error if the center thickness or the angle is not finite.
    pub fn wedge(
        center_thickness: Length,
        wedge_angle: Angle,
        outline: Outline,
    ) -> OpmResult<Self> {
        let tilt = Isometry::new(
            Point3::origin(),
            Point3::new(wedge_angle, Angle::zero(), Angle::zero()),
        )?;
        Self::two_faced(
            SurfaceShape::Plane,
            SurfaceShape::Plane,
            Isometry::new_along_z(center_thickness)?.append(&tilt),
            outline,
        )
    }
    /// A single surface.
    ///
    /// # Arguments
    ///
    /// * `axis` - the frame of the outline relative to the node; its z axis is the direction the
    ///   outline is projected along. The identity for a surface used along its own axis.
    /// * `shape` - the shape of the surface.
    /// * `vertex` - position and orientation of its vertex, stated in the `axis` frame.
    /// * `outline` - its transversal extent in the xy plane of the `axis` frame, or `None` for an
    ///   unbounded (virtual) surface.
    ///
    /// # Errors
    ///
    /// This function returns an error if the surface is curved too tightly to reach the edge of the
    /// outline.
    pub fn surface(
        axis: Isometry,
        shape: SurfaceShape,
        vertex: Isometry,
        outline: Option<Outline>,
    ) -> OpmResult<Self> {
        Ok(Self::Surface(Box::new(SurfaceGeometry::new(
            axis,
            shape,
            vertex,
            outline,
            SurfaceExtent::Edge,
        )?)))
    }
    /// A detector: a surface at the node that records the light within its outline and lets all
    /// of it pass (see [`SurfaceExtent::Window`]).
    ///
    /// # Arguments
    ///
    /// * `shape` - the shape of the surface the light is recorded on.
    /// * `outline` - the window the light is recorded within, or `None` to record all of it.
    ///
    /// # Errors
    ///
    /// See [`Geometry::surface`].
    pub fn detector(shape: SurfaceShape, outline: Option<Outline>) -> OpmResult<Self> {
        Ok(Self::Surface(Box::new(SurfaceGeometry::new(
            Isometry::identity(),
            shape,
            Isometry::identity(),
            outline,
            SurfaceExtent::Window,
        )?)))
    }
    /// A flat surface at the node itself.
    ///
    /// # Arguments
    ///
    /// * `outline` - its transversal extent, or `None` for an unbounded (virtual) surface.
    #[must_use]
    pub fn plane(outline: Option<Outline>) -> Self {
        // A plane reaches every outline, so nothing is left to check.
        Self::Surface(Box::new(SurfaceGeometry {
            face: Face::new(SurfaceShape::Plane, Isometry::identity()),
            extent: outline.map_or(SurfaceExtent::Open, |outline| {
                SurfaceExtent::Edge(Rim::new(outline, Isometry::identity()))
            }),
        }))
    }
    /// An extruded solid along the node's own axis whose front vertex sits at the node.
    ///
    /// # Arguments
    ///
    /// * `front` - the shape of the front face.
    /// * `rear` - the shape of the rear face.
    /// * `rear_vertex` - position and orientation of the rear face's vertex relative to the node.
    /// * `outline` - the transversal extent (optic shape).
    ///
    /// # Errors
    ///
    /// See [`Extruded::new`].
    fn two_faced(
        front: SurfaceShape,
        rear: SurfaceShape,
        rear_vertex: Isometry,
        outline: Outline,
    ) -> OpmResult<Self> {
        Ok(Self::Extruded(Box::new(Extruded::new(
            Isometry::identity(),
            Face::new(front, Isometry::identity()),
            Face::new(rear, rear_vertex),
            outline,
        )?)))
    }
    /// Return this geometry moved by an isometry, such as the placement of its node.
    ///
    /// Every face and every rim is moved; the geometry is the same thing at another place.
    ///
    /// # Arguments
    ///
    /// * `by` - the frame this geometry is stated in, given in the frame it is moved to.
    #[must_use]
    pub fn moved(&self, by: &Isometry) -> Self {
        self.kind().moved(by)
    }
    /// Return the kind of geometry this is, as the one thing every question is asked of.
    #[must_use]
    pub fn kind(&self) -> &dyn GeometryKind {
        match self {
            Self::Surface(surface) => &**surface,
            Self::Extruded(extruded) => &**extruded,
            Self::Parametric(parametric) => parametric,
            Self::Assembly(assembly) => assembly,
        }
    }
}

/// What every kind of geometry answers about its faces, its volume and its placement.
///
/// The faces of a geometry are numbered. What a face means, for instance which one light enters
/// through, is not decided here but by the node using the geometry. Every kind answers each
/// question itself.
pub trait GeometryKind {
    /// Return the face with the given number.
    ///
    /// # Returns
    ///
    /// The face, in the frame the geometry is stated in, or `None` if there is no such face.
    fn face(&self, index: usize) -> Option<&Face>;
    /// Return the number of faces.
    fn face_count(&self) -> usize;
    /// Return whether a point on the unbounded surface of a face belongs to the component.
    ///
    /// The surface of a face is unbounded: a plane reaches in every direction, a sphere closes on
    /// itself. Which of its points belong to the component is the lateral boundary of the face, and
    /// it depends on the kind: an extruded solid ends at the [`Rim`] of its profile, the faces of a
    /// parametric solid end at each other, a part of an assembly ends where its own solid does. A
    /// ray meets a face where its surface does and this agrees.
    ///
    /// # Arguments
    ///
    /// - `face`: the number of the face the point lies on.
    /// - `point`: a point on the surface of that face, in the frame the geometry is stated in.
    ///
    /// # Returns
    ///
    /// `true` if the face reaches to this point; `false` if the point lies on the surface beyond
    /// the edge of the face, or if there is no such face.
    fn includes(&self, face: usize, point: &Point3<Length>) -> bool;
    /// Return the window a detector records within, or `None` if all light on its faces is recorded.
    ///
    /// A window is no boundary: light outside it passes the face like light inside, it is just not
    /// recorded (see [`GeometryKind::includes`] for where a face ends).
    fn window(&self) -> Option<&Rim>;
    /// Return the volume this geometry encloses, or `None` for a single surface, which encloses
    /// none.
    fn as_body(&self) -> Option<&dyn Body>;
    /// Return this geometry moved by an isometry (see [`Geometry::moved`]).
    ///
    /// # Arguments
    ///
    /// * `by` - the frame this geometry is stated in, given in the frame it is moved to.
    fn moved(&self, by: &Isometry) -> Geometry;
}

/// A single surface without thickness.
///
/// The face is stated in the frame of the geometry. The rim of its extent, if there is one, keeps
/// the axis its outline is extruded along; an off-axis parabola uses it to measure its optic shape
/// parallel to the axis of its parent parabola. An open surface has no rim and so no axis of its
/// own, which is why the axis is already applied to the face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceGeometry {
    face: Face,
    extent: SurfaceExtent,
}

/// How far a [`SurfaceGeometry`] reaches sideways, and what its rim means for a ray outside it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SurfaceExtent {
    /// The surface is unbounded.
    Open,
    /// The component ends at the rim: a ray outside misses it, and the analyzer's missed surface
    /// strategy decides what happens to the ray.
    Edge(Rim),
    /// The rim only bounds what a detector records: a ray outside passes the surface like one
    /// inside, but is not recorded and leaves no hit.
    Window(Rim),
}

impl SurfaceExtent {
    /// Return this extent moved by an isometry (see [`Rim::moved`]).
    fn moved(&self, by: &Isometry) -> Self {
        match self {
            Self::Open => Self::Open,
            Self::Edge(rim) => Self::Edge(rim.moved(by)),
            Self::Window(rim) => Self::Window(rim.moved(by)),
        }
    }
}

impl SurfaceGeometry {
    /// Create a new [`SurfaceGeometry`].
    ///
    /// # Arguments
    ///
    /// * `axis` - the frame of the outline relative to the node.
    /// * `shape` - the shape of the surface.
    /// * `vertex` - position and orientation of its vertex, stated in the `axis` frame.
    /// * `outline` - its transversal extent, or `None` for an unbounded surface.
    /// * `extent` - what the rim of the outline means for a ray outside it.
    ///
    /// # Errors
    ///
    /// This function returns an error if the surface is curved too tightly to reach the edge of the
    /// outline.
    fn new(
        axis: Isometry,
        shape: SurfaceShape,
        vertex: Isometry,
        outline: Option<Outline>,
        extent: fn(Rim) -> SurfaceExtent,
    ) -> OpmResult<Self> {
        let face = Face::new(shape, axis.append(&vertex));
        if let Some(outline) = &outline {
            face.check_reach(outline.transversal_reach(), "surface")?;
        }
        Ok(Self {
            face,
            extent: outline.map_or(SurfaceExtent::Open, |outline| {
                extent(Rim::new(outline, axis))
            }),
        })
    }
    /// The number of the one face of a surface.
    pub const FACE: usize = 0;
}

/// A surface ends at its edge, if it has one. The window of a detector is no edge: the surface
/// reaches as far as light does.
impl GeometryKind for SurfaceGeometry {
    fn face(&self, index: usize) -> Option<&Face> {
        (index == Self::FACE).then_some(&self.face)
    }
    fn face_count(&self) -> usize {
        1
    }
    fn includes(&self, face: usize, point: &Point3<Length>) -> bool {
        GeometryKind::face(self, face).is_some_and(|surface| match &self.extent {
            SurfaceExtent::Edge(rim) => rim.bounds(surface, point),
            SurfaceExtent::Open | SurfaceExtent::Window(_) => true,
        })
    }
    fn window(&self) -> Option<&Rim> {
        match &self.extent {
            SurfaceExtent::Window(rim) => Some(rim),
            SurfaceExtent::Open | SurfaceExtent::Edge(_) => None,
        }
    }
    fn as_body(&self) -> Option<&dyn Body> {
        None
    }
    fn moved(&self, by: &Isometry) -> Geometry {
        Geometry::Surface(Box::new(Self {
            face: self.face.moved(by),
            extent: self.extent.moved(by),
        }))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::CircleShape,
        degree,
        error::OpossumError,
        geometry::{Parabola, Part, Side},
        material::Material,
        millimeter,
        properties::proptype::AssetRef,
        refractive_index::RefrIndexConst,
    };
    use approx::assert_abs_diff_eq;
    use uom::si::length::millimeter;

    fn circle() -> OpmResult<Outline> {
        Outline::new(CircleShape::new(millimeter!(12.5))?, Vec::new())
    }
    /// Return the frame of the face with the given number.
    fn frame_of(geometry: &Geometry, index: usize) -> OpmResult<Isometry> {
        geometry
            .kind()
            .face(index)
            .map(|face| *face.frame())
            .ok_or_else(|| OpossumError::Other(format!("there is no face {index}")))
    }
    #[test]
    fn a_surface_has_one_face() {
        let plane = Geometry::plane(None);
        assert_eq!(plane.kind().face_count(), 1);
        assert!(plane.kind().face(SurfaceGeometry::FACE).is_some());
        assert!(plane.kind().face(SurfaceGeometry::FACE + 1).is_none());
    }
    #[test]
    fn a_singlet_has_its_rear_vertex_one_center_thickness_behind_the_front() -> OpmResult<()> {
        let geometry = Geometry::singlet(
            millimeter!(50.0),
            millimeter!(f64::INFINITY),
            millimeter!(5.0),
            circle()?,
        )?;
        assert_eq!(geometry.kind().face_count(), 2);
        assert_abs_diff_eq!(
            frame_of(&geometry, Extruded::FRONT)?
                .translation()
                .z
                .get::<millimeter>(),
            0.0
        );
        assert_abs_diff_eq!(
            frame_of(&geometry, Extruded::REAR)?
                .translation()
                .z
                .get::<millimeter>(),
            5.0
        );
        Ok(())
    }
    #[test]
    fn a_wedge_tilts_its_rear_face() -> OpmResult<()> {
        let geometry = Geometry::wedge(millimeter!(10.0), degree!(5.0), circle()?)?;
        let normal =
            frame_of(&geometry, Extruded::REAR)?.transform_vector_f64(&nalgebra::Vector3::z());
        assert_abs_diff_eq!(normal.y, -(5.0_f64.to_radians().sin()), epsilon = 1e-12);
        Ok(())
    }
    /// A geometry moved to a node has every face at the node followed by where the face sits in the
    /// geometry, and every rim along with it.
    #[test]
    fn a_moved_geometry_has_every_face_and_rim_at_its_node() -> OpmResult<()> {
        let node_frame = Isometry::new(millimeter!(1.0, -2.0, 30.0), degree!(10.0, 5.0, 0.0))?;
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let vertex = Isometry::new_along_z(millimeter!(20.0))?;
        let geometries = [
            Geometry::singlet(
                millimeter!(50.0),
                millimeter!(-80.0),
                millimeter!(5.0),
                circle()?,
            )?,
            Geometry::surface(axis, SurfaceShape::Plane, vertex, Some(circle()?))?,
        ];
        for geometry in geometries {
            let moved = geometry.moved(&node_frame);
            assert_eq!(moved.kind().face_count(), geometry.kind().face_count());
            for index in 0..geometry.kind().face_count() {
                assert_eq!(
                    frame_of(&moved, index)?,
                    node_frame.append(&frame_of(&geometry, index)?)
                );
            }
        }
        // The rim moves along: a point on the moved face 12 mm off its axis is still on it.
        let surface = Geometry::surface(axis, SurfaceShape::Plane, vertex, Some(circle()?))?;
        let point = millimeter!(23.0, 0.0, 12.0);
        assert!(surface.kind().includes(SurfaceGeometry::FACE, &point));
        assert!(
            surface
                .moved(&node_frame)
                .kind()
                .includes(SurfaceGeometry::FACE, &node_frame.transform_point(&point))
        );
        Ok(())
    }
    /// Every kind of solid has its faces numbered, encloses a volume and moves its faces along; a
    /// single surface encloses none.
    #[test]
    fn every_kind_of_solid_numbers_its_faces_and_encloses_a_volume() -> OpmResult<()> {
        let parametric = Geometry::Parametric(ParametricSolid::new(vec![(
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Side::Behind,
        )])?);
        let glass = AssetRef::Inline(Material::from(&RefrIndexConst::new(1.5)?));
        let part = Part::new(&Isometry::identity(), parametric.clone(), glass)?;
        let assembly = Geometry::Assembly(Assembly::new(vec![part], vec![])?);
        let frame = Isometry::new(millimeter!(1.0, -2.0, 30.0), degree!(10.0, 5.0, 0.0))?;
        for geometry in [parametric, assembly] {
            assert_eq!(geometry.kind().face_count(), 1);
            assert!(geometry.kind().as_body().is_some());
            assert_eq!(
                frame_of(&geometry.moved(&frame), 0)?,
                frame.append(&frame_of(&geometry, 0)?)
            );
        }
        assert!(Geometry::plane(None).kind().as_body().is_none());
        Ok(())
    }
    /// A curved face has to reach as far out as the outline it is bounded by; a sphere or a
    /// cylinder ends at its radius of curvature. Exactly reaching it (a hemisphere) is still a shape.
    #[test]
    fn an_outline_beyond_the_curvature_is_rejected() -> OpmResult<()> {
        let flat = millimeter!(f64::INFINITY);
        for radius in [10.0, -10.0] {
            let curved = millimeter!(radius);
            for (front, rear) in [(curved, flat), (flat, curved)] {
                let error = Geometry::singlet(front, rear, millimeter!(5.0), circle()?)
                    .expect_err("a curvature tighter than the outline was accepted");
                assert!(error.to_string().contains("does not reach"), "{error}");
            }
            assert!(
                Geometry::cylindrical_singlet(curved, flat, millimeter!(5.0), circle()?).is_err()
            );
            let mirror = |outline| {
                Geometry::surface(
                    Isometry::identity(),
                    SurfaceShape::spherical(curved)?,
                    Isometry::identity(),
                    outline,
                )
            };
            assert!(mirror(Some(circle()?)).is_err());
            // An unbounded surface has no edge to reach.
            assert!(mirror(None).is_ok());
        }
        assert!(Geometry::singlet(millimeter!(12.5), flat, millimeter!(5.0), circle()?).is_ok());
        // A parabola opens without end, so even a short focal length reaches any edge.
        let parabola = SurfaceShape::Parabola(Parabola::new(millimeter!(1.0))?);
        assert!(
            Geometry::surface(
                Isometry::identity(),
                parabola,
                Isometry::identity(),
                Some(circle()?)
            )
            .is_ok()
        );
        Ok(())
    }
    /// A surface with a cross section ends at its edge, which is measured across the axis the cross
    /// section is extruded along: here the node's x axis, so the circle lies in the node's yz plane.
    #[test]
    fn a_surface_ends_at_its_edge_across_its_axis() -> OpmResult<()> {
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let vertex = Isometry::new_along_z(millimeter!(20.0))?;
        let geometry = Geometry::surface(axis, SurfaceShape::Plane, vertex, Some(circle()?))?;
        assert_eq!(
            frame_of(&geometry, SurfaceGeometry::FACE)?,
            axis.append(&vertex)
        );
        for ((x, y, z), included) in [
            ((23.0, 0.0, 0.0), true),
            ((23.0, 0.0, 12.0), true),
            ((23.0, 12.0, 0.0), true),
            ((23.0, 0.0, 13.0), false),
            ((23.0, -13.0, 0.0), false),
        ] {
            assert_eq!(
                geometry
                    .kind()
                    .includes(SurfaceGeometry::FACE, &millimeter!(x, y, z)),
                included,
                "({x}, {y}, {z})"
            );
        }
        assert!(geometry.kind().window().is_none());
        Ok(())
    }
    /// Without a cross section there is nothing to end at, but the face is still stated in the axis
    /// frame.
    #[test]
    fn an_unbounded_surface_is_placed_along_its_axis() -> OpmResult<()> {
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let vertex = Isometry::new_along_z(millimeter!(20.0))?;
        let geometry = Geometry::surface(axis, SurfaceShape::Plane, vertex, None)?;
        assert_eq!(
            frame_of(&geometry, SurfaceGeometry::FACE)?,
            axis.append(&vertex)
        );
        assert!(
            geometry
                .kind()
                .includes(SurfaceGeometry::FACE, &millimeter!(23.0, 1000.0, 1000.0))
        );
        assert!(geometry.kind().window().is_none());
        Ok(())
    }
    /// A detector's cross section is a window: light outside it still passes, it is just not recorded.
    #[test]
    fn a_detector_bounds_its_window_not_its_edge() -> OpmResult<()> {
        let far = millimeter!(1000.0, 1000.0, 0.0);
        let bounded = Geometry::detector(SurfaceShape::Plane, Some(circle()?))?;
        assert_eq!(
            bounded.kind().window(),
            Some(&Rim::new(circle()?, Isometry::identity()))
        );
        assert!(bounded.kind().includes(SurfaceGeometry::FACE, &far));
        let open = Geometry::detector(SurfaceShape::Plane, None)?;
        assert!(open.kind().window().is_none());
        assert!(open.kind().includes(SurfaceGeometry::FACE, &far));
        Ok(())
    }
    /// A surface without a cross section is unbounded, a bounded surface ends at its edge and an
    /// extruded solid at the rim of its profile, stated along the extrusion axis.
    #[test]
    fn only_a_geometry_with_a_cross_section_ends_sideways() -> OpmResult<()> {
        // A point 5 mm and a point a meter off the axis, on the face at the given distance along it.
        let bounded = |geometry: Geometry, along_axis: f64| {
            (
                geometry
                    .kind()
                    .includes(0, &millimeter!(along_axis, 5.0, 0.0)),
                geometry
                    .kind()
                    .includes(0, &millimeter!(along_axis, 1000.0, 1000.0)),
            )
        };
        // The plane faces along z, so its points are given in the xy plane: x off the axis.
        let plane = |geometry: Geometry| {
            (
                geometry.kind().includes(0, &millimeter!(0.0, 5.0, 0.0)),
                geometry
                    .kind()
                    .includes(0, &millimeter!(1000.0, 1000.0, 0.0)),
            )
        };
        assert_eq!(plane(Geometry::plane(None)), (true, true));
        assert_eq!(plane(Geometry::plane(Some(circle()?))), (true, false));
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let extruded = Geometry::Extruded(Box::new(Extruded::new(
            axis,
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Face::new(
                SurfaceShape::Plane,
                Isometry::new_along_z(millimeter!(20.0))?,
            ),
            circle()?,
        )?));
        // Extruded along the node's x axis, the profile reaches around it in y and z; the front face
        // lies at x = 3 mm.
        assert_eq!(bounded(extruded, 3.0), (true, false));
        Ok(())
    }
    #[test]
    fn a_geometry_survives_a_round_trip_through_a_file() -> OpmResult<()> {
        let singlet = Geometry::singlet(
            millimeter!(50.0),
            millimeter!(-80.0),
            millimeter!(5.0),
            circle()?,
        )?;
        let text = ron::to_string(&singlet).map_err(|e| OpossumError::Other(e.to_string()))?;
        let read: Geometry =
            ron::from_str(&text).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(read, singlet);
        Ok(())
    }
}
