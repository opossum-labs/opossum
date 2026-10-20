#![warn(missing_docs)]
//! The lateral boundary of a component.
//!
//! The [`Face`]s of a component are unbounded; how far the component reaches sideways is the
//! [`Outline`] of its optic shape. A [`Rim`] places that outline in space and answers the
//! questions both a [`Body`](crate::geometry::body::Body) and the surfaces light is traced at have
//! to ask: does a point lie within it, and does a point on the unbounded surface belong to the
//! component? Questions about the region itself, in its own plane, are answered by the
//! [`Outline`].

use nalgebra::{Point2, Point3};
use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use crate::{apertures::Outline, geometry::Face, utils::geom_transformation::Isometry};

/// The lateral boundary of a component: its outline, extruded along an axis.
///
/// A point lies within the rim if its projection along the axis falls into the outline. The rim
/// stores no absolute frame. It is stated relative to a reference frame handed to
/// [`Rim::contains`], which is the node's frame for the surfaces of a component as well as for the
/// body of a placed [`Extruded`](crate::geometry::Extruded) solid. The rim therefore follows
/// wherever that frame is when it is asked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rim {
    outline: Outline,
    axis: Isometry,
}

impl Rim {
    /// Create a new [`Rim`].
    ///
    /// # Arguments
    ///
    /// * `outline` - the transversal extent, in the xy plane of `axis`.
    /// * `axis` - the frame of the outline relative to the reference frame; its z axis is the
    ///   direction the outline is extruded along.
    #[must_use]
    pub const fn new(outline: Outline, axis: Isometry) -> Self {
        Self { outline, axis }
    }
    /// Return the outline of this [`Rim`], in the xy plane of its axis.
    #[must_use]
    pub const fn outline(&self) -> &Outline {
        &self.outline
    }
    /// Return the axis of this [`Rim`]: the frame of its outline relative to the reference frame,
    /// with the z axis as the direction the outline is extruded along.
    #[must_use]
    pub const fn axis(&self) -> &Isometry {
        &self.axis
    }
    /// Return the frame the outline of this [`Rim`] lies in.
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
    /// `true` if the point's projection along the axis lies within the outline.
    #[must_use]
    pub fn contains(&self, point: &Point3<Length>, reference: &Isometry) -> bool {
        let local_point = self.frame(reference).inverse_transform_point(point);
        self.outline
            .contains(Point2::new(local_point.x, local_point.y))
    }
    /// Return whether a point on an unbounded surface is part of the component this [`Rim`] bounds.
    ///
    /// A point outside the rim is not part of the component, and neither is one on the far side of
    /// a closed surface behind its equator (see [`Face::is_on_vertex_sheet`]).
    ///
    /// # Arguments
    ///
    /// * `face` - the face with the unbounded surface.
    /// * `point` - a point on the surface, in global coordinates.
    /// * `reference` - the frame this rim is stated relative to.
    #[must_use]
    pub fn bounds(&self, face: &Face, point: &Point3<Length>, reference: &Isometry) -> bool {
        self.contains(point, reference) && face.is_on_vertex_sheet(point)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{apertures::CircleShape, degree, error::OpmResult, millimeter};

    #[test]
    fn rim_contains_respects_a_decentred_tilted_frame() -> OpmResult<()> {
        // A circle of 5 mm around (3, 4) mm of a plane, extruded along the reference's x axis.
        let plane = Isometry::new(millimeter!(0.0, 0.0, 2.0), degree!(0.0, 90.0, 0.0))?;
        let rim = Rim::new(
            Outline::new(CircleShape::new(millimeter!(5.0))?, Vec::new())?,
            plane.append(&Isometry::new_translation(millimeter!(3.0, 4.0, 0.0))?),
        );
        let reference = Isometry::new(millimeter!(10.0, 0.0, 0.0), degree!(0.0, 0.0, 30.0))?;
        // Points are stated in that plane and placed from there.
        let placed = |x: f64, y: f64, z: f64| {
            reference
                .append(&plane)
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
