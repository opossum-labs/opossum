#![warn(missing_docs)]
//! The shape of an optical component, described rather than built.
//!
//! A node's properties (curvatures, thickness, optic shape, ...) are what a user enters; the node
//! derives a [`Geometry`] from them (see
//! [`OpticNode::geometry`](crate::core_optics::OpticNode::geometry)), and the runtime surfaces rays
//! are traced against are built from that description. The node type thereby acts as a preset: a
//! lens is a [`Geometry::singlet`] whose parameters are the lens's properties.
//!
//! The first distinction is whether a component is a single surface ([`Geometry::Surface`]) or
//! encloses a volume ([`Geometry::Solid`]). The kinds of [`Solid`] are described next to it, one
//! file each.
//!
//! Only surfaces and extruded solids can be traced so far. The other two kinds of solid are
//! described, but everything that would have to trace them returns an error saying so.

use nalgebra::Point3;
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use uom::si::f64::{Angle, Length};

use crate::{
    error::{OpmResult, OpossumError},
    geometry::{
        Extruded, Placeable, Placed, Rim, RimRole, Solid,
        face::{Face, SurfaceShape},
    },
    types::validated_type_definitions::ValidatedCrossSection,
    utils::geom_transformation::Isometry,
};

/// The shape of an optical component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Geometry {
    /// A single surface without thickness, such as a thin mirror, a grating or a detector.
    Surface(Box<SurfaceGeometry>),
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
    /// * `cross_section` - the transversal extent (optic shape).
    ///
    /// # Errors
    ///
    /// This function returns an error if the center thickness is not finite or if a face is curved
    /// too tightly to reach the edge of the cross section.
    pub fn singlet(
        front_radius: Length,
        rear_radius: Length,
        center_thickness: Length,
        cross_section: ValidatedCrossSection,
    ) -> OpmResult<Self> {
        Self::two_faced(
            SurfaceShape::spherical(front_radius),
            SurfaceShape::spherical(rear_radius),
            Isometry::new_along_z(center_thickness)?,
            cross_section,
        )
    }
    /// A cylindrical singlet lens: two cylindrical (or flat) faces a center thickness apart.
    ///
    /// # Arguments
    ///
    /// * `front_radius` - radius of curvature of the front face; infinite for a flat face.
    /// * `rear_radius` - radius of curvature of the rear face; infinite for a flat face.
    /// * `center_thickness` - distance between the two vertices.
    /// * `cross_section` - the transversal extent (optic shape).
    ///
    /// # Errors
    ///
    /// This function returns an error if the center thickness is not finite or if a face is curved
    /// too tightly to reach the edge of the cross section.
    pub fn cylindrical_singlet(
        front_radius: Length,
        rear_radius: Length,
        center_thickness: Length,
        cross_section: ValidatedCrossSection,
    ) -> OpmResult<Self> {
        Self::two_faced(
            SurfaceShape::cylindrical(front_radius),
            SurfaceShape::cylindrical(rear_radius),
            Isometry::new_along_z(center_thickness)?,
            cross_section,
        )
    }
    /// A wedge: two flat faces, the rear one tilted about the local x axis.
    ///
    /// # Arguments
    ///
    /// * `center_thickness` - distance between the two faces on the axis.
    /// * `wedge_angle` - tilt of the rear face.
    /// * `cross_section` - the transversal extent (optic shape).
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
        Self::two_faced(
            SurfaceShape::Plane,
            SurfaceShape::Plane,
            Isometry::new_along_z(center_thickness)?.append(&tilt),
            cross_section,
        )
    }
    /// A single surface.
    ///
    /// # Arguments
    ///
    /// * `axis` - the frame of the cross section relative to the node; its z axis is the direction
    ///   the cross section is projected along. The identity for a surface used along its own axis.
    /// * `shape` - the shape of the surface.
    /// * `vertex` - position and orientation of its vertex, stated in the `axis` frame.
    /// * `cross_section` - its transversal extent in the xy plane of the `axis` frame, or `None`
    ///   for an unbounded (virtual) surface.
    ///
    /// # Errors
    ///
    /// This function returns an error if the surface is curved too tightly to reach the edge of the
    /// cross section, or if the outline of the cross section cannot be determined.
    pub fn surface(
        axis: Isometry,
        shape: SurfaceShape,
        vertex: Isometry,
        cross_section: Option<ValidatedCrossSection>,
    ) -> OpmResult<Self> {
        Ok(Self::Surface(Box::new(SurfaceGeometry::new(
            axis,
            shape,
            vertex,
            cross_section,
            RimRole::Edge,
        )?)))
    }
    /// A detector: a surface at the node that records the light within its cross section and lets
    /// all of it pass (see [`RimRole::Window`]).
    ///
    /// # Arguments
    ///
    /// * `shape` - the shape of the surface the light is recorded on.
    /// * `cross_section` - the window the light is recorded within, or `None` to record all of it.
    ///
    /// # Errors
    ///
    /// See [`Geometry::surface`].
    pub fn detector(
        shape: SurfaceShape,
        cross_section: Option<ValidatedCrossSection>,
    ) -> OpmResult<Self> {
        Ok(Self::Surface(Box::new(SurfaceGeometry::new(
            Isometry::identity(),
            shape,
            Isometry::identity(),
            cross_section,
            RimRole::Window,
        )?)))
    }
    /// A flat surface at the node itself.
    ///
    /// # Arguments
    ///
    /// * `cross_section` - its transversal extent, or `None` for an unbounded (virtual) surface.
    #[must_use]
    pub fn plane(cross_section: Option<ValidatedCrossSection>) -> Self {
        // A plane reaches every cross section, so nothing is left to check.
        Self::Surface(Box::new(SurfaceGeometry {
            face: Face::new(SurfaceShape::Plane, Isometry::identity()),
            rim: cross_section
                .map(|cross_section| Rim::new(cross_section, Isometry::identity(), RimRole::Edge)),
        }))
    }
    /// An extruded solid along the node's own axis whose front vertex sits at the node.
    ///
    /// # Arguments
    ///
    /// * `front` - the shape of the front face.
    /// * `rear` - the shape of the rear face.
    /// * `rear_vertex` - position and orientation of the rear face's vertex relative to the node.
    /// * `cross_section` - the transversal extent (optic shape).
    ///
    /// # Errors
    ///
    /// See [`Extruded::new`].
    fn two_faced(
        front: SurfaceShape,
        rear: SurfaceShape,
        rear_vertex: Isometry,
        cross_section: ValidatedCrossSection,
    ) -> OpmResult<Self> {
        Ok(Self::Solid(Solid::Extruded(Box::new(Extruded::new(
            Isometry::identity(),
            Face::new(front, Isometry::identity()),
            Face::new(rear, rear_vertex),
            cross_section,
        )?))))
    }
    /// Build this geometry at the placement of its node, for tracing rays against it.
    ///
    /// # Arguments
    ///
    /// * `node_frame` - the placement of the node.
    ///
    /// # Errors
    ///
    /// This function returns an error if a face cannot be built or if the geometry cannot be traced
    /// yet ([`ParametricSolid`], [`Assembly`]).
    pub fn place(&self, node_frame: &Isometry) -> OpmResult<Placed<Self>> {
        match self {
            Self::Solid(Solid::Parametric(_)) => Err(not_supported_yet("a parametric solid")),
            Self::Solid(Solid::Assembly(_)) => Err(not_supported_yet("an assembly")),
            Self::Surface(_) | Self::Solid(Solid::Extruded(_)) => {
                Placed::new(self.clone(), node_frame)
            }
        }
    }
    /// Return the kind of geometry this is, as the one thing the faces and the rim are asked of.
    fn as_placeable(&self) -> &dyn Placeable {
        match self {
            Self::Surface(surface) => &**surface,
            Self::Solid(solid) => solid,
        }
    }
}

