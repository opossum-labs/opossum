#![warn(missing_docs)]
//! Module for handling bounded bodies.
//!
//! A [`GeoSurface`](crate::geometry::geo_surface::GeoSurface) is an unbounded interface: it answers
//! where a [`Ray`] hits it, but it has no notion of an inside. A [`Body`] is the complementary
//! concept — a closed volume that knows which points lie within it and how far a [`Ray`] travels
//! through it. This is the domain any volumetric quantity (such as an inversion density) is defined
//! on.
//!
//! So far the only implementation is a [`Placed`](crate::geometry::Placed)
//! [`Extruded`](crate::geometry::Extruded) solid, a volume bounded by an entrance surface, an exit
//! surface and the transversal cross section of its rim; it implements the trait next to its
//! definition. Since the bounding surfaces are the ones a node already uses for refraction, the
//! usual optical volumes are simply different parameter sets of the same solid: a disk is two
//! [`Plane`](crate::geometry::Plane)s with a circular cross section, a slab two planes with a
//! rectangular one, a lens two [`Sphere`](crate::geometry::Sphere)s.
//!
//! The cross section is expressed as an [`Aperture`](crate::apertures::Aperture), but only those
//! apertures that actually delimit a region qualify, which is what
//! [`ValidatedCrossSection`](crate::types::validated_type_definitions::ValidatedCrossSection)
//! guarantees — an aperture is in general a transmission mask, and softening, inverting or omitting
//! the transmission edge says nothing about where the material ends.

use crate::{
    apertures::{CircleShape, OpticShape, RectangleShape},
    error::{OpmResult, OpossumError},
    light::Ray,
    millimeter,
    utils::geom_transformation::Isometry,
};
use nalgebra::Point3;
use std::{fmt::Debug, ops::Range};
use uom::si::f64::Length;

/// Name of the property holding the transversal extent of a component: its optic shape.
///
/// The optic shape is the size the component is actually available in — the figure a supplier
/// quotes next to the curvatures and the thickness. It bounds the surfaces light is traced at and,
/// for a volume node, the medium between them (see [`Rim`]).
///
/// Every volume node has a bounded one: a component of unstated size has no volume to speak of,
/// and two curved surfaces only happen to close on their own if they are curved strongly enough.
/// Only what merely records light, is an idealization or has no optical function accepts an open
/// optic shape.
pub const OPTIC_SHAPE: &str = "optic shape";

/// The optic shape a round optic starts out with if nothing is defined.
///
/// A circle of 12.5 mm radius: the 25 mm (1 inch) mount most catalogue optics come in.
///
/// # Returns
///
/// The default transversal extent of a round optic.
///
/// # Panics
///
/// Panics if the hard-coded radius is rejected by [`CircleShape`], which cannot happen.
#[must_use]
pub fn default_optic_shape() -> OpticShape {
    CircleShape::new(millimeter!(12.5))
        .expect("12.5 mm is a valid aperture radius")
        .into()
}

/// The optic shape a grating or a detector starts out with if nothing is defined.
///
/// A square of 25 mm × 25 mm.
///
/// # Returns
///
/// The default transversal extent of a grating, or the default window of a detector.
///
/// # Panics
///
/// Panics if the hard-coded size is rejected by [`RectangleShape`], which cannot happen.
#[must_use]
pub fn default_square_optic_shape() -> OpticShape {
    RectangleShape::new(millimeter!(25.0), millimeter!(25.0))
        .expect("25 mm is a valid rectangle size")
        .into()
}

/// An axis-aligned box enclosing a [`Body`].
///
/// The box is expressed in the body's own frame ([`Body::isometry`]) rather than in global
/// coordinates: a discretisation of the body has to be axis-aligned with the component it describes,
/// not with the laboratory, or a tilted component would be sampled on a staircase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    min: Point3<Length>,
    max: Point3<Length>,
}

