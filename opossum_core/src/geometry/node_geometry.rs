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
    /// The face, or `None` if there is no such face. An [`Assembly`] has no faces of its own.
    fn face(&self, index: usize) -> Option<&Face> {
        match self {
            Self::Extruded(extruded) => extruded.face(index),
            Self::Parametric(parametric) => parametric.faces.get(index).map(|(face, _)| face),
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
    /// See [`Solid::face`].
    const fn face(&self, index: usize) -> Option<&Face> {
        match index {
            0 => Some(&self.front),
            1 => Some(&self.rear),
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
/// The shared surface is not stored a second time; the interface only refers to the two faces.
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
    /// * `b` - the other face; it must belong to a different part (see [`Assembly::new`]).
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
    /// This function returns an error if a part is itself an assembly, if an interface refers to a
    /// part or face that does not exist, or if an interface joins two faces of the same part.
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
            for id in [interface.a, interface.b] {
                if parts
                    .get(id.part)
                    .and_then(|part| part.solid.face(id.face))
                    .is_none()
                {
                    return Err(OpossumError::Other(format!(
                        "an interface refers to face {} of part {}, which does not exist",
                        id.face, id.part
                    )));
                }
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
    fn prism_part() -> OpmResult<Part> {
        let solid = Solid::Parametric(ParametricSolid::new(vec![
            (
                Face::new(SurfaceShape::Plane, Isometry::identity()),
                Side::Behind,
            ),
            (
                Face::new(
                    SurfaceShape::Plane,
                    Isometry::new(millimeter!(0.0, 0.0, 10.0), degree!(45.0, 0.0, 0.0))?,
                ),
                Side::InFront,
            ),
        ])?);
        Ok(Part::new(
            Isometry::identity(),
            solid,
            AssetRef::Inline(Material::from(&RefrIndexConst::new(1.5)?)),
        ))
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
        }
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
        let parts = || -> OpmResult<Vec<Part>> { Ok(vec![prism_part()?, prism_part()?]) };
        let joined = Interface::new(FaceId::new(0, 1), FaceId::new(1, 1));
        assert!(Assembly::new(parts()?, vec![joined]).is_ok());
        let no_such_face = Interface::new(FaceId::new(0, 2), FaceId::new(1, 1));
        assert!(Assembly::new(parts()?, vec![no_such_face]).is_err());
        let no_such_part = Interface::new(FaceId::new(0, 1), FaceId::new(2, 1));
        assert!(Assembly::new(parts()?, vec![no_such_part]).is_err());
        let same_part = Interface::new(FaceId::new(0, 0), FaceId::new(0, 1));
        assert!(Assembly::new(parts()?, vec![same_part]).is_err());
        Ok(())
    }
    #[test]
    fn a_doublet_joins_the_rear_of_one_lens_to_the_front_of_the_other() -> OpmResult<()> {
        let lens = |front, rear| -> OpmResult<Part> {
            let Geometry::Solid(solid) =
                Geometry::singlet(front, rear, millimeter!(5.0), circle()?)?
            else {
                unreachable!("a singlet is a solid");
            };
            Ok(Part::new(
                Isometry::identity(),
                solid,
                AssetRef::Inline(Material::from(&RefrIndexConst::new(1.5)?)),
            ))
        };
        let parts = || -> OpmResult<Vec<Part>> {
            Ok(vec![
                lens(millimeter!(60.0), millimeter!(-40.0))?,
                lens(millimeter!(-40.0), millimeter!(f64::INFINITY))?,
            ])
        };
        let cemented = Interface::new(FaceId::new(0, 1), FaceId::new(1, 0));
        assert!(Assembly::new(parts()?, vec![cemented]).is_ok());
        let beyond_the_rear = Interface::new(FaceId::new(0, 2), FaceId::new(1, 0));
        assert!(Assembly::new(parts()?, vec![beyond_the_rear]).is_err());
        Ok(())
    }
    #[test]
    fn an_assembly_cannot_contain_an_assembly() -> OpmResult<()> {
        let inner = Assembly::new(vec![prism_part()?], vec![])?;
        let nested = Part::new(
            Isometry::identity(),
            Solid::Assembly(inner),
            AssetRef::Inline(Material::from(&RefrIndexConst::new(1.5)?)),
        );
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
        let cube = Geometry::Solid(Solid::Assembly(Assembly::new(
            vec![prism_part()?, prism_part()?],
            vec![Interface::new(FaceId::new(0, 1), FaceId::new(1, 1))],
        )?));
        for geometry in [singlet, cube] {
            let text = ron::to_string(&geometry).map_err(|e| OpossumError::Other(e.to_string()))?;
            let read: Geometry =
                ron::from_str(&text).map_err(|e| OpossumError::Other(e.to_string()))?;
            assert_eq!(read, geometry);
        }
        Ok(())
    }
}
