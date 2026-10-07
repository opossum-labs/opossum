#![warn(missing_docs)]
//! The shape of an optical component, described rather than built.
//!
//! A node's properties (curvatures, thickness, clear aperture, ...) are what a user enters; the node
//! derives a [`Geometry`] from them (see
//! [`OpticNode::geometry`](crate::core_optics::OpticNode::geometry)), and the runtime surfaces rays
//! are traced against are built from that description. The node type thereby acts as a preset: a
//! lens is a [`Geometry::singlet`] whose parameters are the lens's properties.
//!
//! The first distinction is whether a component is a single surface ([`Geometry::Surface`]) or
//! encloses a volume ([`Geometry::Solid`]). A [`Solid`] is either two faces capping a profile
//! extruded along an axis ([`Extruded`]), the intersection of the inner sides of several faces
//! ([`ParametricSolid`]) or several solids sharing interfaces ([`Assembly`]).
//!
//! Only surfaces and extruded solids can be traced so far. The other two are described, but
//! everything that would have to trace them returns an error saying so.

use nalgebra::Point3;
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use uom::si::f64::{Angle, Length};

use crate::{
    error::{OpmResult, OpossumError},
    geometry::{
        Rim,
        body::SurfaceBoundedBody,
        face::{Face, SurfaceShape},
        geo_surface::GeoSurfaceRef,
    },
    material::Material,
    properties::proptype::AssetRef,
    types::validated_type_definitions::ValidatedCrossSection,
    utils::geom_transformation::Isometry,
};

/// A geometric surface built from a [`Face`] and its anchor: the surface's own frame relative to
/// the node (see [`Face::place`]).
pub type PlacedSurface = (GeoSurfaceRef, Isometry);

/// The shape of an optical component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Geometry {
    /// A single surface without thickness, such as a thin mirror, a grating or a detector.
    Surface(SurfaceGeometry),
    /// A component enclosing a volume of material.
    Solid(Solid),
}

