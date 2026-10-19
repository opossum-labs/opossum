#![warn(missing_docs)]
//! Where a face ends sideways.
//!
//! A [`GeoSurface`](crate::geometry::geo_surface::GeoSurface) is unbounded: a plane reaches in every
//! direction, a sphere closes on itself. Which of its points belong to the thing it is a face of is
//! the lateral boundary of that face, and it depends on the kind of thing. An extruded solid ends at
//! the [`Rim`](crate::geometry::Rim) of its cross section, the faces of a parametric solid end at
//! each other, a part of an assembly ends where its own solid does. [`FaceBoundary`] is the question
//! every kind answers for itself; a ray meets a face where its surface does and the boundary agrees.

use nalgebra::Point3;
use uom::si::f64::Length;

use crate::geometry::Placement;

/// Where the faces of a description end sideways.
///
/// Implemented by the description of a thing, which does not know where it sits: every method is
/// given the [`Placement`] it is built at. Asking a [`Placed`](crate::geometry::Placed) thing
/// supplies it.
pub trait FaceBoundary {
    /// Return whether a point on the unbounded surface of a face belongs to the described [`Placed`](crate::geometry::Placed) thing.
    ///
    /// # Arguments
    ///
    /// - `at`: where the [`Placed`](crate::geometry::Placed) thing sits.
    /// - `face`: the number of the face the point lies on.
    /// - `point`: a point on the surface of that face, in global coordinates.
    ///
    /// # Returns
    ///
    /// `true` if the face reaches to this point; `false` if the point lies on the surface beyond
    /// the edge of the face, or if the [`Placed`](crate::geometry::Placed) thing has no such face.
    fn includes(&self, at: Placement<'_>, face: usize, point: &Point3<Length>) -> bool;
}
