#![warn(missing_docs)]
//! One optical surface of a component, described rather than built: its shape and where its vertex
//! sits.
//!
//! A [`GeoSurface`](crate::geometry::geo_surface::GeoSurface) is the runtime object rays are
//! intersected with, placed somewhere in the world. A [`Face`] is the description it is built from,
//! stated relative to the component it belongs to, so it can be compared, stored and placed anew
//! wherever the component moves.

use std::sync::{Arc, Mutex};

use nalgebra::Vector3;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use crate::{
    error::{OpmResult, OpossumError},
    geometry::{Cylinder, Parabola, Plane, Sphere, geo_surface::GeoSurfaceRef},
    nanometer, radian,
    utils::geom_transformation::Isometry,
};

/// The shape of an optical surface, independent of where it is placed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SurfaceShape {
    /// A flat surface.
    Plane,
    /// A spherical surface.
    Sphere {
        /// The radius of curvature. A positive radius has its centre behind the vertex (+z).
        radius: Length,
    },
    /// A cylindrical surface, curved along the local x axis.
    Cylinder {
        /// The radius of curvature. A positive radius has its axis behind the vertex (+z).
        radius: Length,
    },
    /// A parabolic surface.
    Parabola {
        /// The focal length of the parabola.
        focal_length: Length,
    },
}

impl SurfaceShape {
    /// A spherical surface with the given radius of curvature.
    ///
    /// # Arguments
    ///
    /// * `radius` - the radius of curvature.
    ///
    /// # Returns
    ///
    /// [`SurfaceShape::Sphere`], or [`SurfaceShape::Plane`] for an infinite radius.
    #[must_use]
    pub const fn spherical(radius: Length) -> Self {
        if radius.value.is_infinite() {
            Self::Plane
        } else {
            Self::Sphere { radius }
        }
    }
    /// A cylindrical surface with the given radius of curvature.
    ///
    /// # Arguments
    ///
    /// * `radius` - the radius of curvature.
    ///
    /// # Returns
    ///
    /// [`SurfaceShape::Cylinder`], or [`SurfaceShape::Plane`] for an infinite radius.
    #[must_use]
    pub const fn cylindrical(radius: Length) -> Self {
        if radius.value.is_infinite() {
            Self::Plane
        } else {
            Self::Cylinder { radius }
        }
    }
    /// Whether this shape is the same as another one, up to a tolerance on its radius or focal length.
    ///
    /// # Arguments
    ///
    /// * `other` - the shape to compare with.
    /// * `tolerance` - the largest difference of radius or focal length still counted as equal.
    ///
    /// # Returns
    ///
    /// `true` if both are the same kind of surface and their radius or focal length agree within
    /// `tolerance`.
    fn matches(&self, other: &Self, tolerance: Length) -> bool {
        let close = |a: &Length, b: &Length| (*a - *b).abs() <= tolerance;
        match self {
            Self::Plane => matches!(other, Self::Plane),
            Self::Sphere { radius } => {
                matches!(other, Self::Sphere { radius: other_radius } if close(radius, other_radius))
            }
            Self::Cylinder { radius } => {
                matches!(other, Self::Cylinder { radius: other_radius } if close(radius, other_radius))
            }
            Self::Parabola { focal_length } => {
                matches!(other, Self::Parabola { focal_length: other_focal_length }
                    if close(focal_length, other_focal_length))
            }
        }
    }
    /// Whether a surface of this shape reaches the given distance from its axis.
    ///
    /// A sphere or a cylinder ends at its radius of curvature; exactly reaching it (a hemisphere)
    /// is still a shape. A cylinder is held to its radius in every direction, as a body bounded by
    /// it is. The other shapes reach arbitrarily far.
    ///
    /// # Arguments
    ///
    /// * `distance` - the distance from the axis to be reached.
    ///
    /// # Returns
    ///
    /// `true` if the surface reaches that far.
    fn reaches(&self, distance: Length) -> bool {
        match self {
            Self::Plane | Self::Parabola { .. } => true,
            Self::Sphere { radius } | Self::Cylinder { radius } => distance <= radius.abs(),
        }
    }
    /// Whether turning this shape about its own axis (local z) changes it.
    ///
    /// # Returns
    ///
    /// `false` for a shape that is rotationally symmetric about its axis. `true` for a
    /// [`SurfaceShape::Cylinder`], which is curved along its local x axis only; half a turn still
    /// maps it onto itself.
    const fn depends_on_roll(&self) -> bool {
        match self {
            Self::Plane | Self::Sphere { .. } | Self::Parabola { .. } => false,
            Self::Cylinder { .. } => true,
        }
    }
    /// Where the geometric surface's own frame sits relative to the vertex.
    ///
    /// A [`Sphere`] and a [`Cylinder`] have their frame at the centre of curvature, one radius
    /// behind the vertex; every other surface has it at the vertex.
    ///
    /// # Errors
    ///
    /// This function returns an error if the radius is not finite.
    fn vertex_to_frame(&self) -> OpmResult<Isometry> {
        match self {
            Self::Sphere { radius } | Self::Cylinder { radius } => Isometry::new_along_z(*radius),
            Self::Plane | Self::Parabola { .. } => Ok(Isometry::identity()),
        }
    }
    /// Build the geometric surface of this shape with its own frame at the given isometry.
    ///
    /// # Errors
    ///
    /// This function returns an error if the radius or the focal length is zero or not finite.
    fn build(&self, frame: Isometry) -> OpmResult<GeoSurfaceRef> {
        Ok(match self {
            Self::Plane => GeoSurfaceRef(Arc::new(Mutex::new(Plane::new(frame)))),
            Self::Sphere { radius } => {
                GeoSurfaceRef(Arc::new(Mutex::new(Sphere::new(*radius, frame)?)))
            }
            Self::Cylinder { radius } => {
                GeoSurfaceRef(Arc::new(Mutex::new(Cylinder::new(*radius, frame)?)))
            }
            Self::Parabola { focal_length } => {
                GeoSurfaceRef(Arc::new(Mutex::new(Parabola::new(*focal_length, frame)?)))
            }
        })
    }
}