impl Geometry {
    /// A spherical singlet lens: two spherical (or flat) faces a center thickness apart.
    ///
    /// # Arguments
    ///
    /// * `front_radius` - radius of curvature of the front face; infinite for a flat face.
    /// * `rear_radius` - radius of curvature of the rear face; infinite for a flat face.
    /// * `center_thickness` - distance between the two vertices.
    /// * `cross_section` - the transversal extent (clear aperture).
    ///
    /// # Errors
    ///
    /// This function returns an error if the center thickness is not finite.
    pub fn singlet(
        front_radius: Length,
        rear_radius: Length,
        center_thickness: Length,
        cross_section: ValidatedCrossSection,
    ) -> OpmResult<Self> {
        Ok(Self::two_faced(
            SurfaceShape::spherical(front_radius),
            SurfaceShape::spherical(rear_radius),
            Isometry::new_along_z(center_thickness)?,
            cross_section,
        ))
    }
    /// A cylindrical singlet lens: two cylindrical (or flat) faces a center thickness apart.
    ///
    /// # Arguments
    ///
    /// * `front_radius` - radius of curvature of the front face; infinite for a flat face.
    /// * `rear_radius` - radius of curvature of the rear face; infinite for a flat face.
    /// * `center_thickness` - distance between the two vertices.
    /// * `cross_section` - the transversal extent (clear aperture).
    ///
    /// # Errors
    ///
    /// This function returns an error if the center thickness is not finite.
    pub fn cylindrical_singlet(
        front_radius: Length,
        rear_radius: Length,
        center_thickness: Length,
        cross_section: ValidatedCrossSection,
    ) -> OpmResult<Self> {
        Ok(Self::two_faced(
            SurfaceShape::cylindrical(front_radius),
            SurfaceShape::cylindrical(rear_radius),
            Isometry::new_along_z(center_thickness)?,
            cross_section,
        ))
    }
    /// A wedge: two flat faces, the rear one tilted about the local x axis.
    ///
    /// # Arguments
    ///
    /// * `center_thickness` - distance between the two faces on the axis.
    /// * `wedge_angle` - tilt of the rear face.
    /// * `cross_section` - the transversal extent (clear aperture).
    ///
    /// # Errors
    ///
    /// This function returns an error if the center thickness or the angle is not finite.
    pub fn wedge(
        center_thickness: Length,
        wedge_angle: Angle,
        cross_section: ValidatedCrossSection,
    ) -> OpmResult<Self> {
        let tilt = Isometry::new(
            Point3::origin(),
            Point3::new(wedge_angle, Angle::zero(), Angle::zero()),
        )?;
        Ok(Self::two_faced(
            SurfaceShape::Plane,
            SurfaceShape::Plane,
            Isometry::new_along_z(center_thickness)?.append(&tilt),
            cross_section,
        ))
    }
    /// A single surface.
    ///
    /// # Arguments
    ///
    /// * `shape` - the shape of the surface.
    /// * `vertex` - position and orientation of its vertex relative to the node.
    /// * `cross_section` - its transversal extent, or `None` for an unbounded (virtual) surface.
    #[must_use]
    pub const fn surface(
        shape: SurfaceShape,
        vertex: Isometry,
        cross_section: Option<ValidatedCrossSection>,
    ) -> Self {
        Self::Surface(SurfaceGeometry {
            face: Face::new(shape, vertex),
            cross_section,
        })
    }
    /// A flat surface at the node itself.
    ///
    /// # Arguments
    ///
    /// * `cross_section` - its transversal extent, or `None` for an unbounded (virtual) surface.
    #[must_use]
    pub fn plane(cross_section: Option<ValidatedCrossSection>) -> Self {
        Self::surface(SurfaceShape::Plane, Isometry::identity(), cross_section)
    }
    /// An extruded solid along the node's own axis whose front vertex sits at the node.
    ///
    /// # Arguments
    ///
    /// * `front` - the shape of the front face.
    /// * `rear` - the shape of the rear face.
    /// * `rear_vertex` - position and orientation of the rear face's vertex relative to the node.
    /// * `cross_section` - the transversal extent (clear aperture).
    fn two_faced(
        front: SurfaceShape,
        rear: SurfaceShape,
        rear_vertex: Isometry,
        cross_section: ValidatedCrossSection,
    ) -> Self {
        Self::Solid(Solid::Extruded(Extruded::new(
            Isometry::identity(),
            Face::new(front, Isometry::identity()),
            Face::new(rear, rear_vertex),
            cross_section,
        )))
    }
    /// Build the surfaces light enters and leaves through.
    ///
    /// A single surface is both: the same geometric surface is returned twice, so that every port
    /// of the node shares it.
    ///
    /// # Arguments
    ///
    /// * `node_frame` - the placement of the node.
    ///
    /// # Returns
    ///
    /// The entrance and the exit surface, each with its anchor relative to the node.
    ///
    /// # Errors
    ///
    /// This function returns an error if a face cannot be built or if the geometry is not traced
    /// through an entrance and an exit surface yet ([`ParametricSolid`], [`Assembly`]).
    pub fn entrance_and_exit(
        &self,
        node_frame: &Isometry,
    ) -> OpmResult<(PlacedSurface, PlacedSurface)> {
        match self {
            Self::Surface(surface) => {
                let placed = surface.face.place(node_frame, &Isometry::identity())?;
                Ok((placed.clone(), placed))
            }
            Self::Solid(solid) => solid.entrance_and_exit(node_frame),
        }
    }
    /// Build the body of material this geometry encloses.
    ///
    /// The body gets surfaces of its own, built from the same description as the ones light is
    /// traced at (see [`Geometry::entrance_and_exit`]), and the cross section as its lateral bound.
    ///
    /// # Arguments
    ///
    /// * `node_frame` - the placement of the node.
    ///
    /// # Returns
    ///
    /// The body, placed at the node's frame followed by the extrusion axis.
    ///
    /// # Errors
    ///
    /// This function returns an error for a single surface, which encloses no volume, if a face
    /// cannot be built, or if the solid cannot be traced yet ([`ParametricSolid`], [`Assembly`]).
    pub fn body(&self, node_frame: &Isometry) -> OpmResult<SurfaceBoundedBody> {
        match self {
            Self::Surface(_) => Err(OpossumError::Other(
                "a single surface encloses no volume".into(),
            )),
            Self::Solid(Solid::Extruded(extruded)) => extruded.body(node_frame),
            Self::Solid(Solid::Parametric(_)) => Err(not_supported_yet("a parametric solid")),
            Self::Solid(Solid::Assembly(_)) => Err(not_supported_yet("an assembly")),
        }
    }
    /// Return the lateral boundary of this geometry, stated relative to the node.
    ///
    /// # Returns
    ///
    /// The [`Rim`] of an extruded solid or of a bounded surface, or `None` for a surface without
    /// a cross section, which is unbounded.
    ///
    /// # Errors
    ///
    /// This function returns an error for a solid that cannot be traced yet ([`ParametricSolid`],
    /// [`Assembly`]): `None` would declare it unbounded.
    pub fn rim(&self) -> OpmResult<Option<Rim>> {
        match self {
            Self::Surface(surface) => Ok(surface
                .cross_section
                .clone()
                .map(|cross_section| Rim::new(cross_section, Isometry::identity()))),
            Self::Solid(Solid::Extruded(extruded)) => Ok(Some(Rim::new(
                extruded.cross_section.clone(),
                extruded.axis,
            ))),
            Self::Solid(Solid::Parametric(_)) => Err(not_supported_yet("a parametric solid")),
            Self::Solid(Solid::Assembly(_)) => Err(not_supported_yet("an assembly")),
        }
    }
}

