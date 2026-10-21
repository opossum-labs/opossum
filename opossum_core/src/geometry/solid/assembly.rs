#![warn(missing_docs)]
//! Several solids, each with its own material, sharing interfaces.

use nalgebra::Point3;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use super::Side;
use crate::{
    error::{OpmResult, OpossumError},
    geometry::{
        Face, Geometry, GeometryKind, Rim,
        body::{Body, BoundingBox},
    },
    light::Ray,
    material::Material,
    properties::proptype::AssetRef,
    utils::geom_transformation::Isometry,
};

/// One solid of an [`Assembly`], placed where it sits in the assembly, with its material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Part {
    geometry: Geometry,
    material: AssetRef<Material>,
}

impl Part {
    /// Create a new [`Part`].
    ///
    /// # Arguments
    ///
    /// * `placement` - where the solid sits relative to the node.
    /// * `geometry` - the solid itself.
    /// * `material` - the material the solid is made of.
    ///
    /// # Errors
    ///
    /// This function returns an error if the geometry encloses no volume or is an assembly itself;
    /// the parts of an inner assembly are added directly instead.
    pub fn new(
        placement: &Isometry,
        mut geometry: Geometry,
        material: AssetRef<Material>,
    ) -> OpmResult<Self> {
        if geometry.kind().as_body().is_none() {
            return Err(OpossumError::Other(
                "a part of an assembly has to enclose a volume".into(),
            ));
        }
        if matches!(geometry, Geometry::Assembly(_)) {
            return Err(OpossumError::Other(
                "a part of an assembly must not be an assembly itself; add its parts directly"
                    .into(),
            ));
        }
        geometry.move_by(placement);
        Ok(Self { geometry, material })
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
    /// * `face` - the index of the face in that part's solid, as
    ///   [`GeometryKind`] numbers them: 0 and 1 are front and rear of an
    ///   [`Extruded`](crate::geometry::Extruded) solid, a
    ///   [`ParametricSolid`](crate::geometry::ParametricSolid) counts its faces.
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
    frame: Isometry,
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
    /// - an interface refers to a part or face that does not exist or joins two faces of the same
    ///   part,
    /// - the two faces of an interface are not the same surface at the same place (see
    ///   [`Face::coincides_with`]), or the two parts lie on the same side of it.
    pub fn new(parts: Vec<Part>, interfaces: Vec<Interface>) -> OpmResult<Self> {
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
                    .and_then(|part| {
                        let kind = part.geometry.kind();
                        Some((kind.face(id.face)?, kind.as_body()?.side(id.face)?))
                    })
                    .ok_or_else(|| {
                        OpossumError::Other(format!(
                            "an interface refers to face {} of part {}, which does not exist",
                            id.face, id.part
                        ))
                    })
            };
            let (face_a, side_a) = resolve(interface.a)?;
            let (face_b, side_b) = resolve(interface.b)?;
            if !face_a.coincides_with(face_b) {
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
        Ok(Self {
            frame: Isometry::identity(),
            parts,
            interfaces,
        })
    }
    /// Return the part a face of the assembly belongs to, and the number the face has in the solid
    /// of that part.
    ///
    /// The faces of an assembly are those of its parts, part after part.
    ///
    /// # Arguments
    ///
    /// * `face` - the number of the face in the assembly.
    fn locate(&self, face: usize) -> Option<(&Part, usize)> {
        let mut first_face = 0;
        for part in &self.parts {
            let count = part.geometry.kind().face_count();
            if face < first_face + count {
                return Some((part, face - first_face));
            }
            first_face += count;
        }
        None
    }
}

/// A face of an assembly is a face of one of its parts and ends where that part's solid says.
impl GeometryKind for Assembly {
    fn face(&self, index: usize) -> Option<&Face> {
        self.locate(index)
            .and_then(|(part, own_face)| part.geometry.kind().face(own_face))
    }
    fn face_count(&self) -> usize {
        self.parts
            .iter()
            .map(|part| part.geometry.kind().face_count())
            .sum()
    }
    fn includes(&self, face: usize, point: &Point3<Length>) -> bool {
        self.locate(face)
            .is_some_and(|(part, own_face)| part.geometry.kind().includes(own_face, point))
    }
    fn window(&self) -> Option<&Rim> {
        None
    }
    fn as_body(&self) -> Option<&dyn Body> {
        Some(self)
    }
    fn move_by(&mut self, by: &Isometry) {
        self.frame = by.append(&self.frame);
        for part in &mut self.parts {
            part.geometry.move_by(by);
        }
    }
}

/// An assembly contains what one of its parts contains; a ray travels through its parts one after
/// the other, so the chord is the sum of theirs. Every part answers for itself: parts must not
/// overlap, or the overlap counts twice.
impl Body for Assembly {
    fn contains(&self, point: &Point3<Length>) -> bool {
        self.parts.iter().any(|part| {
            part.geometry
                .kind()
                .as_body()
                .is_some_and(|body| body.contains(point))
        })
    }
    fn path_length_inside(&self, ray: &Ray) -> Option<Length> {
        self.parts
            .iter()
            .filter_map(|part| part.geometry.kind().as_body()?.path_length_inside(ray))
            .reduce(|one, other| one + other)
    }
    /// The extent of an assembly is not determined yet.
    ///
    /// # Errors
    ///
    /// This function always returns an error.
    fn bounding_box(&self) -> OpmResult<BoundingBox> {
        Err(OpossumError::Other(
            "the extent of an assembly cannot be determined yet".into(),
        ))
    }
    /// The frame the parts are stated in: the node's, once the assembly is moved there.
    fn isometry(&self) -> Isometry {
        self.frame
    }
    fn side(&self, face: usize) -> Option<Side> {
        self.locate(face)
            .and_then(|(part, own_face)| part.geometry.kind().as_body()?.side(own_face))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{CircleShape, Outline},
        degree,
        geometry::{ParametricSolid, SurfaceShape},
        joule,
        light::Ray,
        millimeter, nanometer,
        refractive_index::RefrIndexConst,
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::{Point3, Vector3};
    use num_traits::Zero;
    use uom::si::{
        f64::{Angle, Length},
        length::millimeter,
    };

    fn circle() -> OpmResult<Outline> {
        Outline::new(CircleShape::new(millimeter!(12.5))?, Vec::new())
    }
    /// The given geometry as a part made of glass with a refractive index of 1.5, at the given
    /// placement.
    fn part_of(geometry: Geometry, placement: Isometry) -> OpmResult<Part> {
        Part::new(
            &placement,
            geometry,
            AssetRef::Inline(Material::from(&RefrIndexConst::new(1.5)?)),
        )
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
        let solid = Geometry::Parametric(ParametricSolid::new(vec![
            (
                Face::new(SurfaceShape::Plane, Isometry::identity()),
                Side::Behind,
            ),
            (diagonal()?, Side::InFront),
        ])?);
        part_of(solid, Isometry::identity())
    }
    /// The rear prism of the same cube: behind the diagonal (face 0), in front of its exit face.
    fn opposite_prism_part() -> OpmResult<Part> {
        let solid = Geometry::Parametric(ParametricSolid::new(vec![
            (diagonal()?, Side::Behind),
            (
                Face::new(
                    SurfaceShape::Plane,
                    Isometry::new_along_z(millimeter!(20.0))?,
                ),
                Side::InFront,
            ),
        ])?);
        part_of(solid, Isometry::identity())
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
    /// The faces of an assembly are those of its parts, part after part, each placed with its part -
    /// so the cemented faces of a doublet come out at the same place. Moved, every face moves along.
    #[test]
    fn an_assembly_numbers_the_faces_of_its_parts_one_after_the_other() -> OpmResult<()> {
        let behind_the_crown = Isometry::new_along_z(millimeter!(5.0))?;
        let doublet = Assembly::new(
            vec![
                crown_part()?,
                flint_part(millimeter!(-40.0), behind_the_crown)?,
            ],
            vec![Interface::new(FaceId::new(0, 1), FaceId::new(1, 0))],
        )?;
        let frame = Isometry::new_along_z(millimeter!(100.0))?;
        let moved = placed_at(&doublet, &frame);
        assert_eq!(moved.face_count(), 4);
        assert!(moved.window().is_none());
        assert!(moved.face(4).is_none());
        // the vertices of the crown front, of the crown rear and flint front (cemented) and of the
        // flint rear
        for (index, z) in [0.0, 5.0, 5.0, 10.0].into_iter().enumerate() {
            let (Some(face), Some(moved_face)) = (doublet.face(index), moved.face(index)) else {
                return Err(OpossumError::Other(format!("there is no face {index}")));
            };
            assert_abs_diff_eq!(
                face.frame().translation().z.get::<millimeter>(),
                z,
                epsilon = 1e-9
            );
            assert_abs_diff_eq!(
                moved_face.frame().translation().z.get::<millimeter>(),
                z + 100.0,
                epsilon = 1e-9
            );
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
    /// The crown and the flint lens of a doublet, cemented along the flint's front at z = 5 mm.
    fn doublet() -> OpmResult<Assembly> {
        let behind_the_crown = Isometry::new_along_z(millimeter!(5.0))?;
        Assembly::new(
            vec![
                crown_part()?,
                flint_part(millimeter!(-40.0), behind_the_crown)?,
            ],
            vec![Interface::new(FaceId::new(0, 1), FaceId::new(1, 0))],
        )
    }
    /// The crown lens at the origin and a second lens 30 mm aside and 8 mm further along z, without
    /// an interface between them.
    fn side_by_side() -> OpmResult<Assembly> {
        let aside = Isometry::new_translation(millimeter!(30.0, 0.0, 8.0))?;
        Assembly::new(
            vec![crown_part()?, flint_part(millimeter!(-40.0), aside)?],
            vec![],
        )
    }
    /// A node frame away from the origin and turned about all three axes.
    fn node_frame() -> OpmResult<Isometry> {
        Isometry::new(millimeter!(3.0, -2.0, 100.0), degree!(10.0, -20.0, 30.0))
    }
    /// A copy of the assembly, moved to the given node frame.
    fn placed_at(assembly: &Assembly, node_frame: &Isometry) -> Assembly {
        let mut placed = assembly.clone();
        placed.move_by(node_frame);
        placed
    }
    /// The chord of a ray through one part of the assembly on its own, placed where the assembly
    /// places it.
    fn chord_through_part(
        assembly: &Assembly,
        index: usize,
        node_frame: &Isometry,
        ray: &Ray,
    ) -> OpmResult<Option<Length>> {
        let part = assembly
            .parts
            .get(index)
            .ok_or_else(|| OpossumError::Other(format!("there is no part {index}")))?;
        let mut geometry = part.geometry.clone();
        geometry.move_by(node_frame);
        Ok(geometry
            .kind()
            .as_body()
            .and_then(|body| body.path_length_inside(ray)))
    }
    fn test_ray(position: Point3<Length>, direction: Vector3<f64>) -> OpmResult<Ray> {
        Ray::new(position, direction, nanometer!(1053.0), joule!(1.0))
    }
    /// A point of an assembly's own frame, in global coordinates.
    fn global(node_frame: &Isometry, x: f64, y: f64, z: f64) -> Point3<Length> {
        node_frame.transform_point(&millimeter!(x, y, z))
    }
    /// Every part is asked in its own frame: the second lens sits aside, so a point in front of
    /// it is in the lens only where its edge is measured from that place.
    #[test]
    fn an_assembly_contains_what_one_of_its_parts_contains() -> OpmResult<()> {
        let node_frame = node_frame()?;
        let placed = placed_at(&side_by_side()?, &node_frame);
        for ((x, y, z), inside) in [
            ((0.0, 0.0, 2.0), true),
            ((30.0, 0.0, 10.0), true),
            ((15.0, 0.0, 2.0), false),
            ((0.0, 0.0, 10.0), false),
            ((30.0, 0.0, 2.0), false),
            ((55.0, 0.0, 10.0), false),
        ] {
            assert_eq!(
                placed.contains(&global(&node_frame, x, y, z)),
                inside,
                "({x}, {y}, {z})"
            );
        }
        Ok(())
    }
    /// A face of an assembly is a face of one of its parts and ends where that part does, in the
    /// frame of the part. Faces 0 and 1 are those of the crown, 2 and 3 those of the second lens.
    #[test]
    fn a_face_of_an_assembly_ends_where_the_solid_of_its_part_does() -> OpmResult<()> {
        let node_frame = node_frame()?;
        let placed = placed_at(&side_by_side()?, &node_frame);
        // The points lie on the spheres, the sheet check allows a nanometer: the front of the crown
        // has a radius of 60 mm, the front of the second lens one of -40 mm and its vertex at z = 8.
        let on_sphere = |radius: f64, off_axis: f64| {
            radius.signum() * (radius.abs() - radius.mul_add(radius, -(off_axis * off_axis)).sqrt())
        };
        for (face, (x, y, z), included) in [
            (0, (0.0, 0.0, 0.0), true),
            (0, (12.4, 0.0, on_sphere(60.0, 12.4)), true),
            (0, (30.0, 0.0, on_sphere(60.0, 30.0)), false),
            (2, (30.0, 0.0, 8.0), true),
            (2, (30.0, 12.4, 8.0 + on_sphere(-40.0, 12.4)), true),
            (2, (30.0, 13.0, 8.0 + on_sphere(-40.0, 13.0)), false),
            // Where the second lens would be without its placement.
            (2, (0.0, 0.0, 8.0 + on_sphere(-40.0, 30.0)), false),
            // There is no fifth face.
            (4, (0.0, 0.0, 0.0), false),
        ] {
            assert_eq!(
                placed.includes(face, &global(&node_frame, x, y, z)),
                included,
                "face {face} at ({x}, {y}, {z})"
            );
        }
        Ok(())
    }
    /// The chord through an assembly is what each part cuts out of the ray, added up - checked
    /// against the parts placed on their own.
    #[test]
    fn a_ray_travels_through_the_parts_one_after_the_other() -> OpmResult<()> {
        let node_frame = node_frame()?;
        let doublet = doublet()?;
        let placed = placed_at(&doublet, &node_frame);
        for (x, direction) in [(0.0, Vector3::z()), (4.0, Vector3::new(0.05, 0.0, 1.0))] {
            let ray = test_ray(
                global(&node_frame, x, 3.0, -20.0),
                node_frame.transform_vector_f64(&direction.normalize()),
            )?;
            let (Some(crown), Some(flint)) = (
                chord_through_part(&doublet, 0, &node_frame, &ray)?,
                chord_through_part(&doublet, 1, &node_frame, &ray)?,
            ) else {
                return Err(OpossumError::Other("the ray missed a lens".into()));
            };
            assert_abs_diff_eq!(
                placed
                    .path_length_inside(&ray)
                    .ok_or_else(|| OpossumError::Other("the ray missed the doublet".into()))?
                    .get::<millimeter>(),
                (crown + flint).get::<millimeter>(),
                epsilon = 1e-9
            );
        }
        // Along the axis the two centers add up to the 10 mm of the doublet.
        let axis = test_ray(
            global(&node_frame, 0.0, 0.0, -20.0),
            node_frame.transform_vector_f64(&Vector3::z()),
        )?;
        let chord = placed
            .path_length_inside(&axis)
            .ok_or_else(|| OpossumError::Other("the ray missed the doublet".into()))?;
        assert_abs_diff_eq!(chord.get::<millimeter>(), 10.0, epsilon = 1e-9);
        Ok(())
    }
    /// Only the part the ray goes through contributes, wherever that part sits.
    #[test]
    fn a_ray_through_one_part_of_an_assembly_has_the_chord_of_that_part() -> OpmResult<()> {
        let node_frame = node_frame()?;
        let assembly = side_by_side()?;
        let placed = placed_at(&assembly, &node_frame);
        for (x, part) in [(0.0, 0), (30.0, 1)] {
            let ray = test_ray(
                global(&node_frame, x, 0.0, -20.0),
                node_frame.transform_vector_f64(&Vector3::z()),
            )?;
            let chord = placed.path_length_inside(&ray);
            assert_eq!(
                chord,
                chord_through_part(&assembly, part, &node_frame, &ray)?
            );
            assert!(chord.is_some(), "x = {x}");
        }
        let between = test_ray(
            global(&node_frame, 15.0, 0.0, -20.0),
            node_frame.transform_vector_f64(&Vector3::z()),
        )?;
        assert!(placed.path_length_inside(&between).is_none());
        Ok(())
    }
    /// Both prisms of a beam splitter cube are slabs on their own; the ray crosses the first from
    /// its front face to the diagonal, the second from the diagonal to its rear face.
    #[test]
    fn a_ray_crosses_the_prisms_of_a_cube_one_after_the_other() -> OpmResult<()> {
        let node_frame = node_frame()?;
        let placed = placed_at(&cube()?, &node_frame);
        let ray = test_ray(
            global(&node_frame, 0.0, 0.0, -20.0),
            node_frame.transform_vector_f64(&Vector3::z()),
        )?;
        let chord = placed
            .path_length_inside(&ray)
            .ok_or_else(|| OpossumError::Other("the ray missed the cube".into()))?;
        assert_abs_diff_eq!(chord.get::<millimeter>(), 20.0, epsilon = 1e-9);
        Ok(())
    }
    /// Even an assembly of parts that each state their extent has none yet.
    #[test]
    fn the_extent_of_an_assembly_cannot_be_determined_yet() -> OpmResult<()> {
        let error = side_by_side()?.bounding_box().unwrap_err();
        assert!(
            error.to_string().contains("cannot be determined yet"),
            "{error}"
        );
        Ok(())
    }
    /// A face of an assembly lies on the side of it its part lies on: the front prism of the cube
    /// behind its entrance face and in front of the diagonal, the rear prism the other way round.
    #[test]
    fn an_assembly_lies_on_the_sides_its_parts_lie_on() -> OpmResult<()> {
        let cube = cube()?;
        let sides: Vec<_> = (0..5).map(|face| cube.side(face)).collect();
        assert_eq!(
            sides,
            [
                Some(Side::Behind),
                Some(Side::InFront),
                Some(Side::Behind),
                Some(Side::InFront),
                None
            ]
        );
        Ok(())
    }
    #[test]
    fn an_assembly_has_the_frame_of_its_node() -> OpmResult<()> {
        let node_frame = node_frame()?;
        assert_eq!(placed_at(&doublet()?, &node_frame).isometry(), node_frame);
        Ok(())
    }
    #[test]
    fn an_assembly_without_parts_has_no_volume() -> OpmResult<()> {
        let empty = Assembly::new(vec![], vec![])?;
        assert!(!empty.contains(&millimeter!(0.0, 0.0, 0.0)));
        let ray = test_ray(millimeter!(0.0, 0.0, -20.0), Vector3::z())?;
        assert!(empty.path_length_inside(&ray).is_none());
        assert!(empty.bounding_box().is_err());
        Ok(())
    }
    /// A part is a solid: neither a single surface nor an assembly of its own.
    #[test]
    fn a_part_is_a_solid_but_not_an_assembly() -> OpmResult<()> {
        let inner = Assembly::new(vec![prism_part()?], vec![])?;
        let nested = part_of(Geometry::Assembly(inner), Isometry::identity()).unwrap_err();
        assert!(nested.to_string().contains("assembly itself"), "{nested}");
        let surface = part_of(Geometry::plane(None), Isometry::identity()).unwrap_err();
        assert!(
            surface.to_string().contains("enclose a volume"),
            "{surface}"
        );
        Ok(())
    }
    #[test]
    fn an_assembly_survives_a_round_trip_through_a_file() -> OpmResult<()> {
        let splitter_cube = Geometry::Assembly(cube()?);
        let text =
            ron::to_string(&splitter_cube).map_err(|e| OpossumError::Other(e.to_string()))?;
        let read: Geometry =
            ron::from_str(&text).map_err(|e| OpossumError::Other(e.to_string()))?;
        assert_eq!(read, splitter_cube);
        Ok(())
    }
}