/// One optical surface of a component: its [`SurfaceShape`] and where its vertex sits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Face {
    shape: SurfaceShape,
    vertex: Isometry,
}

impl Face {
    /// Create a new [`Face`].
    ///
    /// # Arguments
    ///
    /// * `shape` - the shape of the surface.
    /// * `vertex` - position and orientation of the vertex in the frame of the owner of this face.
    #[must_use]
    pub const fn new(shape: SurfaceShape, vertex: Isometry) -> Self {
        Self { shape, vertex }
    }
    /// Build the geometric surface of this face.
    ///
    /// # Arguments
    ///
    /// * `node_frame` - the placement of the node the face belongs to.
    /// * `owner` - the frame this face is stated in, relative to the node.
    ///
    /// # Returns
    ///
    /// The geometric surface, placed at `node_frame` followed by the returned anchor, and that
    /// anchor: the surface's own frame relative to the node.
    ///
    /// # Errors
    ///
    /// This function returns an error if the shape cannot be built (radius or focal length zero or
    /// not finite).
    pub fn place(
        &self,
        node_frame: &Isometry,
        owner: &Isometry,
    ) -> OpmResult<(GeoSurfaceRef, Isometry)> {
        let anchor = owner
            .append(&self.vertex)
            .append(&self.shape.vertex_to_frame()?);
        Ok((self.shape.build(node_frame.append(&anchor))?, anchor))
    }
    /// Check that this face reaches as far out as the cross section that bounds it.
    ///
    /// The vertex is taken to sit on the axis of the cross section, as it does for every face the
    /// presets build.
    ///
    /// # Arguments
    ///
    /// * `transversal_reach` - how far the cross section reaches from its axis.
    /// * `name` - how to name this face in the error, e.g. `"front face"`.
    ///
    /// # Errors
    ///
    /// This function returns an error if the face ends before the cross section does.
    pub(crate) fn check_reach(&self, transversal_reach: Length, name: &str) -> OpmResult<()> {
        if self.shape.reaches(transversal_reach) {
            return Ok(());
        }
        Err(OpossumError::Other(format!(
            "the {name} does not reach the edge of its clear aperture, {:.3} mm from the axis: its \
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
        let vertex = frame.append(&self.vertex);
        let other_vertex = other_frame.append(&other.vertex);
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
    use crate::{degree, millimeter, utils::LockExt};
    use approx::assert_abs_diff_eq;
    use uom::si::length::millimeter;

    #[test]
    fn an_infinite_radius_is_flat() {
        assert_eq!(
            SurfaceShape::spherical(millimeter!(f64::INFINITY)),
            SurfaceShape::Plane
        );
        assert_eq!(
            SurfaceShape::cylindrical(millimeter!(f64::NEG_INFINITY)),
            SurfaceShape::Plane
        );
        assert_eq!(
            SurfaceShape::spherical(millimeter!(100.0)),
            SurfaceShape::Sphere {
                radius: millimeter!(100.0)
            }
        );
    }
    #[test]
    fn a_curved_face_has_its_frame_one_radius_behind_the_vertex() -> OpmResult<()> {
        let face = Face::new(
            SurfaceShape::Sphere {
                radius: millimeter!(-80.0),
            },
            Isometry::new_along_z(millimeter!(5.0))?,
        );
        let (_, anchor) = face.place(&Isometry::identity(), &Isometry::identity())?;
        assert_abs_diff_eq!(anchor.translation().z.get::<millimeter>(), -75.0);
        Ok(())
    }
    #[test]
    fn a_face_is_placed_at_the_node_followed_by_its_anchor() -> OpmResult<()> {
        let node_frame = Isometry::new(millimeter!(1.0, 2.0, 3.0), degree!(10.0, 0.0, 5.0))?;
        let owner = Isometry::new(millimeter!(0.0, 0.0, 4.0), degree!(0.0, 30.0, 0.0))?;
        let face = Face::new(
            SurfaceShape::Cylinder {
                radius: millimeter!(50.0),
            },
            Isometry::new_along_z(millimeter!(2.0))?,
        );
        let (surface, anchor) = face.place(&node_frame, &owner)?;
        let expected = node_frame.append(&anchor);
        let placed = *surface.0.lock_opm()?.isometry();
        assert_abs_diff_eq!(
            (expected.get_inv_transform() * placed.get_transform())
                .translation
                .vector
                .norm(),
            0.0,
            epsilon = 1e-15
        );
        let expected_anchor = owner
            .append(&Isometry::new_along_z(millimeter!(2.0))?)
            .append(&Isometry::new_along_z(millimeter!(50.0))?);
        assert_eq!(anchor, expected_anchor);
        Ok(())
    }
    #[test]
    fn a_face_coincides_only_with_the_same_surface_at_the_same_place() -> OpmResult<()> {
        let sphere = |radius: f64, vertex: Isometry| {
            Face::new(
                SurfaceShape::Sphere {
                    radius: millimeter!(radius),
                },
                vertex,
            )
        };
        let at = |z: f64| Isometry::new_along_z(millimeter!(z));
        let frame = Isometry::identity();
        let face = sphere(50.0, at(5.0)?);
        // the same surface stated in another frame, or differing by rounding noise only
        assert!(face.coincides_with(&frame, &sphere(50.0, at(2.0)?), &at(3.0)?));
        assert!(face.coincides_with(&frame, &sphere(50.0 + 1.0e-10, at(5.0)?), &frame));
        // another radius, another place, the axis flipped, another shape
        for other in [
            sphere(50.001, at(5.0)?),
            sphere(50.0, at(5.001)?),
            sphere(
                50.0,
                Isometry::new(millimeter!(0.0, 0.0, 5.0), degree!(180.0, 0.0, 0.0))?,
            ),
            Face::new(
                SurfaceShape::Cylinder {
                    radius: millimeter!(50.0),
                },
                at(5.0)?,
            ),
        ] {
            assert!(!face.coincides_with(&frame, &other, &frame), "{other:?}");
        }
        // a parabola is compared by its focal length
        let parabola = |focal_length: f64| {
            Face::new(
                SurfaceShape::Parabola {
                    focal_length: millimeter!(focal_length),
                },
                Isometry::identity(),
            )
        };
        assert!(parabola(100.0).coincides_with(&frame, &parabola(100.0), &frame));
        assert!(!parabola(100.0).coincides_with(&frame, &parabola(101.0), &frame));
        Ok(())
    }
    #[test]
    fn an_invalid_shape_cannot_be_placed() {
        let face = Face::new(
            SurfaceShape::Parabola {
                focal_length: millimeter!(0.0),
            },
            Isometry::identity(),
        );
        assert!(
            face.place(&Isometry::identity(), &Isometry::identity())
                .is_err()
        );
    }
}
