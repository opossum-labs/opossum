#![warn(missing_docs)]
//! Several solids, each with its own material, sharing interfaces.

use serde::{Deserialize, Serialize};

use super::{Side, Solid, collect_faces};
use crate::{
    error::{OpmResult, OpossumError},
    geometry::{Face, Placeable, Rim},
    material::Material,
    properties::proptype::AssetRef,
    utils::geom_transformation::Isometry,
};

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

impl Placeable for Assembly {
    /// The faces of all parts, part after part, each stated with the placement of its part.
    fn faces(&self) -> Vec<(&Face, Isometry)> {
        self.parts
            .iter()
            .flat_map(|part| collect_faces(|index| part.face(index)))
            .collect()
    }
    /// An assembly has no lateral boundary of its own; its parts may have one each.
    fn rim(&self) -> Option<&Rim> {
        None
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Aperture, ApertureType, CircleShape},
        degree,
        geometry::{Geometry, ParametricSolid, Placed, SurfaceShape},
        millimeter,
        refractive_index::RefrIndexConst,
        types::validated_type_definitions::ValidatedCrossSection,
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::Point3;
    use num_traits::Zero;
    use uom::si::{
        f64::{Angle, Length},
        length::millimeter,
    };

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
    /// The faces of a placed assembly are those of its parts, part after part, each stated with the
    /// placement of its part - so the cemented faces of a doublet come out at the same place.
    #[test]
    fn a_placed_assembly_numbers_the_faces_of_its_parts_one_after_the_other() -> OpmResult<()> {
        let behind_the_crown = Isometry::new_along_z(millimeter!(5.0))?;
        let doublet = Assembly::new(
            vec![
                crown_part()?,
                flint_part(millimeter!(-40.0), behind_the_crown)?,
            ],
            vec![Interface::new(FaceId::new(0, 1), FaceId::new(1, 0))],
        )?;
        let frame = Isometry::new_along_z(millimeter!(100.0))?;
        let placed = Placed::new(doublet, &frame)?;
        assert_eq!(placed.face_count(), 4);
        assert!(placed.rim().is_none());
        // crown front (R = 60), crown rear and flint front (both R = -40), flint rear (flat)
        for (index, z) in [60.0, -35.0, -35.0, 10.0].into_iter().enumerate() {
            assert_abs_diff_eq!(
                placed.anchor(index)?.translation().z.get::<millimeter>(),
                z,
                epsilon = 1e-9
            );
            let surface = placed
                .surface(index)
                .ok_or_else(|| OpossumError::Other(format!("face {index} has no surface")))?;
            assert_eq!(*surface.0.isometry(), frame.append(&placed.anchor(index)?));
        }
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
    fn an_assembly_survives_a_round_trip_through_a_file() -> OpmResult<()> {
        let splitter_cube = Geometry::Solid(Solid::Assembly(cube()?));
        let text =
            ron::to_string(&splitter_cube).map_err(|e| OpossumError::Other(e.to_string()))?;
        let read: Geometry =
            ron::from_str(&text).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(read, splitter_cube);
        Ok(())
    }
}
