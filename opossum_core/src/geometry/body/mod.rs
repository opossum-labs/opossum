#![warn(missing_docs)]
//! Module for handling bounded bodies.
//!
//! A [`GeoSurface`](crate::geometry::GeoSurface) is an unbounded interface: it answers
//! where a [`Ray`] hits it, but it has no notion of an inside. A [`Body`] is the complementary
//! concept — a closed volume that knows which points lie within it and how far a [`Ray`] travels
//! through it. This is the domain any volumetric quantity (such as an inversion density) is defined
//! on.
//!
//! A body answers in the frame its solid is stated in; moved to its node (see
//! [`Geometry::moved`](crate::geometry::Geometry::moved)), it answers there. Every kind of solid
//! implements the trait next to its definition and hands it out through
//! [`GeometryKind::as_body`](crate::geometry::GeometryKind::as_body): an
//! [`Extruded`](crate::geometry::Extruded) solid, a volume bounded
//! by two faces and the transversal cross section of its rim; a
//! [`ParametricSolid`](crate::geometry::ParametricSolid), bounded by its faces alone; and an
//! [`Assembly`](crate::geometry::Assembly), the parts of which answer for themselves. Since the
//! bounding surfaces are the ones a node already uses for refraction, the usual optical volumes are
//! simply different parameter sets of the same solid: a disk is two
//! [`Plane`](crate::geometry::Plane)s with a circular cross section, a slab two planes with a
//! rectangular one, a lens two [`Sphere`](crate::geometry::Sphere)s.
//!
//! The cross section is the [`Outline`](crate::apertures::Outline) of the optic shape: an edge
//! with bores through it, which always delimits a region.

use crate::{
    apertures::{CircleShape, OpticShape, RectangleShape},
    error::{OpmResult, OpossumError},
    geometry::Side,
    light::Ray,
    millimeter, nanometer,
    utils::{geom_transformation::Isometry, math_utils::distance_3d_point},
};
use nalgebra::{Point3, Vector3};
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
/// [`GeoSurface`](crate::geometry::GeoSurface): it defines an inside and can state how
/// far a [`Ray`] travels within it.
///
/// Implemented by every kind of solid, next to its definition.
pub trait Body: Debug + Send + Sync {
    /// Return whether the given point lies inside this [`Body`].
    ///
    /// # Arguments
    ///
    /// - `point`: the point to be tested, in the frame the body is stated in.
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
    /// Return which side of one of its faces this [`Body`] lies on.
    ///
    /// # Arguments
    ///
    /// - `face`: the number of the face, as [`GeometryKind`](crate::geometry::GeometryKind) numbers
    ///   them.
    ///
    /// # Returns
    ///
    /// The side of the face the body lies on, or `None` if there is no such face.
    fn side(&self, face: usize) -> Option<Side>;
    /// Return the length of the straight chord a [`Ray`] cuts through this [`Body`], found from the
    /// points where it meets the boundary.
    ///
    /// This is the one way [`Body::path_length_inside`] is answered for a body that knows its
    /// boundary as surfaces, so it is provided here rather than written once per body. It is not a
    /// question a body answers for itself, which is why it has no place among the required methods.
    ///
    /// The ray is not refracted on the way. The chord runs between the outermost two of the points
    /// at which the ray can enter or leave the body: its own starting point, if it travels inside
    /// from there, and the points at which it meets the boundary.
    ///
    /// # Arguments
    ///
    /// - `ray_position`: where the ray starts.
    /// - `ray_direction`: the direction the ray travels in.
    /// - `boundary_hits`: the points at which the ray meets the boundary of the body, each only
    ///   where the boundary really is (not on the unbounded surface beyond an edge).
    ///
    /// # Returns
    ///
    /// The length of the chord, or `None` if the ray does not pass through the body.
    fn chord_through(
        &self,
        ray_position: Point3<Length>,
        ray_direction: Vector3<f64>,
        boundary_hits: impl IntoIterator<Item = Point3<Length>>,
    ) -> Option<Length>
    where
        Self: Sized,
    {
        // A ray refracted at a boundary starts on it, and rounding may put it just outside (and its
        // hit there at t = -ε). Whether it travels inside is asked 1 nm further along.
        let probe = ray_position + ray_direction.normalize().map(|c| nanometer!(c));
        let position_along_ray =
            |point: &Point3<Length>| (point - ray_position).map(|c| c.value).dot(&ray_direction);
        // Only the outermost two candidates matter, so they are tracked directly instead of being
        // collected and sorted. Their number is counted rather than derived from their positions:
        // two candidates may well coincide - a ray starting on the boundary hits it at zero
        // distance - and such a ray does travel a (zero) length inside the body, whereas a single
        // candidate means it never entered.
        let mut candidates = 0_usize;
        let mut first: Option<(f64, Point3<Length>)> = None;
        let mut last: Option<(f64, Point3<Length>)> = None;
        let mut consider = |point: Point3<Length>| {
            candidates += 1;
            let position = position_along_ray(&point);
            if first.is_none_or(|(first_position, _)| position < first_position) {
                first = Some((position, point));
            }
            if last.is_none_or(|(last_position, _)| position > last_position) {
                last = Some((position, point));
            }
        };
        if self.contains(&probe) {
            consider(ray_position);
        }
        boundary_hits.into_iter().for_each(consider);
        let (Some((_, first_point)), Some((_, last_point))) = (first, last) else {
            return None;
        };
        if candidates < 2 {
            // The ray either misses the body altogether or leaves it through a boundary that is
            // not part of the model.
            return None;
        }
        // Guard against bodies the ray leaves and re-enters, e.g. a strongly curved meniscus: the
        // section between the outermost two candidates is only fully inside if its center is.
        let center_point = first_point + (last_point - first_point).map(|c| c * 0.5);
        self.contains(&center_point)
            .then(|| distance_3d_point(&first_point, &last_point))
    }
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