/// A single surface without thickness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceGeometry {
    face: Face,
    cross_section: Option<ValidatedCrossSection>,
}

/// A component enclosing a volume of material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Solid {
    /// Two faces capping a profile extruded along an axis.
    Extruded(Extruded),
    /// The intersection of the inner sides of several faces.
    Parametric(ParametricSolid),
    /// Several solids sharing interfaces.
    Assembly(Assembly),
}

impl Solid {
    /// Return the face with the given index, as an [`Interface`] refers to it.
    ///
    /// # Arguments
    ///
    /// * `index` - for an [`Extruded`] solid 0 is the front and 1 the rear face; for a
    ///   [`ParametricSolid`] it is the position in its list of faces.
    ///
    /// # Returns
    ///
    /// The face, the frame it is stated in relative to the solid, and the side of it the solid lies
    /// on; or `None` if there is no such face. An [`Assembly`] has no faces of its own.
    fn face(&self, index: usize) -> Option<(&Face, Isometry, Side)> {
        match self {
            Self::Extruded(extruded) => extruded.face(index),
            Self::Parametric(parametric) => parametric
                .faces
                .get(index)
                .map(|(face, side)| (face, Isometry::identity(), *side)),
            Self::Assembly(_) => None,
        }
    }
    /// See [`Geometry::entrance_and_exit`].
    fn entrance_and_exit(
        &self,
        node_frame: &Isometry,
    ) -> OpmResult<(PlacedSurface, PlacedSurface)> {
        match self {
            Self::Extruded(extruded) => extruded.entrance_and_exit(node_frame),
            Self::Parametric(_) => Err(not_supported_yet("a parametric solid")),
            Self::Assembly(_) => Err(not_supported_yet("an assembly")),
        }
    }
}

/// Two faces capping a profile that is extruded along an axis.
///
/// The profile is the cross section, stated in the plane perpendicular to the axis; it is also the
/// component's clear aperture. The center thickness is not stored separately: it is where the rear
/// face's vertex sits, together with any tilt of that face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extruded {
    axis: Isometry,
    front: Face,
    rear: Face,
    cross_section: ValidatedCrossSection,
}

