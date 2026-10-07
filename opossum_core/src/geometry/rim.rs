#![warn(missing_docs)]
//! The lateral boundary of a component.
//!
//! A component's [`GeoSurface`](crate::geometry::geo_surface::GeoSurface)s are unbounded; how far
//! the component reaches sideways is its cross section. A [`Rim`] answers the one question both a
//! [`Body`](crate::geometry::body::Body) and the surfaces light is traced at have to ask: does a
//! point lie within that cross section?

use nalgebra::{Point2, Point3};
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use crate::{
    apertures::ApertureShape,
    error::{OpmResult, OpossumError},
    meter,
    types::validated_type_definitions::ValidatedCrossSection,
    utils::geom_transformation::Isometry,
};

/// The lateral boundary of a component: its cross section, extruded along an axis.
///
/// A point lies within the rim if its projection along the axis falls into the cross section. The
/// rim stores no absolute frame. It is stated relative to a reference frame handed to
/// [`Rim::contains`] (the node's frame for a component, the body's own frame for a
/// [`SurfaceBoundedBody`](crate::geometry::body::SurfaceBoundedBody)), so it follows wherever that
/// frame is when it is asked.
#[derive(Debug, Clone, PartialEq)]
pub struct Rim {
    cross_section: ValidatedCrossSection,
    axis: Isometry,
    role: RimRole,
}

/// What a [`Rim`] means for a ray that meets the unbounded surface outside of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RimRole {
    /// The component ends at the rim: a ray outside misses it, and the analyzer's missed surface
    /// strategy decides what happens to the ray.
    Edge,
    /// The rim only bounds what a detector records: a ray outside passes the surface like one
    /// inside, but is not recorded and leaves no hit.
    Window,
}

impl Rim {
    /// Create a new [`Rim`] the component ends at ([`RimRole::Edge`]).
    ///
    /// # Arguments
    ///
    /// * `cross_section` - the transversal extent, in the xy plane of `axis`.
    /// * `axis` - the frame of the cross section relative to the reference frame; its z axis is the
    ///   direction the cross section is extruded along.
    #[must_use]
    pub const fn new(cross_section: ValidatedCrossSection, axis: Isometry) -> Self {
        Self {
            cross_section,
            axis,
            role: RimRole::Edge,
        }
    }
    /// Create a new [`Rim`] that only bounds what a detector records ([`RimRole::Window`]).
    ///
    /// # Arguments
    ///
    /// See [`Rim::new`].
    #[must_use]
    pub const fn window(cross_section: ValidatedCrossSection, axis: Isometry) -> Self {
        Self {
            cross_section,
            axis,
            role: RimRole::Window,
        }
    }
    /// Return the cross section of this [`Rim`], in the xy plane of its axis.
    #[must_use]
    pub const fn cross_section(&self) -> &ValidatedCrossSection {
        &self.cross_section
    }
    /// Return what this [`Rim`] means for a ray outside of it.
    #[must_use]
    pub const fn role(&self) -> RimRole {
        self.role
    }
    /// Return the frame the cross section of this [`Rim`] lies in.
    ///
    /// # Arguments
    ///
    /// * `reference` - the frame this rim is stated relative to.
    ///
    /// # Returns
    ///
    /// The reference frame followed by the axis, in global coordinates.
    #[must_use]
    pub fn frame(&self, reference: &Isometry) -> Isometry {
        reference.append(&self.axis)
    }
    /// Return whether the given point lies within this [`Rim`].
    ///
    /// # Arguments
    ///
    /// * `point` - the point to be tested, in global coordinates.
    /// * `reference` - the frame this rim is stated relative to.
    ///
    /// # Returns
    ///
    /// `true` if the point's projection along the axis lies within the cross section.
    #[must_use]
    pub fn contains(&self, point: &Point3<Length>, reference: &Isometry) -> bool {
        let local_point = self.frame(reference).inverse_transform_point(point);
        // The cross section is a binary hole, so a transmission above zero means "inside".
        self.cross_section.get().apodize(&local_point) > 0.0
    }
    /// Return the points the cross section of this [`Rim`] reaches farthest in, in the xy plane of
    /// its axis.
    ///
    /// Both the axis-aligned transversal bounds and the largest distance from the axis follow from
    /// these points. They are not a tessellation of the outline: a circle is described by four
    /// points plus, if it is shifted off the axis, the single point of it lying farthest out — a
    /// direction none of the axis-aligned extremes points in.
    ///
    /// # Returns
    ///
    /// The extreme points of the cross section, with the isometry of its
    /// [`Aperture`](crate::apertures::Aperture) already applied.
    ///
    /// # Errors
    ///
    /// This function returns an error if the cross section is not one of the binary shapes. Since
    /// [`ValidatedCrossSection`] admits nothing else, that cannot happen for a rim built through its
    /// constructor.
    pub(crate) fn outline(&self) -> OpmResult<Vec<Point2<Length>>> {
        let cross_section = self.cross_section.get();
        let transform = |point: Point2<Length>| {
            cross_section.isometry().map_or(point, |iso| {
                let transformed =
                    iso.transform_point(&Point3::new(point.x, point.y, Length::zero()));
                Point2::new(transformed.x, transformed.y)
            })
        };
        match cross_section.shape() {
            ApertureShape::BinaryCircle(circle) => {
                // A circle is indifferent to the rotation of its aperture, so only its center
                // moves. The axis-aligned bounds follow from that center alone, while the point
                // farthest from the axis lies on the far side of the shifted circle.
                let center = transform(Point2::origin());
                let radius = circle.radius();
                let mut outline = vec![
                    Point2::new(center.x + radius, center.y),
                    Point2::new(center.x - radius, center.y),
                    Point2::new(center.x, center.y + radius),
                    Point2::new(center.x, center.y - radius),
                ];
                let shift = center.x.value.hypot(center.y.value);
                if shift > 0.0 {
                    let stretch = 1.0 + radius.value / shift;
                    outline.push(Point2::new(center.x * stretch, center.y * stretch));
                }
                Ok(outline)
            }
            ApertureShape::BinaryRectangle(rectangle) => {
                let half_width = rectangle.width() / 2.0;
                let half_height = rectangle.height() / 2.0;
                Ok([
                    Point2::new(half_width, half_height),
                    Point2::new(-half_width, half_height),
                    Point2::new(-half_width, -half_height),
                    Point2::new(half_width, -half_height),
                ]
                .map(transform)
                .to_vec())
            }
            ApertureShape::BinaryPolygon(polygon) => {
                Ok(polygon.points().iter().map(|p| transform(*p)).collect())
            }
            shape => Err(OpossumError::Other(format!(
                "the extent of a component bounded by a '{shape}' cross section is undefined"
            ))),
        }
    }
    /// Return how far the cross section of this [`Rim`] reaches from its axis.
    ///
    /// # Returns
    ///
    /// The largest distance of a point of the cross section from the axis.
    ///
    /// # Errors
    ///
    /// This function returns an error if the outline of the cross section cannot be determined (see
    /// [`Rim::outline`]).
    pub(crate) fn transversal_reach(&self) -> OpmResult<Length> {
        Ok(self.outline()?.iter().fold(Length::zero(), |reach, point| {
            Length::max(reach, meter!(point.x.value.hypot(point.y.value)))
        }))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        apertures::{Aperture, ApertureType},
        degree,
        error::OpmResult,
        millimeter,
    };
    use nalgebra::Point2;

