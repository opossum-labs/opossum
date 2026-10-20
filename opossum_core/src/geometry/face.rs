#![warn(missing_docs)]
//! One optical surface of a component: its shape and the frame of its vertex.
//!
//! A [`Face`] is a [`SurfaceShape`] at a frame. Stated relative to the component it belongs to, it
//! can be compared and stored; moved to where the component sits (see [`Face::moved`]), it is what
//! rays are intersected with. The math of the shape is done in the vertex frame (see
//! [`GeoSurface`]); the face turns rays and points into that frame and back.

use nalgebra::{Point2, Point3, Vector3};
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use crate::{
    error::{OpmResult, OpossumError},
    geometry::{GeoSurface, SurfaceShape},
    light::Ray,
    nanometer, radian,
    utils::geom_transformation::Isometry,
};

/// One optical surface of a component: its [`SurfaceShape`] and the frame of its vertex.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Face {
    shape: SurfaceShape,
    frame: Isometry,
}

impl Face {
    /// Create a new [`Face`].
    ///
    /// # Arguments
    ///
    /// * `shape` - the shape of the surface.
    /// * `frame` - position and orientation of the vertex, in the frame this face is stated in.
    #[must_use]
    pub const fn new(shape: SurfaceShape, frame: Isometry) -> Self {
        Self { shape, frame }
    }
    /// Returns the shape of this [`Face`].
    #[must_use]
    pub const fn shape(&self) -> &SurfaceShape {
        &self.shape
    }
    /// Returns the frame of the vertex of this [`Face`].
    #[must_use]
    pub const fn frame(&self) -> &Isometry {
        &self.frame
    }
    /// Returns the surface of this [`Face`], to be asked in the vertex frame.
    #[must_use]
    pub fn surface(&self) -> &dyn GeoSurface {
        self.shape.as_geo_surface()
    }
    /// Return the anchor of this face: the frame of its vertex relative to the node.
    ///
    /// # Arguments
    ///
    /// * `owner` - the frame this face is stated in, relative to the node.
    #[must_use]
    pub fn anchor(&self, owner: &Isometry) -> Isometry {
        owner.append(&self.frame)
    }
    /// Return this face moved by an isometry.
    ///
    /// # Arguments
    ///
    /// * `by` - the frame this face is stated in, given in the frame it is moved to.
    ///
    /// # Returns
    ///
    /// The face with its vertex frame at `by` followed by its own frame.
    #[must_use]
    pub fn moved(&self, by: &Isometry) -> Self {
        Self {
            shape: self.shape.clone(),
            frame: by.append(&self.frame),
        }
    }
    /// Calculate the intersection point of a [`Ray`] with the unbounded surface of this face and
    /// the normal there.
    ///
    /// # Arguments
    ///
    /// * `ray` - the ray, in the frame this face is stated in.
    ///
    /// # Returns
    ///
    /// The intersection point and the normalized surface normal there, pointing against the ray
    /// direction, both in the frame this face is stated in; or `None` if the ray misses the
    /// surface.
    #[must_use]
    pub fn intersect(&self, ray: &Ray) -> Option<(Point3<Length>, Vector3<f64>)> {
        let local_ray = ray.inverse_transformed_ray(&self.frame);
        self.surface()
            .calc_intersect_and_normal(&local_ray)
            .map(|(point, normal)| {
                (
                    self.frame.transform_point(&point),
                    self.frame.transform_vector_f64(&normal),
                )
            })
    }
    /// Return whether a point lies behind the surface of this face (see [`GeoSurface::is_behind`]).
    ///
    /// # Arguments
    ///
    /// * `point` - the point, in the frame this face is stated in.
    #[must_use]
    pub fn is_behind(&self, point: &Point3<Length>) -> bool {
        self.surface()
            .is_behind(&self.frame.inverse_transform_point(point))
    }
    /// Return whether a point of the surface of this face lies on the sheet through its vertex.
    ///
    /// A [`Sphere`](crate::geometry::Sphere) or [`Cylinder`](crate::geometry::Cylinder) closes on
    /// itself, so a ray can meet it on its far side, behind the equator, which is not part of a
    /// component built from it. [`GeoSurface::local_z_at`] always answers for the sheet through
    /// the vertex, so a point lies on that sheet if its own local z agrees. A single-sheeted
    /// surface always does.
    ///
    /// # Arguments
    ///
    /// * `point` - a point on the surface, in the frame this face is stated in.
    ///
    /// # Returns
    ///
    /// `true` if the point lies on the sheet through the vertex, up to floating-point noise.
    #[must_use]
    pub fn is_on_vertex_sheet(&self, point: &Point3<Length>) -> bool {
        let local_point = self.frame.inverse_transform_point(point);
        self.surface()
            .local_z_at(&Point2::new(local_point.x, local_point.y))
            .is_some_and(|sheet_z| (local_point.z - sheet_z).abs() <= nanometer!(1.0))
    }
    /// Check that this face reaches as far out as the outline that bounds it.
    ///
    /// The vertex is taken to sit on the axis of the outline, as it does for every face the
    /// presets build.
    ///
    /// # Arguments
    ///
    /// * `transversal_reach` - how far the outline reaches from its axis.
    /// * `name` - how to name this face in the error, e.g. `"front face"`.
    ///
    /// # Errors
    ///
    /// This function returns an error if the face ends before the outline does.
    pub(crate) fn check_reach(&self, transversal_reach: Length, name: &str) -> OpmResult<()> {
        if self.surface().reaches(transversal_reach) {
            return Ok(());
        }
        Err(OpossumError::Other(format!(
            "the {name} does not reach the edge of its optic shape, {:.3} mm from the axis: its \
             radius of curvature is smaller",
            transversal_reach.get::<uom::si::length::millimeter>()
        )))
    }
    /// Whether this face is the same surface at the same place as another face.
    ///
    /// Both faces have to have the same shape, and their vertices have to sit at the same point with
    /// their axes pointing the same way. How far a face is turned about its axis matters only for a
    /// shape that is not symmetric about it, a cylinder. Deviations as small as floating-point noise
    /// are tolerated.
    ///
    /// # Arguments
    ///
    /// * `frame` - the frame this face is stated in.
    /// * `other` - the face to compare with.
    /// * `other_frame` - the frame `other` is stated in.
    ///
    /// # Returns
    ///
    /// `true` if both faces describe the same surface.
    #[must_use]
    pub fn coincides_with(&self, frame: &Isometry, other: &Self, other_frame: &Isometry) -> bool {
        // Tolerances that only absorb floating-point noise, as for the axes of two node inputs.
        let angle_tolerance = radian!(1.0e-9);
        let length_tolerance = nanometer!(1.0);
        let vertex = frame.append(&self.frame);
        let other_vertex = other_frame.append(&other.frame);
        // With both axes aligned, the local y axes have to run along the same line; half a turn
        // reverses them and still maps a cylinder onto itself.
        let same_roll = || {
            vertex
                .transform_vector_f64(&Vector3::y())
                .cross(&other_vertex.transform_vector_f64(&Vector3::y()))
                .norm()
                <= angle_tolerance.value
        };
        self.shape.matches(&other.shape, length_tolerance)
            && vertex
                .axis_mismatch(&other_vertex, angle_tolerance, length_tolerance)
                .is_none()
            && (!self.shape.depends_on_roll() || same_roll())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        degree,
        geometry::{Cylinder, Parabola, Sphere},
        millimeter,
    };
    use approx::assert_abs_diff_eq;

