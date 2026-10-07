#![warn(missing_docs)]
//! The lateral boundary of a component.
//!
//! A component's [`GeoSurface`](crate::geometry::geo_surface::GeoSurface)s are unbounded; how far
//! the component reaches sideways is its cross section. A [`Rim`] answers the one question both a
//! [`Body`](crate::geometry::body::Body) and the surfaces light is traced at have to ask: does a
//! point lie within that cross section?

use nalgebra::Point3;
use uom::si::f64::Length;

use crate::{
    types::validated_type_definitions::ValidatedCrossSection, utils::geom_transformation::Isometry,
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
}

impl Rim {
    /// Create a new [`Rim`].
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
        }
    }
    /// Return the cross section of this [`Rim`], in the xy plane of its axis.
    #[must_use]
    pub const fn cross_section(&self) -> &ValidatedCrossSection {
        &self.cross_section
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
        let local_point = reference.append(&self.axis).inverse_transform_point(point);
        // The cross section is a binary hole, so a transmission above zero means "inside".
        self.cross_section.get().apodize(&local_point) > 0.0
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
