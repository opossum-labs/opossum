#![warn(missing_docs)]
//! Solids bounded by several faces, each stating which of its sides is inside.

use nalgebra::Point3;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use super::{Side, face_hits};
use crate::{
    error::{OpmResult, OpossumError},
    gain::Inversion,
    geometry::{
        Face, FaceIndex, GeometryKind, Rim,
        body::{Body, BoundingBox},
    },
    light::Ray,
    utils::geom_transformation::Isometry,
};

/// A solid bounded by several faces: the intersection of the inner sides of all of them.
///
/// Built from flat faces only, such a solid is always convex (prisms, corner cubes, roof prisms);
/// curved faces with their outside as inner side also allow concave shapes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParametricSolid {
    frame: Isometry,
    faces: Vec<(Face, Side)>,
    #[serde(skip)]
    inversion: Option<Inversion>,
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
        Ok(Self {
            frame: Isometry::identity(),
            faces,
            inversion: None,
        })
    }
    /// Return, for every face in the order they are numbered, whether the point lies on the side of
    /// it that is inside the solid.
    ///
    /// # Arguments
    ///
    /// * `point` - the point to be tested, in the frame the solid is stated in.
    fn inner_sides<'a>(&'a self, point: &'a Point3<Length>) -> impl Iterator<Item = bool> + 'a {
        self.faces
            .iter()
            .map(move |(face, side)| side.contains(face.is_behind(point)))
    }
}

/// A face ends where it meets the others: a point on its surface belongs to the solid if it lies on
/// the inner side of every *other* face. As for every component, a face is the part of its surface
/// on the sheet through its vertex.
impl GeometryKind for ParametricSolid {
    fn face(&self, index: FaceIndex) -> Option<&Face> {
        self.faces.get(usize::from(index)).map(|(face, _)| face)
    }
    fn face_count(&self) -> usize {
        self.faces.len()
    }
    /// The faces are named in the order they are numbered, counting from "surface 1".
    fn face_name(&self, face: FaceIndex) -> Option<String> {
        let index = usize::from(face);
        (index < self.faces.len()).then(|| format!("surface {}", index + 1))
    }
    fn includes(&self, face: FaceIndex, point: &Point3<Length>) -> bool {
        GeometryKind::face(self, face).is_some_and(|surface| surface.is_on_vertex_sheet(point))
            && self
                .inner_sides(point)
                .enumerate()
                .all(|(index, inside)| FaceIndex::new(index) == face || inside)
    }
    fn window(&self) -> Option<&Rim> {
        None
    }
    fn as_body(&self) -> Option<&dyn Body> {
        Some(self)
    }
    fn as_body_mut(&mut self) -> Option<&mut dyn Body> {
        Some(self)
    }
    fn move_by(&mut self, by: &Isometry) {
        self.frame = by.append(&self.frame);
        for (face, _) in &mut self.faces {
            face.move_by(by);
        }
    }
}

/// A point is inside this solid if it lies on the inner side of every one of its faces. A face
/// bounds the solid only where it lies within all the other faces, which is where a ray meets it.
impl Body for ParametricSolid {
    fn contains(&self, point: &Point3<Length>) -> bool {
        self.inner_sides(point).all(|inside| inside)
    }
    fn path_length_inside(&self, ray: &Ray) -> Option<Length> {
        let faces = self.faces.iter().map(|(face, _)| face);
        let hits = face_hits(faces, ray, |face, point| self.includes(face, point));
        self.chord_through(ray.position(), ray.direction(), hits)
    }
    /// The extent of the solid is not determined yet.
    ///
    /// # Errors
    ///
    /// This function always returns an error.
    fn bounding_box(&self) -> OpmResult<BoundingBox> {
        Err(OpossumError::Other(
            "the extent of a parametric solid cannot be determined yet".into(),
        ))
    }
    /// The frame the faces are stated in: the node's, once the solid is moved there.
    fn isometry(&self) -> Isometry {
        self.frame
    }
    fn side(&self, face: FaceIndex) -> Option<Side> {
        self.faces.get(usize::from(face)).map(|(_, side)| *side)
    }
    fn inversion(&self) -> Option<&Inversion> {
        self.inversion.as_ref()
    }
    fn inversion_mut(&mut self) -> &mut Option<Inversion> {
        &mut self.inversion
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        degree,
        geometry::{Sphere, SurfaceShape},
        joule,
        light::Ray,
        millimeter, nanometer,
    };
    use approx::assert_abs_diff_eq;
    use nalgebra::{Point3, Vector3};
    use uom::si::{f64::Length, length::millimeter};