impl Extruded {
    /// Create a new [`Extruded`] solid.
    ///
    /// # Arguments
    ///
    /// * `axis` - the frame of the extrusion relative to the node; its z axis is the extrusion
    ///   direction. The identity for a component used along its own axis, such as a lens.
    /// * `front` - the face capping the profile towards -z, stated in the `axis` frame.
    /// * `rear` - the face capping the profile towards +z, stated in the `axis` frame.
    /// * `cross_section` - the profile, in the xy plane of the `axis` frame.
    #[must_use]
    pub const fn new(
        axis: Isometry,
        front: Face,
        rear: Face,
        cross_section: ValidatedCrossSection,
    ) -> Self {
        Self {
            axis,
            front,
            rear,
            cross_section,
        }
    }
    /// See [`Geometry::body`].
    fn body(&self, node_frame: &Isometry) -> OpmResult<SurfaceBoundedBody> {
        let ((front, _), (rear, _)) = self.entrance_and_exit(node_frame)?;
        Ok(SurfaceBoundedBody::new(
            front,
            rear,
            self.cross_section.clone(),
            node_frame.append(&self.axis),
        ))
    }
    /// See [`Solid::face`]. The profile lies behind the front face and in front of the rear face.
    const fn face(&self, index: usize) -> Option<(&Face, Isometry, Side)> {
        match index {
            0 => Some((&self.front, self.axis, Side::Behind)),
            1 => Some((&self.rear, self.axis, Side::InFront)),
            _ => None,
        }
    }
    /// See [`Geometry::entrance_and_exit`].
    fn entrance_and_exit(
        &self,
        node_frame: &Isometry,
    ) -> OpmResult<(PlacedSurface, PlacedSurface)> {
        Ok((
            self.front.place(node_frame, &self.axis)?,
            self.rear.place(node_frame, &self.axis)?,
        ))
    }
}

/// Which side of a face lies inside a [`ParametricSolid`], as
/// [`GeoSurface::is_behind`](crate::geometry::geo_surface::GeoSurface::is_behind) tells them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    /// The side the surface reports as behind it.
    Behind,
    /// The side the surface reports as in front of it.
    InFront,
}

/// A solid bounded by several faces: the intersection of the inner sides of all of them.
///
/// Built from flat faces only, such a solid is always convex (prisms, corner cubes, roof prisms);
/// curved faces with their outside as inner side also allow concave shapes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParametricSolid {
    faces: Vec<(Face, Side)>,
}

impl ParametricSolid {
    /// Create a new [`ParametricSolid`].
    ///
    /// # Arguments
    ///
    /// * `faces` - every bounding face, stated relative to the node, with the side that is inside.
    ///
    /// # Errors
    ///
    /// This function returns an error if no face is given.
    pub fn new(faces: Vec<(Face, Side)>) -> OpmResult<Self> {
        if faces.is_empty() {
            return Err(OpossumError::Other(
                "a parametric solid needs at least one bounding face".into(),
            ));
        }
        Ok(Self { faces })
    }
}

/// One solid of an [`Assembly`], with its placement and material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    placement: Isometry,
    solid: Solid,
    material: AssetRef<Material>,
}

impl Part {
    /// Create a new [`Part`].
    ///
    /// # Arguments
    ///
    /// * `placement` - where the solid sits relative to the node.
    /// * `solid` - the solid itself; it must not be an [`Assembly`] (see [`Assembly::new`]).
    /// * `material` - the material the solid is made of.
    #[must_use]
    pub const fn new(placement: Isometry, solid: Solid, material: AssetRef<Material>) -> Self {
        Self {
            placement,
            solid,
            material,
        }
    }
    /// Return a face of this part's solid, placed with the part.
    ///
    /// # Arguments
    ///
    /// * `index` - the index of the face in the part's solid (see [`FaceId::new`]).
    ///
    /// # Returns
    ///
    /// The face, the frame it is stated in relative to the node, and the side of it the part lies
    /// on; or `None` if the solid has no such face.
    fn face(&self, index: usize) -> Option<(&Face, Isometry, Side)> {
        self.solid
            .face(index)
            .map(|(face, owner, side)| (face, self.placement.append(&owner), side))
    }
}

/// Refers to one face of one part of an [`Assembly`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceId {
    part: usize,
    face: usize,
}

impl FaceId {
    /// Create a new [`FaceId`].
    ///
    /// # Arguments
    ///
    /// * `part` - the index of the part in the assembly.
    /// * `face` - the index of the face in that part's solid (see [`Solid`]'s face numbering: 0 and
    ///   1 are front and rear of an [`Extruded`] solid, a [`ParametricSolid`] counts its faces).
    #[must_use]
    pub const fn new(part: usize, face: usize) -> Self {
        Self { part, face }
    }
}

/// Two faces of different parts of an [`Assembly`] lying on each other, such as the cemented
/// surface of a doublet or the diagonal of a beam splitter cube.
///
/// The shared surface belongs to both parts: each states it among its own faces, and the interface
/// links the two. They have to describe the same surface at the same place, with the two parts on
/// opposite sides of it (see [`Assembly::new`]). Light crosses the interface only where both parts
/// reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interface {
    a: FaceId,
    b: FaceId,
}