impl BoundingBox {
    /// Create a new [`BoundingBox`] from two opposite corners.
    ///
    /// # Arguments
    ///
    /// - `min`: the corner with the smallest coordinate on every axis
    /// - `max`: the corner with the largest coordinate on every axis
    ///
    /// # Returns
    ///
    /// The box spanned by the two corners. A box may be flat on an axis (a body of zero thickness
    /// is still a body), but not inverted.
    ///
    /// # Errors
    ///
    /// This function returns an error if any coordinate is not finite or if `min` exceeds `max` on
    /// any axis.
    pub fn new(min: Point3<Length>, max: Point3<Length>) -> OpmResult<Self> {
        if min.iter().chain(max.iter()).any(|c| !c.is_finite()) {
            return Err(OpossumError::Other(
                "the corners of a bounding box must be finite".into(),
            ));
        }
        if min.iter().zip(max.iter()).any(|(low, high)| low > high) {
            return Err(OpossumError::Other(
                "the lower corner of a bounding box must not exceed its upper corner".into(),
            ));
        }
        Ok(Self { min, max })
    }
    /// Return the extent of this [`BoundingBox`] along the x axis.
    #[must_use]
    pub fn x_range(&self) -> Range<Length> {
        self.min.x..self.max.x
    }
    /// Return the extent of this [`BoundingBox`] along the y axis.
    #[must_use]
    pub fn y_range(&self) -> Range<Length> {
        self.min.y..self.max.y
    }
    /// Return the extent of this [`BoundingBox`] along the z axis.
    #[must_use]
    pub fn z_range(&self) -> Range<Length> {
        self.min.z..self.max.z
    }
}

/// Trait for handling closed bodies.
///
/// A body is the volumetric counterpart of a
/// [`GeoSurface`](crate::geometry::geo_surface::GeoSurface): it defines an inside and can state how
/// far a [`Ray`] travels within it.
pub trait Body: Debug + Send + Sync {
    /// Return whether the given point lies inside this [`Body`].
    ///
    /// # Arguments
    ///
    /// - `point`: the point to be tested, given in global coordinates.
    ///
    /// # Returns
    ///
    /// `true` if the point lies inside the body. The body is half-open along the direction of
    /// propagation: a point lying exactly on the entrance boundary counts as inside, one lying
    /// exactly on the exit boundary does not. This way neighbouring bodies sharing a surface do not
    /// both claim the points on it.
    fn contains(&self, point: &Point3<Length>) -> bool;
    /// Return the geometrical path length of the given [`Ray`] inside this [`Body`].
    ///
    /// The ray is not refracted on the way — the returned length is the straight chord between the
    /// point where the ray enters the body (or its starting point, if it already starts inside) and
    /// the point where it leaves the body again.
    ///
    /// # Arguments
    ///
    /// - `ray`: the ray to be traced through the body.
    ///
    /// # Returns
    ///
    /// The path length inside the body or `None` if the ray does not pass through it.
    fn path_length_inside(&self, ray: &Ray) -> Option<Length>;
    /// Return an axis-aligned box that encloses this [`Body`].
    ///
    /// [`Body::contains`] answers where the body is *not*, one point at a time; this states where it
    /// is worth asking at all. Anything discretising the body needs both: the box gives the domain
    /// to lay a grid over, [`Body::contains`] carves the body out of it.
    ///
    /// # Returns
    ///
    /// A [`BoundingBox`] containing the whole body, expressed in the body's own frame (see
    /// [`Body::isometry`]). It is not required to be the tightest such box — a grid masks its cells
    /// individually anyway — but a body must never reach outside it.
    ///
    /// # Errors
    ///
    /// This function returns an error if the extent of the body cannot be determined.
    fn bounding_box(&self) -> OpmResult<BoundingBox>;
    /// Return the frame this [`Body`] is placed in.
    ///
    /// This is the frame [`Body::bounding_box`] is expressed in, so it is what turns that box back
    /// into the global coordinates [`Body::contains`] expects. It is the frame of the component the
    /// body belongs to, which is what makes a grid over the body follow the component when it is
    /// moved or tilted.
    fn isometry(&self) -> Isometry;
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::millimeter;

    #[test]
    fn a_bounding_box_may_be_flat_but_not_inverted() {
        assert!(BoundingBox::new(millimeter!(0.0, 0.0, 0.0), millimeter!(1.0, 1.0, 0.0)).is_ok());
        assert!(BoundingBox::new(millimeter!(0.0, 0.0, 1.0), millimeter!(1.0, 1.0, 0.0)).is_err());
        assert!(
            BoundingBox::new(millimeter!(0.0, 0.0, f64::NAN), millimeter!(1.0, 1.0, 1.0)).is_err()
        );
    }
}