    /// A block of 10 mm × 10 mm × 10 mm from six flat faces: the front face at z = 0, the rear face
    /// at z = 10, and one face at each of x = ±5 mm and y = ±5 mm. The faces on the sides are turned
    /// so that their z axis points into the block at the negative side and out of it at the positive.
    fn block() -> OpmResult<ParametricSolid> {
        let plane = |x: f64, y: f64, z: f64, about_x: f64, about_y: f64| {
            Isometry::new(millimeter!(x, y, z), degree!(about_x, about_y, 0.0))
        };
        ParametricSolid::new(vec![
            (
                Face::new(SurfaceShape::Plane, plane(0.0, 0.0, 0.0, 0.0, 0.0)?),
                Side::Behind,
            ),
            (
                Face::new(SurfaceShape::Plane, plane(0.0, 0.0, 10.0, 0.0, 0.0)?),
                Side::InFront,
            ),
            (
                Face::new(SurfaceShape::Plane, plane(-5.0, 0.0, 5.0, 0.0, 90.0)?),
                Side::Behind,
            ),
            (
                Face::new(SurfaceShape::Plane, plane(5.0, 0.0, 5.0, 0.0, 90.0)?),
                Side::InFront,
            ),
            (
                Face::new(SurfaceShape::Plane, plane(0.0, -5.0, 5.0, -90.0, 0.0)?),
                Side::Behind,
            ),
            (
                Face::new(SurfaceShape::Plane, plane(0.0, 5.0, 5.0, -90.0, 0.0)?),
                Side::InFront,
            ),
        ])
    }
    /// The block placed at the given node frame.
    fn placed_block(node_frame: &Isometry) -> OpmResult<ParametricSolid> {
        let mut block = block()?;
        block.move_by(node_frame);
        Ok(block)
    }
    fn test_ray(position: Point3<Length>, direction: Vector3<f64>) -> OpmResult<Ray> {
        Ray::new(position, direction, nanometer!(1053.0), joule!(1.0))
    }
    /// The chord of the ray through the block, in millimeter.
    fn chord(block: &ParametricSolid, ray: &Ray) -> OpmResult<f64> {
        block
            .path_length_inside(ray)
            .map(|length| length.get::<millimeter>())
            .ok_or_else(|| OpossumError::Other("the ray missed the block".into()))
    }