    /// A face is anchored at its vertex, whatever its shape: a curved surface is computed in its
    /// vertex frame, not around its center of curvature.
    #[test]
    fn a_face_is_anchored_at_its_vertex() -> OpmResult<()> {
        let owner = Isometry::new(millimeter!(0.0, 0.0, 4.0), degree!(0.0, 30.0, 0.0))?;
        let vertex = Isometry::new_along_z(millimeter!(5.0))?;
        for shape in [
            SurfaceShape::Plane,
            SurfaceShape::Sphere(Sphere::new(millimeter!(-80.0))?),
            SurfaceShape::Cylinder(Cylinder::new(millimeter!(50.0))?),
        ] {
            assert_eq!(
                Face::new(shape, vertex).anchor(&owner),
                owner.append(&vertex)
            );
        }
        Ok(())
    }
    /// A face moved to a node sits at the node followed by its own frame, and meets a ray there:
    /// a ray along the moved axis hits the vertex.
    #[test]
    fn a_moved_face_sits_at_the_node_followed_by_its_frame() -> OpmResult<()> {
        let node_frame = Isometry::new(millimeter!(1.0, 2.0, 3.0), degree!(10.0, 0.0, 5.0))?;
        let face = Face::new(
            SurfaceShape::Cylinder(Cylinder::new(millimeter!(50.0))?),
            Isometry::new_along_z(millimeter!(2.0))?,
        );
        let moved = face.moved(&node_frame);
        assert_eq!(*moved.frame(), node_frame.append(face.frame()));
        let ray = Ray::new(
            node_frame.transform_point(&millimeter!(0.0, 0.0, -10.0)),
            node_frame.transform_vector_f64(&Vector3::z()),
            nanometer!(1053.0),
            crate::joule!(1.0),
        )?;
        let (point, _) = moved
            .intersect(&ray)
            .ok_or_else(|| OpossumError::Other("the ray missed the face".into()))?;
        let vertex = moved.frame().transform_point(&Point3::origin());
        assert_abs_diff_eq!(
            (point - vertex).map(|c| c.value).norm(),
            0.0,
            epsilon = 1e-15
        );
        Ok(())
    }
    #[test]
    fn a_face_coincides_only_with_the_same_surface_at_the_same_place() -> OpmResult<()> {
        let sphere = |radius: f64, vertex: Isometry| -> OpmResult<Face> {
            Ok(Face::new(
                SurfaceShape::Sphere(Sphere::new(millimeter!(radius))?),
                vertex,
            ))
        };
        let at = |z: f64| Isometry::new_along_z(millimeter!(z));
        let frame = Isometry::identity();
        let face = sphere(50.0, at(5.0)?)?;
        // the same surface stated in another frame, or differing by rounding noise only
        assert!(face.coincides_with(&frame, &sphere(50.0, at(2.0)?)?, &at(3.0)?));
        assert!(face.coincides_with(&frame, &sphere(50.0 + 1.0e-10, at(5.0)?)?, &frame));
        // another radius, another place, the axis flipped, another shape
        for other in [
            sphere(50.001, at(5.0)?)?,
            sphere(50.0, at(5.001)?)?,
            sphere(
                50.0,
                Isometry::new(millimeter!(0.0, 0.0, 5.0), degree!(180.0, 0.0, 0.0))?,
            )?,
            Face::new(
                SurfaceShape::Cylinder(Cylinder::new(millimeter!(50.0))?),
                at(5.0)?,
            ),
        ] {
            assert!(!face.coincides_with(&frame, &other, &frame), "{other:?}");
        }
        // a parabola is compared by its focal length
        let parabola = |focal_length: f64| -> OpmResult<Face> {
            Ok(Face::new(
                SurfaceShape::Parabola(Parabola::new(millimeter!(focal_length))?),
                Isometry::identity(),
            ))
        };
        assert!(parabola(100.0)?.coincides_with(&frame, &parabola(100.0)?, &frame));
        assert!(!parabola(100.0)?.coincides_with(&frame, &parabola(101.0)?, &frame));
        Ok(())
    }
}
