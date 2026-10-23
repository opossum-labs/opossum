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
/// lies where its frame says; moving the component moves it along (see [`Rim::move_by`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rim {
    outline: Outline,
    frame: Isometry,
}

impl Rim {
    /// Create a new [`Rim`].
    ///
    /// # Arguments
    ///
    /// * `outline` - the transversal extent, in the xy plane of `frame`.
    /// * `frame` - the frame of the outline, in the frame the rim is stated in; its z axis is the
    ///   direction the outline is extruded along.
    #[must_use]
    pub const fn new(outline: Outline, frame: Isometry) -> Self {
        Self { outline, frame }
    }
    /// Return the outline of this [`Rim`], in the xy plane of its frame.
    #[must_use]
    pub const fn outline(&self) -> &Outline {
        &self.outline
    }
    /// Return the frame the outline of this [`Rim`] lies in, with the z axis as the direction the
    /// outline is extruded along.
    #[must_use]
    pub const fn frame(&self) -> &Isometry {
        &self.frame
    }
    /// Move this rim by an isometry.
    ///
    /// # Arguments
    ///
    /// * `by` - the frame this rim is stated in, given in the frame it is moved to.
    pub fn move_by(&mut self, by: &Isometry) {
        self.frame = by.append(&self.frame);
    }
    /// Return whether the given point lies within this [`Rim`].
    ///
    /// # Arguments
    ///
    /// * `point` - the point to be tested, in the frame this rim is stated in.
    ///
    /// # Returns
    ///
    /// `true` if the point's projection along the axis lies within the outline.
    #[must_use]
    pub fn contains(&self, point: &Point3<Length>) -> bool {
        let local_point = self.frame.inverse_transform_point(point);
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
    /// * `point` - a point on the surface, in the frame this rim is stated in.
    #[must_use]
    pub fn bounds(&self, face: &Face, point: &Point3<Length>) -> bool {
        self.contains(point) && face.is_on_vertex_sheet(point)
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
        let mut at_reference = rim.clone();
        at_reference.move_by(&reference);
        for ((x, y, z), inside) in [
            ((3.0, 4.0, 0.0), true),
            ((6.0, 4.0, -20.0), true),
            ((3.0, 4.0, 150.0), true),
            ((3.0, 9.1, 0.0), false),
            ((-1.0, 0.5, 0.0), false),
        ] {
            assert_eq!(
                at_reference.contains(&placed(x, y, z)),
                inside,
                "({x}, {y}, {z})"
            );
        }
        // The rim follows its frame: moved 20 mm across the axis, it leaves the point behind.
        let moved = reference.append(&Isometry::new_translation(millimeter!(0.0, 20.0, 0.0))?);
        let mut aside = rim;
        aside.move_by(&moved);
        assert!(!aside.contains(&placed(3.0, 4.0, 0.0)));
        Ok(())
    }
}