impl Placeable for Geometry {
    fn faces(&self) -> Vec<(&Face, Isometry)> {
        self.as_placeable().faces()
    }
    fn rim(&self) -> Option<&Rim> {
        self.as_placeable().rim()
    }
}

/// A single surface without thickness.
///
/// The face is stated relative to the node. The rim, if there is one, keeps the axis its cross
/// section is extruded along; an off-axis parabola uses it to measure its optic shape parallel to
/// the axis of its parent parabola. A surface without a cross section has no rim and so no axis
/// of its own, which is why the axis is already applied to the face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceGeometry {
    face: Face,
    rim: Option<Rim>,
}

impl SurfaceGeometry {
    /// Create a new [`SurfaceGeometry`].
    ///
    /// # Arguments
    ///
    /// * `axis` - the frame of the cross section relative to the node.
    /// * `shape` - the shape of the surface.
    /// * `vertex` - position and orientation of its vertex, stated in the `axis` frame.
    /// * `cross_section` - its transversal extent, or `None` for an unbounded surface.
    /// * `role` - what the edge of the cross section means for a ray outside it.
    ///
    /// # Errors
    ///
    /// This function returns an error if the surface is curved too tightly to reach the edge of the
    /// cross section, or if the outline of the cross section cannot be determined.
    fn new(
        axis: Isometry,
        shape: SurfaceShape,
        vertex: Isometry,
        cross_section: Option<ValidatedCrossSection>,
        role: RimRole,
    ) -> OpmResult<Self> {
        let face = Face::new(shape, axis.append(&vertex));
        if let Some(cross_section) = &cross_section {
            face.check_reach(cross_section.transversal_reach(), "surface")?;
        }
        Ok(Self {
            face,
            rim: cross_section.map(|cross_section| Rim::new(cross_section, axis, role)),
        })
    }
    /// The number of the one face of a surface.
    pub const FACE: usize = 0;
}

