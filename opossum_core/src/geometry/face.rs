#![warn(missing_docs)]
//! One optical surface of a component, described rather than built: its shape and where its vertex
//! sits.
//!
//! A [`GeoSurface`](crate::geometry::geo_surface::GeoSurface) is the runtime object rays are
//! intersected with, placed somewhere in the world. A [`Face`] is the description it is built from,
//! stated relative to the component it belongs to, so it can be compared, stored and placed anew
//! wherever the component moves.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use crate::{
    error::OpmResult,
    geometry::{Cylinder, Parabola, Plane, Sphere, geo_surface::GeoSurfaceRef},
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
