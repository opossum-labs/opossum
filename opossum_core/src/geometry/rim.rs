#![warn(missing_docs)]
//! The lateral boundary of a component.
//!
//! A component's [`GeoSurface`](crate::geometry::geo_surface::GeoSurface)s are unbounded; how far
//! the component reaches sideways is its cross section. A [`Rim`] places that cross section in
//! space and answers the one question both a [`Body`](crate::geometry::body::Body) and the
//! surfaces light is traced at have to ask: does a point lie within it? Questions about the region
//! itself, in its own plane, are answered by the [`ValidatedCrossSection`].

use nalgebra::{Point2, Point3};
use serde::{Deserialize, Serialize};
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
        self.cross_section
            .contains(Point2::new(local_point.x, local_point.y))
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