impl Placeable for SurfaceGeometry {
    fn faces(&self) -> Vec<(&Face, Isometry)> {
        vec![(&self.face, Isometry::identity())]
    }
    fn rim(&self) -> Option<&Rim> {
        self.rim.as_ref()
    }
}

/// The error for a geometry that is described but cannot be traced yet.
fn not_supported_yet(what: &str) -> OpossumError {
    OpossumError::Other(format!(
        "{what} is not supported yet: it has no lateral boundary to clip a hit to, so it cannot be \
         traced"
    ))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Aperture, ApertureType, CircleShape},
        degree,
        geometry::{Assembly, ParametricSolid, Part, Side},
        material::Material,
        millimeter,
        properties::proptype::AssetRef,
        refractive_index::RefrIndexConst,
    };
    use approx::assert_abs_diff_eq;
    use uom::si::length::millimeter;

    fn circle() -> OpmResult<ValidatedCrossSection> {
        ValidatedCrossSection::try_new(Aperture::new(
            CircleShape::new(millimeter!(12.5))?.into(),
            ApertureType::Hole,
            None,
            None,
        )?)
    }
    #[test]
    fn a_surface_has_one_face() -> OpmResult<()> {
        let placed = Geometry::plane(None).place(&Isometry::identity())?;
        assert_eq!(placed.face_count(), 1);
        assert!(placed.surface(SurfaceGeometry::FACE).is_some());
        assert!(placed.surface(SurfaceGeometry::FACE + 1).is_none());
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
        let placed = geometry.place(&Isometry::identity())?;
        assert_abs_diff_eq!(
            placed
                .anchor(Extruded::FRONT)?
                .translation()
                .z
                .get::<millimeter>(),
            50.0
        );
        assert_abs_diff_eq!(
            placed
                .anchor(Extruded::REAR)?
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
        let rear = geometry
            .place(&Isometry::identity())?
            .anchor(Extruded::REAR)?;
        let normal = rear.transform_vector_f64(&nalgebra::Vector3::z());
        assert_abs_diff_eq!(normal.y, -(5.0_f64.to_radians().sin()), epsilon = 1e-12);
        Ok(())
    }
    #[test]
    fn solids_that_cannot_be_traced_yet_say_so() -> OpmResult<()> {
        let parametric = Solid::Parametric(ParametricSolid::new(vec![(
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Side::Behind,
        )])?);
        let glass = AssetRef::Inline(Material::from(&RefrIndexConst::new(1.5)?));
        let part = Part::new(Isometry::identity(), parametric.clone(), glass);
        let assembly = Solid::Assembly(Assembly::new(vec![part], vec![])?);
        for solid in [parametric, assembly] {
            let error = Geometry::Solid(solid)
                .place(&Isometry::identity())
                .unwrap_err();
            assert!(error.to_string().contains("not supported yet"));
        }
        Ok(())
    }
    /// A curved face has to reach as far out as the cross section it is bounded by; a sphere or a
    /// cylinder ends at its radius of curvature. Exactly reaching it (a hemisphere) is still a shape.
    #[test]
    fn a_cross_section_beyond_the_curvature_is_rejected() -> OpmResult<()> {
        let flat = millimeter!(f64::INFINITY);
        for radius in [10.0, -10.0] {
            let curved = millimeter!(radius);
            for (front, rear) in [(curved, flat), (flat, curved)] {
                let error = Geometry::singlet(front, rear, millimeter!(5.0), circle()?)
                    .expect_err("a curvature tighter than the cross section was accepted");
                assert!(error.to_string().contains("does not reach"), "{error}");
            }
            assert!(
                Geometry::cylindrical_singlet(curved, flat, millimeter!(5.0), circle()?).is_err()
            );
            let mirror = |cross_section| {
                Geometry::surface(
                    Isometry::identity(),
                    SurfaceShape::spherical(curved),
                    Isometry::identity(),
                    cross_section,
                )
            };
            assert!(mirror(Some(circle()?)).is_err());
            // An unbounded surface has no edge to reach.
            assert!(mirror(None).is_ok());
        }
        assert!(Geometry::singlet(millimeter!(12.5), flat, millimeter!(5.0), circle()?).is_ok());
        // A parabola opens without end, so even a short focal length reaches any edge.
        let parabola = SurfaceShape::Parabola {
            focal_length: millimeter!(1.0),
        };
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
    #[test]
    fn a_surface_places_its_face_and_its_rim_along_its_axis() -> OpmResult<()> {
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let vertex = Isometry::new_along_z(millimeter!(20.0))?;
        let geometry = Geometry::surface(axis, SurfaceShape::Plane, vertex, Some(circle()?))?;
        let placed = geometry.place(&Isometry::identity())?;
        assert_eq!(placed.anchor(SurfaceGeometry::FACE)?, axis.append(&vertex));
        assert_eq!(
            placed.rim(),
            Some(&Rim::new(circle()?, axis, RimRole::Edge))
        );
        Ok(())
    }
    /// Without a cross section there is no rim to carry the axis, but the face is still stated in the
    /// axis frame.
    #[test]
    fn an_unbounded_surface_is_placed_along_its_axis() -> OpmResult<()> {
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let vertex = Isometry::new_along_z(millimeter!(20.0))?;
        let geometry = Geometry::surface(axis, SurfaceShape::Plane, vertex, None)?;
        let placed = geometry.place(&Isometry::identity())?;
        assert_eq!(placed.anchor(SurfaceGeometry::FACE)?, axis.append(&vertex));
        assert_eq!(placed.rim(), None);
        Ok(())
    }
    /// A detector's cross section is a window: light outside it still passes, it is just not recorded.
    #[test]
    fn a_detector_bounds_its_window_not_its_edge() -> OpmResult<()> {
        let bounded = Geometry::detector(SurfaceShape::Plane, Some(circle()?))?;
        assert_eq!(
            bounded.place(&Isometry::identity())?.rim(),
            Some(&Rim::new(circle()?, Isometry::identity(), RimRole::Window))
        );
        let open = Geometry::detector(SurfaceShape::Plane, None)?;
        assert_eq!(open.place(&Isometry::identity())?.rim(), None);
        Ok(())
    }
    /// A surface without a cross section is unbounded; a bounded surface and an extruded solid have
    /// a rim, stated along the extrusion axis.
    #[test]
    fn rim_is_none_for_a_virtual_plane() -> OpmResult<()> {
        let rim_of = |geometry: Geometry| -> OpmResult<Option<Rim>> {
            Ok(geometry.place(&Isometry::identity())?.rim().cloned())
        };
        assert_eq!(rim_of(Geometry::plane(None))?, None);
        assert_eq!(
            rim_of(Geometry::plane(Some(circle()?)))?,
            Some(Rim::new(circle()?, Isometry::identity(), RimRole::Edge))
        );
        let axis = Isometry::new(millimeter!(3.0, 0.0, 0.0), degree!(0.0, 90.0, 0.0))?;
        let extruded = Geometry::Solid(Solid::Extruded(Box::new(Extruded::new(
            axis,
            Face::new(SurfaceShape::Plane, Isometry::identity()),
            Face::new(
                SurfaceShape::Plane,
                Isometry::new_along_z(millimeter!(20.0))?,
            ),
            circle()?,
        )?)));
        assert_eq!(
            rim_of(extruded)?,
            Some(Rim::new(circle()?, axis, RimRole::Edge))
        );
        Ok(())
    }
    #[test]
    fn a_single_surface_encloses_no_volume() -> OpmResult<()> {
        let placed = Geometry::plane(None).place(&Isometry::identity())?;
        assert!(Placed::<Extruded>::try_from(placed).is_err());
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
