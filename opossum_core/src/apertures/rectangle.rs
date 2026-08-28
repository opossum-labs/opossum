use nalgebra::{Point2, Point3, Vector2};
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::meter};
use utoipa::ToSchema;

use super::Shape;
use crate::{
    apertures::ApertureShape, error::OpmResult,
    types::validated_type_definitions::ValidatedSideLengths2D,
};
use opm_macros_lib::EnsureValidated;
/// Configuration data for a rectangular aperture.
#[derive(
    Copy, Debug, Clone, Serialize, Deserialize, PartialEq, EnsureValidated, ToSchema, Default,
)]
pub struct RectangleShape {
    #[schema(value_type = Object)]
    side_length: ValidatedSideLengths2D,
}

impl From<RectangleShape> for ApertureShape {
    fn from(rect: RectangleShape) -> Self {
        Self::BinaryRectangle(rect)
    }
}

impl RectangleShape {
    /// Create a new rectangular aperture configuration by given width, height and the center point.
    ///
    /// # Errors
    ///
    /// This function will return an error if width and/or height are negative, NaN or Infinity.
    pub fn new(width: Length, height: Length) -> OpmResult<Self> {
        let mut new_rect = Self::default();
        new_rect.set_width(width)?;
        new_rect.set_height(height)?;
        Ok(new_rect)
    }

    /// Returns the width of this [`RectangleShape`].
    #[must_use]
    pub fn width(&self) -> Length {
        self.side_length.get().x
    }
    /// Returns the height of this [`RectangleShape`].
    #[must_use]
    pub fn height(&self) -> Length {
        self.side_length.get().y
    }
    /// Sets the width of this [`RectangleShape`].
    /// # Errors
    /// This function will return an error if the given width is negative, NaN or Infinity
    pub fn set_width(&mut self, width: Length) -> OpmResult<()> {
        self.side_length.set(Point2::new(width, self.height()))?;
        Ok(())
    }
    /// Sets the height of this [`RectangleShape`].
    /// # Errors
    /// This function will return an error if the given height is negative, NaN or Infinity
    pub fn set_height(&mut self, height: Length) -> OpmResult<()> {
        self.side_length.set(Point2::new(self.width(), height))?;
        Ok(())
    }
}
impl Shape for RectangleShape {
    fn transmission_factor(&self, point: &Point3<Length>) -> f64 {
        let q = Vector2::new(
            point.x.value.abs() - self.side_length.get().x.get::<meter>() / 2.,
            point.y.value.abs() - self.side_length.get().y.get::<meter>() / 2.,
        );
        let mut q_max = q;
        q_max.iter_mut().for_each(|x: &mut f64| *x = x.max(0.0));
        let sdf_val = q_max.x.mul_add(q_max.x, q_max.y.powi(2)).sqrt() + q.x.max(q.y).min(0.0);

        if sdf_val <= 0. { 1.0 } else { 0.0 }
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::meter;
    #[test]
    fn new() {
        assert!(RectangleShape::new(meter!(2.0), meter!(1.0)).is_ok());
        assert!(RectangleShape::new(meter!(0.0), meter!(1.0)).is_err());
        assert!(RectangleShape::new(meter!(-1.0), meter!(1.0)).is_err());
        assert!(RectangleShape::new(meter!(f64::NAN), meter!(1.0)).is_err());
        assert!(RectangleShape::new(meter!(f64::INFINITY), meter!(1.0)).is_err());
        assert!(RectangleShape::new(meter!(1.0), meter!(0.0)).is_err());
        assert!(RectangleShape::new(meter!(1.0), meter!(-1.0)).is_err());
        assert!(RectangleShape::new(meter!(1.0), meter!(f64::NAN)).is_err());
        assert!(RectangleShape::new(meter!(1.0), meter!(f64::INFINITY)).is_err());
    }
    #[test]
    fn getters() -> OpmResult<()> {
        let r = RectangleShape::new(meter!(2.0), meter!(1.0))?;
        assert_eq!(r.width(), meter!(2.0));
        assert_eq!(r.height(), meter!(1.0));
        Ok(())
    }
    #[test]
    fn transmission_factor() -> OpmResult<()> {
        let r = RectangleShape::new(meter!(1.0), meter!(2.0))?;
        assert_eq!(r.transmission_factor(&meter!(0.0, 0.0, 0.)), 1.0);
        assert_eq!(r.transmission_factor(&meter!(0.5, 0.0, 0.)), 1.0);
        assert_eq!(r.transmission_factor(&meter!(0.5, 1.0, 0.)), 1.0);
        assert_eq!(r.transmission_factor(&meter!(-0.5, 1.0, 0.)), 1.0);
        assert_eq!(r.transmission_factor(&meter!(-0.5, -1.0, 0.)), 1.0);
        assert_eq!(r.transmission_factor(&meter!(-1.0, -1.0, 0.)), 0.0);
        assert_eq!(r.transmission_factor(&meter!(0.0, 1.1, 0.)), 0.0);
        Ok(())
    }
    #[test]
    fn from() {
        let rs = RectangleShape::default();
        let aps: ApertureShape = rs.into();
        assert!(matches!(aps, ApertureShape::BinaryRectangle(_)))
    }
}