impl Interface {
    /// Create a new [`Interface`].
    ///
    /// # Arguments
    ///
    /// * `a` - one of the two faces.
    /// * `b` - the other face; it must belong to a different part and lie on `a` (see
    ///   [`Assembly::new`]).
    #[must_use]
    pub const fn new(a: FaceId, b: FaceId) -> Self {
        Self { a, b }
    }
}

/// Several solids, each with its own material, sharing interfaces.
///
/// Covers cemented components (doublet, beam splitter cube), components with an air gap between
/// their parts, and bonded crystals with undoped end caps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assembly {
    parts: Vec<Part>,
    interfaces: Vec<Interface>,
}

impl Assembly {
    /// Create a new [`Assembly`].
    ///
    /// # Arguments
    ///
    /// * `parts` - the solids the assembly is made of.
    /// * `interfaces` - the faces of different parts that lie on each other.
    ///
    /// # Errors
    ///
    /// This function returns an error if
    /// - a part is itself an assembly,
    /// - an interface refers to a part or face that does not exist or joins two faces of the same
    ///   part,
    /// - the two faces of an interface are not the same surface at the same place (see
    ///   [`Face::coincides_with`]), or the two parts lie on the same side of it.
    pub fn new(parts: Vec<Part>, interfaces: Vec<Interface>) -> OpmResult<Self> {
        if let Some(index) = parts
            .iter()
            .position(|part| matches!(part.solid, Solid::Assembly(_)))
        {
            return Err(OpossumError::Other(format!(
                "part {index} of an assembly is an assembly itself; add its parts directly"
            )));
        }
        for interface in &interfaces {
            if interface.a.part == interface.b.part {
                return Err(OpossumError::Other(format!(
                    "an interface must join two different parts, not part {} with itself",
                    interface.a.part
                )));
            }
            let resolve = |id: FaceId| {
                parts
                    .get(id.part)
                    .and_then(|part| part.face(id.face))
                    .ok_or_else(|| {
                        OpossumError::Other(format!(
                            "an interface refers to face {} of part {}, which does not exist",
                            id.face, id.part
                        ))
                    })
            };
            let (face_a, frame_a, side_a) = resolve(interface.a)?;
            let (face_b, frame_b, side_b) = resolve(interface.b)?;
            if !face_a.coincides_with(&frame_a, face_b, &frame_b) {
                return Err(OpossumError::Other(format!(
                    "the faces an interface joins have to be the same surface at the same place, \
                     but face {} of part {} and face {} of part {} are not",
                    interface.a.face, interface.a.part, interface.b.face, interface.b.part
                )));
            }
            if side_a == side_b {
                return Err(OpossumError::Other(format!(
                    "the parts an interface joins have to lie on opposite sides of it, but parts {} \
                     and {} lie on the same side",
                    interface.a.part, interface.b.part
                )));
            }
        }
        Ok(Self { parts, interfaces })
    }
}