    #[test]
    fn a_parametric_solid_needs_a_face() {
        assert!(ParametricSolid::new(vec![]).is_err());
    }
    /// A parametric solid numbers its faces in the order they were given and has no rim: its faces
    /// bound it by themselves. Moved, every face moves along.
    #[test]
    fn a_parametric_solid_numbers_its_faces_as_given() -> OpmResult<()> {
        let at = |z: f64| Isometry::new_along_z(millimeter!(z));
        let solid = ParametricSolid::new(vec![
            (Face::new(SurfaceShape::Plane, at(0.0)?), Side::Behind),
            (Face::new(SurfaceShape::Plane, at(10.0)?), Side::InFront),
            (
                Face::new(
                    SurfaceShape::Sphere(Sphere::new(millimeter!(40.0))?),
                    at(2.0)?,
                ),
                Side::Behind,
            ),
        ])?;
        let frame = Isometry::new_along_z(millimeter!(100.0))?;
        let mut moved = solid.clone();
        moved.move_by(&frame);
        assert_eq!(moved.face_count(), 3);
        assert!(moved.window().is_none());
        for (index, z) in [0.0, 10.0, 2.0].into_iter().enumerate() {
            let (Some(face), Some(moved_face)) = (
                solid.face(FaceIndex::new(index)),
                moved.face(FaceIndex::new(index)),
            ) else {
                return Err(OpossumError::Other(format!("there is no face {index}")));
            };
            assert_abs_diff_eq!(
                face.frame().translation().z.get::<millimeter>(),
                z,
                epsilon = 1e-9
            );
            assert_eq!(*moved_face.frame(), frame.append(face.frame()));
        }
        Ok(())
    }
    #[test]
    fn a_block_contains_what_is_on_the_inner_side_of_all_its_faces() -> OpmResult<()> {
        let block = placed_block(&Isometry::identity())?;
        assert!(block.contains(&millimeter!(0.0, 0.0, 5.0)));
        for (x, y, z) in [
            (4.9, 0.0, 5.0),
            (-4.9, 0.0, 5.0),
            (0.0, 4.9, 5.0),
            (0.0, -4.9, 5.0),
            (0.0, 0.0, 0.1),
            (0.0, 0.0, 9.9),
        ] {
            assert!(block.contains(&millimeter!(x, y, z)), "({x}, {y}, {z})");
        }
        // Beyond any single one of the six faces.
        for (x, y, z) in [
            (5.1, 0.0, 5.0),
            (-5.1, 0.0, 5.0),
            (0.0, 5.1, 5.0),
            (0.0, -5.1, 5.0),
            (0.0, 0.0, -0.1),
            (0.0, 0.0, 10.1),
        ] {
            assert!(!block.contains(&millimeter!(x, y, z)), "({x}, {y}, {z})");
        }
        Ok(())
    }
    /// The solid owns the points on a face it lies behind, not those on a face it lies in front of:
    /// two solids sharing a face do not both claim its points.
    #[test]
    fn a_point_on_a_face_belongs_to_the_solid_only_if_it_lies_behind_that_face() -> OpmResult<()> {
        let block = placed_block(&Isometry::identity())?;
        for (x, y, z, belongs) in [
            (0.0, 0.0, 0.0, true),
            (0.0, 0.0, 10.0, false),
            (-5.0, 0.0, 5.0, true),
            (5.0, 0.0, 5.0, false),
            (0.0, -5.0, 5.0, true),
            (0.0, 5.0, 5.0, false),
        ] {
            assert_eq!(
                block.contains(&millimeter!(x, y, z)),
                belongs,
                "({x}, {y}, {z})"
            );
        }
        Ok(())
    }
    /// A face of the block reaches as far as the others let it: the front face covers the 10 mm
    /// square in its plane, not the plane. Faces 0 and 1 are the front and the rear, 2 to 5 the
    /// sides at x = -5, x = 5, y = -5 and y = 5.
    #[test]
    fn a_face_of_the_block_ends_where_it_meets_the_others() -> OpmResult<()> {
        let block = placed_block(&Isometry::identity())?;
        for (face, (x, y, z), included) in [
            (0, (4.9, -4.9, 0.0), true),
            (0, (5.1, 0.0, 0.0), false),
            (0, (0.0, -5.1, 0.0), false),
            (1, (-4.9, 4.9, 10.0), true),
            (1, (0.0, 5.1, 10.0), false),
            (3, (5.0, 0.0, 5.0), true),
            (3, (5.0, 0.0, 10.1), false),
            (3, (5.0, 5.1, 5.0), false),
            (4, (0.0, -5.0, 0.1), true),
            (4, (5.1, -5.0, 5.0), false),
            // There is no seventh face.
            (6, (0.0, 0.0, 5.0), false),
        ] {
            assert_eq!(
                block.includes(FaceIndex::new(face), &millimeter!(x, y, z)),
                included,
                "face {face} at ({x}, {y}, {z})"
            );
        }
        Ok(())
    }
    #[test]
    fn a_ray_crosses_the_block_from_one_face_to_the_opposite_one() -> OpmResult<()> {
        let block = placed_block(&Isometry::identity())?;
        for (position, direction, expected) in [
            // through the front and the rear face, along x through the sides
            (millimeter!(0.0, 0.0, -20.0), Vector3::z(), 10.0),
            (millimeter!(-20.0, 0.0, 5.0), Vector3::x(), 10.0),
            (millimeter!(0.0, 20.0, 5.0), -Vector3::y(), 10.0),
            // starting inside, and starting on the front face
            (millimeter!(0.0, 0.0, 5.0), Vector3::z(), 5.0),
            (millimeter!(0.0, 0.0, 5.0), -Vector3::z(), 5.0),
            (millimeter!(0.0, 0.0, 0.0), Vector3::z(), 10.0),
        ] {
            let ray = test_ray(position, direction)?;
            assert_abs_diff_eq!(chord(&block, &ray)?, expected, epsilon = 1e-9);
        }
        Ok(())
    }
    /// The ray enters through the face at x = -5 and leaves through the front face at z = 0. The plane
    /// of the face at x = 5 is hit further on, at z = -2.25, outside the block: counting that hit
    /// would stretch the chord to the far side.
    #[test]
    fn a_hit_beyond_the_edge_of_a_face_does_not_count() -> OpmResult<()> {
        let block = placed_block(&Isometry::identity())?;
        let ray = test_ray(
            millimeter!(-20.0, 0.0, 4.0),
            Vector3::new(1.0, 0.0, -0.25).normalize(),
        )?;
        assert_abs_diff_eq!(chord(&block, &ray)?, 1.0625_f64.sqrt(), epsilon = 1e-9);
        Ok(())
    }
    #[test]
    fn a_ray_beside_the_block_does_not_pass_through_it() -> OpmResult<()> {
        let block = placed_block(&Isometry::identity())?;
        // It meets the planes of the front and the rear face, but outside the block.
        let beside = test_ray(millimeter!(0.0, 7.0, -20.0), Vector3::z())?;
        assert!(block.path_length_inside(&beside).is_none());
        let pointing_away = test_ray(millimeter!(0.0, 0.0, -20.0), -Vector3::z())?;
        assert!(block.path_length_inside(&pointing_away).is_none());
        Ok(())
    }
    #[test]
    fn a_placed_block_follows_its_node() -> OpmResult<()> {
        let node_frame = Isometry::new(millimeter!(3.0, -2.0, 100.0), degree!(10.0, -20.0, 30.0))?;
        let block = placed_block(&node_frame)?;
        assert!(block.contains(&node_frame.transform_point(&millimeter!(0.0, 0.0, 5.0))));
        assert!(!block.contains(&node_frame.transform_point(&millimeter!(5.1, 0.0, 5.0))));
        assert!(!block.contains(&millimeter!(0.0, 0.0, 5.0)));
        let ray = test_ray(
            node_frame.transform_point(&millimeter!(0.0, 0.0, -20.0)),
            node_frame.transform_vector_f64(&Vector3::z()),
        )?;
        assert_abs_diff_eq!(chord(&block, &ray)?, 10.0, epsilon = 1e-9);
        Ok(())
    }
    /// The front face of a dome is the cap of a sphere of 40 mm radius with its vertex at z = 0. It
    /// ends where another face cuts it, and it is the cap only: not the far half of the sphere.
    #[test]
    fn a_curved_face_ends_at_the_other_faces_on_the_sheet_through_its_vertex() -> OpmResult<()> {
        let dome_with = |other: Face, side: Side| -> OpmResult<ParametricSolid> {
            let sphere = Face::new(
                SurfaceShape::Sphere(Sphere::new(millimeter!(40.0))?),
                Isometry::identity(),
            );
            ParametricSolid::new(vec![(sphere, Side::Behind), (other, side)])
        };
        let on_sphere = |x: f64| 40.0 - 40.0_f64.mul_add(40.0, -(x * x)).sqrt();
        // A rear face at z = 20 mm cuts the cap at 34.6 mm off the axis.
        let rear = Face::new(
            SurfaceShape::Plane,
            Isometry::new_along_z(millimeter!(20.0))?,
        );
        let cut = dome_with(rear, Side::InFront)?;
        assert!(cut.includes(FaceIndex::new(0), &millimeter!(10.0, 0.0, on_sphere(10.0))));
        assert!(!cut.includes(FaceIndex::new(0), &millimeter!(36.0, 0.0, on_sphere(36.0))));
        // A face that cuts nothing: the far half of the sphere is still no part of the front face.
        let behind_the_vertex = Face::new(
            SurfaceShape::Plane,
            Isometry::new_along_z(millimeter!(-10.0))?,
        );
        let uncut = dome_with(behind_the_vertex, Side::Behind)?;
        assert!(uncut.includes(FaceIndex::new(0), &millimeter!(10.0, 0.0, on_sphere(10.0))));
        assert!(!uncut.includes(
            FaceIndex::new(0),
            &millimeter!(10.0, 0.0, 80.0 - on_sphere(10.0))
        ));
        Ok(())
    }
    #[test]
    fn the_extent_of_a_parametric_solid_cannot_be_determined_yet() -> OpmResult<()> {
        let error = placed_block(&Isometry::identity())?
            .bounding_box()
            .unwrap_err();
        assert!(
            error.to_string().contains("cannot be determined"),
            "{error}"
        );
        Ok(())
    }
    #[test]
    fn a_parametric_solid_is_placed_in_the_frame_of_its_node() -> OpmResult<()> {
        let node_frame = Isometry::new(millimeter!(3.0, -2.0, 100.0), degree!(10.0, -20.0, 30.0))?;
        assert_eq!(placed_block(&node_frame)?.isometry(), node_frame);
        Ok(())
    }
}