    #[test]
    fn rim_contains_respects_a_decentred_tilted_frame() -> OpmResult<()> {
        // A circle of 5 mm around (3, 4) mm, extruded along the reference's x axis.
        let axis = Isometry::new(millimeter!(0.0, 0.0, 2.0), degree!(0.0, 90.0, 0.0))?;
        let rim = Rim::new(
            ValidatedCrossSection::try_new(Aperture::new_circle(
                millimeter!(5.0),
                ApertureType::Hole,
                Some(Point2::new(millimeter!(3.0), millimeter!(4.0))),
            )?)?,
            axis,
        );
        let reference = Isometry::new(millimeter!(10.0, 0.0, 0.0), degree!(0.0, 0.0, 30.0))?;
        // Points are stated in the frame of the cross section and placed from there.
        let placed = |x: f64, y: f64, z: f64| {
            reference
                .append(&axis)
                .transform_point(&millimeter!(x, y, z))
        };
        // Along the axis the rim reaches arbitrarily far; across it only as far as the circle.
        for ((x, y, z), inside) in [
            ((3.0, 4.0, 0.0), true),
            ((6.0, 4.0, -20.0), true),
            ((3.0, 4.0, 150.0), true),
            ((3.0, 9.1, 0.0), false),
            ((-1.0, 0.5, 0.0), false),
        ] {
            assert_eq!(
                rim.contains(&placed(x, y, z), &reference),
                inside,
                "({x}, {y}, {z})"
            );
        }
        // The rim follows its reference: moved 20 mm across the axis, it leaves the point behind.
        let moved = reference.append(&Isometry::new_translation(millimeter!(0.0, 20.0, 0.0))?);
        assert!(!rim.contains(&placed(3.0, 4.0, 0.0), &moved));
        Ok(())
    }
}