/// The error for a geometry that is described but cannot be traced yet.
fn not_supported_yet(what: &str) -> OpossumError {
    OpossumError::Other(format!(
        "{what} is not supported yet: it cannot be traced through an entrance and an exit surface"
    ))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Aperture, ApertureType, CircleShape},
        degree,
        geometry::body::Body,
        millimeter,
        refractive_index::RefrIndexConst,
    };
    use approx::assert_abs_diff_eq;
    use std::sync::Arc;
    use uom::si::length::millimeter;

    fn circle() -> OpmResult<ValidatedCrossSection> {
        ValidatedCrossSection::try_new(Aperture::new(
            CircleShape::new(millimeter!(12.5))?.into(),
            ApertureType::Hole,
            None,
            None,
        )?)
    }
    /// A part made of glass with a refractive index of 1.5.
    fn glass_part(placement: Isometry, solid: Solid) -> OpmResult<Part> {
        Ok(Part::new(
            placement,
            solid,
            AssetRef::Inline(Material::from(&RefrIndexConst::new(1.5)?)),
        ))
    }
    /// The solid of the given geometry as a glass part at the given placement.
    fn part_of(geometry: Geometry, placement: Isometry) -> OpmResult<Part> {
        let Geometry::Solid(solid) = geometry else {
            unreachable!("only solids are made into parts");
        };
        glass_part(placement, solid)
    }
    /// The diagonal of a beam splitter cube: tilted by 45° about x, 10 mm along z.
    fn diagonal() -> OpmResult<Face> {
        Ok(Face::new(
            SurfaceShape::Plane,
            Isometry::new(millimeter!(0.0, 0.0, 10.0), degree!(45.0, 0.0, 0.0))?,
        ))
    }
    /// The front prism of a beam splitter cube: behind its entrance face, in front of the
    /// diagonal (face 1).
    fn prism_part() -> OpmResult<Part> {
        let solid = Solid::Parametric(ParametricSolid::new(vec![
            (
                Face::new(SurfaceShape::Plane, Isometry::identity()),
                Side::Behind,
            ),
            (diagonal()?, Side::InFront),
        ])?);
        glass_part(Isometry::identity(), solid)
    }
    /// The rear prism of the same cube: behind the diagonal (face 0), in front of its exit face.
    fn opposite_prism_part() -> OpmResult<Part> {
        let solid = Solid::Parametric(ParametricSolid::new(vec![
            (diagonal()?, Side::Behind),
            (
                Face::new(
                    SurfaceShape::Plane,
                    Isometry::new_along_z(millimeter!(20.0))?,
                ),
                Side::InFront,
            ),
        ])?);
        glass_part(Isometry::identity(), solid)
    }
    /// A beam splitter cube: both prisms cemented along the diagonal.
    fn cube() -> OpmResult<Assembly> {
        Assembly::new(
            vec![prism_part()?, opposite_prism_part()?],
            vec![Interface::new(FaceId::new(0, 1), FaceId::new(1, 0))],
        )
    }
    /// The crown lens of a doublet at the origin, its rear face (R = -40 mm) 5 mm behind it.
    fn crown_part() -> OpmResult<Part> {
        part_of(
            Geometry::singlet(
                millimeter!(60.0),
                millimeter!(-40.0),
                millimeter!(5.0),
                circle()?,
            )?,
            Isometry::identity(),
        )
    }
    /// The flint lens of a doublet with the given front radius, at the given placement.
    fn flint_part(front_radius: Length, placement: Isometry) -> OpmResult<Part> {
        part_of(
            Geometry::singlet(
                front_radius,
                millimeter!(f64::INFINITY),
                millimeter!(5.0),
                circle()?,
            )?,
            placement,
        )
    }
    #[test]
    fn a_surface_is_entrance_and_exit_at_once() -> OpmResult<()> {
        let ((entrance, _), (exit, _)) =
            Geometry::plane(None).entrance_and_exit(&Isometry::identity())?;
        assert!(Arc::ptr_eq(&entrance.0, &exit.0));
        Ok(())
    }
    #[test]
    fn a_singlet_has_its_rear_vertex_one_center_thickness_behind_the_front() -> OpmResult<()> {
        let geometry = Geometry::singlet(
            millimeter!(50.0),
            millimeter!(f64::INFINITY),
            millimeter!(5.0),
            circle()?,
        )?;
        let ((_, front), (_, rear)) = geometry.entrance_and_exit(&Isometry::identity())?;
        assert_abs_diff_eq!(front.translation().z.get::<millimeter>(), 50.0);
        assert_abs_diff_eq!(rear.translation().z.get::<millimeter>(), 5.0);
        Ok(())
    }
    #[test]
    fn a_wedge_tilts_its_rear_face() -> OpmResult<()> {
        let geometry = Geometry::wedge(millimeter!(10.0), degree!(5.0), circle()?)?;
        let (_, (_, rear)) = geometry.entrance_and_exit(&Isometry::identity())?;
        let normal = rear.transform_vector_f64(&nalgebra::Vector3::z());
        assert_abs_diff_eq!(normal.y, -(5.0_f64.to_radians().sin()), epsilon = 1e-12);
        Ok(())
    }
    #[test]
    fn an_extruded_solid_places_its_faces_along_its_axis() -> OpmResult<()> {
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let extruded = Extruded::new(
            axis,
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Face::new(
                SurfaceShape::Plane,
                Isometry::new_along_z(millimeter!(20.0))?,
            ),
            circle()?,
        );
        let ((_, front), (_, rear)) =
            Geometry::Solid(Solid::Extruded(extruded)).entrance_and_exit(&Isometry::identity())?;
        assert_eq!(front, axis);
        assert_eq!(
            rear,
            axis.append(&Isometry::new_along_z(millimeter!(20.0))?)
        );
        Ok(())
    }
    #[test]
    fn solids_that_cannot_be_traced_yet_say_so() -> OpmResult<()> {
        let parametric = Geometry::Solid(prism_part()?.solid);
        let assembly =
            Geometry::Solid(Solid::Assembly(Assembly::new(vec![prism_part()?], vec![])?));
        for geometry in [parametric, assembly] {
            let error = geometry
                .entrance_and_exit(&Isometry::identity())
                .unwrap_err();
            assert!(error.to_string().contains("not supported yet"));
            let error = geometry.body(&Isometry::identity()).unwrap_err();
            assert!(error.to_string().contains("not supported yet"));
            let error = geometry.rim().unwrap_err();
            assert!(error.to_string().contains("not supported yet"));
        }
        Ok(())
    }
    /// A surface without a cross section is unbounded; a bounded surface and an extruded solid have
    /// a rim, stated along the extrusion axis.
    #[test]
    fn rim_is_none_for_a_virtual_plane() -> OpmResult<()> {
        assert_eq!(Geometry::plane(None).rim()?, None);
        assert_eq!(
            Geometry::plane(Some(circle()?)).rim()?,
            Some(Rim::new(circle()?, Isometry::identity()))
        );
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let extruded = Geometry::Solid(Solid::Extruded(Extruded::new(
            axis,
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Face::new(
                SurfaceShape::Plane,
                Isometry::new_along_z(millimeter!(20.0))?,
            ),
            circle()?,
        )));
        assert_eq!(extruded.rim()?, Some(Rim::new(circle()?, axis)));
        Ok(())
    }
    #[test]
    fn a_single_surface_encloses_no_volume() {
        assert!(Geometry::plane(None).body(&Isometry::identity()).is_err());
    }
    #[test]
    fn an_extruded_body_is_placed_along_its_axis() -> OpmResult<()> {
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let extruded = Extruded::new(
            axis,
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Face::new(
                SurfaceShape::Plane,
                Isometry::new_along_z(millimeter!(20.0))?,
            ),
            circle()?,
        );
        let node_frame = Isometry::new(millimeter!(0.0, 5.0, 0.0), degree!(0.0, 0.0, 0.0))?;
        let body = Geometry::Solid(Solid::Extruded(extruded)).body(&node_frame)?;
        assert_eq!(*body.isometry(), node_frame.append(&axis));
        // halfway along the extrusion axis, which runs along the node's x axis
        let halfway = node_frame
            .append(&axis)
            .transform_point(&millimeter!(0.0, 0.0, 10.0));
        assert!(body.contains(&halfway)?);
        Ok(())
    }
    #[test]
    fn a_parametric_solid_needs_a_face() {
        assert!(ParametricSolid::new(vec![]).is_err());
    }
    #[test]
    fn an_assembly_checks_its_interfaces() -> OpmResult<()> {
        let parts = || -> OpmResult<Vec<Part>> { Ok(vec![prism_part()?, opposite_prism_part()?]) };
        let joined = Interface::new(FaceId::new(0, 1), FaceId::new(1, 0));
        assert!(Assembly::new(parts()?, vec![joined]).is_ok());
        let no_such_face = Interface::new(FaceId::new(0, 2), FaceId::new(1, 0));
        assert!(Assembly::new(parts()?, vec![no_such_face]).is_err());
        let no_such_part = Interface::new(FaceId::new(0, 1), FaceId::new(2, 0));
        assert!(Assembly::new(parts()?, vec![no_such_part]).is_err());
        let same_part = Interface::new(FaceId::new(0, 0), FaceId::new(0, 1));
        assert!(Assembly::new(parts()?, vec![same_part]).is_err());
        Ok(())
    }
    /// Two parts on the same side of their interface overlap instead of meeting there - the same
    /// prism twice, for instance.
    #[test]
    fn the_parts_of_an_interface_lie_on_opposite_sides_of_it() -> OpmResult<()> {
        let same_side = Interface::new(FaceId::new(0, 1), FaceId::new(1, 1));
        assert!(Assembly::new(vec![prism_part()?, prism_part()?], vec![same_side]).is_err());
        Ok(())
    }
    #[test]
    fn a_doublet_joins_the_rear_of_one_lens_to_the_front_of_the_other() -> OpmResult<()> {
        let behind_the_crown = Isometry::new_along_z(millimeter!(5.0))?;
        let parts = || -> OpmResult<Vec<Part>> {
            Ok(vec![
                crown_part()?,
                flint_part(millimeter!(-40.0), behind_the_crown)?,
            ])
        };
        let cemented = Interface::new(FaceId::new(0, 1), FaceId::new(1, 0));
        assert!(Assembly::new(parts()?, vec![cemented]).is_ok());
        let beyond_the_rear = Interface::new(FaceId::new(0, 2), FaceId::new(1, 0));
        assert!(Assembly::new(parts()?, vec![beyond_the_rear]).is_err());
        Ok(())
    }
    /// The cemented faces of a doublet have to be the same surface at the same place.
    #[test]
    fn a_doublet_whose_cemented_faces_do_not_meet_is_rejected() -> OpmResult<()> {
        let cemented = Interface::new(FaceId::new(0, 1), FaceId::new(1, 0));
        // Both lenses at the origin overlap, and the flint's front lies 5 mm before the crown's rear.
        let overlapping = flint_part(millimeter!(-40.0), Isometry::identity())?;
        let with_a_gap = flint_part(millimeter!(-40.0), Isometry::new_along_z(millimeter!(5.1))?)?;
        let other_radius =
            flint_part(millimeter!(-41.0), Isometry::new_along_z(millimeter!(5.0))?)?;
        for flint in [overlapping, with_a_gap, other_radius] {
            assert!(Assembly::new(vec![crown_part()?, flint], vec![cemented]).is_err());
        }
        Ok(())
    }
    /// A cylindrical face is curved in one direction only, so both parts have to agree on it. Turned
    /// by half a turn about its axis it is the very same surface again.
    #[test]
    fn a_cylindrical_interface_has_to_agree_in_its_orientation() -> OpmResult<()> {
        let crown = || {
            part_of(
                Geometry::cylindrical_singlet(
                    millimeter!(60.0),
                    millimeter!(-40.0),
                    millimeter!(5.0),
                    circle()?,
                )?,
                Isometry::identity(),
            )
        };
        let flint = |roll: Angle| {
            part_of(
                Geometry::cylindrical_singlet(
                    millimeter!(-40.0),
                    millimeter!(f64::INFINITY),
                    millimeter!(5.0),
                    circle()?,
                )?,
                Isometry::new(
                    millimeter!(0.0, 0.0, 5.0),
                    Point3::new(Angle::zero(), Angle::zero(), roll),
                )?,
            )
        };
        let cemented = Interface::new(FaceId::new(0, 1), FaceId::new(1, 0));
        for (roll, coincides) in [(0.0, true), (180.0, true), (90.0, false)] {
            assert_eq!(
                Assembly::new(vec![crown()?, flint(degree!(roll))?], vec![cemented]).is_ok(),
                coincides,
                "flint turned by {roll}°"
            );
        }
        Ok(())
    }
    #[test]
    fn an_assembly_cannot_contain_an_assembly() -> OpmResult<()> {
        let inner = Assembly::new(vec![prism_part()?], vec![])?;
        let nested = glass_part(Isometry::identity(), Solid::Assembly(inner))?;
        assert!(Assembly::new(vec![nested], vec![]).is_err());
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
        let splitter_cube = Geometry::Solid(Solid::Assembly(cube()?));
        for geometry in [singlet, splitter_cube] {
            let text = ron::to_string(&geometry).map_err(|e| OpossumError::Other(e.to_string()))?;
            let read: Geometry =
                ron::from_str(&text).map_err(|e| OpossumError::Other(e.to_string()))?;
            assert_eq!(read, geometry);
        }
        Ok(())
    }
}
