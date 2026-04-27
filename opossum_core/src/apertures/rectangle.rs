use nalgebra::{Isometry2, Point2, Vector2};
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::meter};
use utoipa::ToSchema;

use super::Shape;
use crate::{
    error::OpmResult,
    generic_validators::ValidateTrait,
    millimeter,
    types::validated_type_definitions::{ValidatedCenter, ValidatedSideLengths},
    validated,
};
use opm_macros_lib::EnsureValidated;
/// Configuration data for a rectangular aperture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnsureValidated, ToSchema, Default)]
pub struct RectangleShape {
    #[schema(value_type = Object)]
    side_length: ValidatedSideLengths,
    #[schema(value_type = Object)]
    center: ValidatedCenter,
}

impl RectangleShape {
    /// Create a new rectangular aperture configuration by given width, height and the center point.
    ///
    /// # Errors
    ///
    /// This function will return an error if width and/or height are negative, NaN or Infinity.
    pub fn new(width: Length, height: Length, center: Point2<Length>) -> OpmResult<Self> {
        let mut new_rect = Self::default();
        new_rect.set_width(width)?;
        new_rect.set_height(height)?;
        new_rect.set_center_x(center.x)?;
        new_rect.set_center_y(center.y)?;
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
    /// Returns the center of this [`RectangleShape`].
    #[must_use]
    pub fn center(&self) -> Point2<Length> {
        *self.center.get()
    }
    /// Sets the width of this [`RectangleShape`].
    pub fn set_width(&mut self, width: Length) -> OpmResult<()> {
        self.side_length.set(Point2::new(width, self.height()))?;
        Ok(())
    }
    /// Sets the height of this [`RectangleShape`].
    pub fn set_height(&mut self, height: Length) -> OpmResult<()> {
        self.side_length.set(Point2::new(self.width(), height))?;
        Ok(())
    }
    /// Sets the x-center of this [`RectangleShape`].
    pub fn set_center_x(&mut self, center_x: Length) -> OpmResult<()> {
        self.center
            .set(Point2::new(center_x, self.center.get().y))?;
        Ok(())
    }
    /// Sets the y-center of this [`RectangleShape`].
    pub fn set_center_y(&mut self, center_y: Length) -> OpmResult<()> {
        self.center
            .set(Point2::new(self.center.get().x, center_y))?;
        Ok(())
    }
}
impl Shape for RectangleShape {
    fn transmission_factor(&self, point: &Point2<Length>) -> f64 {
        let translation = Isometry2::translation(
            self.center.get().x.get::<meter>(),
            self.center.get().y.get::<meter>(),
        );
        let point_meter = Point2::<f64>::new(point.x.get::<meter>(), point.y.get::<meter>());
        let point_transformed = translation.inverse_transform_point(&point_meter);

        let q = Vector2::new(
            point_transformed.x.abs() - self.side_length.get().x.get::<meter>() / 2.,
            point_transformed.y.abs() - self.side_length.get().y.get::<meter>() / 2.,
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
        let p = meter!(0.0, 0.0);
        assert!(RectangleShape::new(meter!(2.0), meter!(1.0), p).is_ok());
        assert!(RectangleShape::new(meter!(0.0), meter!(1.0), p).is_err());
        assert!(RectangleShape::new(meter!(-1.0), meter!(1.0), p).is_err());
        assert!(RectangleShape::new(meter!(f64::NAN), meter!(1.0), p).is_err());
        assert!(RectangleShape::new(meter!(f64::INFINITY), meter!(1.0), p).is_err());
        assert!(RectangleShape::new(meter!(1.0), meter!(0.0), p).is_err());
        assert!(RectangleShape::new(meter!(1.0), meter!(-1.0), p).is_err());
        assert!(RectangleShape::new(meter!(1.0), meter!(f64::NAN), p).is_err());
        assert!(RectangleShape::new(meter!(1.0), meter!(f64::INFINITY), p).is_err());
        let p = meter!(f64::NAN, 0.0);
        assert!(RectangleShape::new(meter!(2.0), meter!(1.0), p).is_err());
        let p = meter!(f64::INFINITY, 0.0);
        assert!(RectangleShape::new(meter!(2.0), meter!(1.0), p).is_err());
    }
    #[test]
    fn getters() {
        let r = RectangleShape::new(meter!(2.0), meter!(1.0), meter!(3.0, 4.0)).unwrap();
        assert_eq!(r.width(), meter!(2.0));
        assert_eq!(r.height(), meter!(1.0));
        assert_eq!(r.center().x, meter!(3.0));
        assert_eq!(r.center().y, meter!(4.0));
    }
    #[test]
    fn transmission_factor() {
        let r = RectangleShape::new(meter!(1.0), meter!(2.0), meter!(1.0, 1.0)).unwrap();
        assert_eq!(r.transmission_factor(&meter!(1.0, 1.0)), 1.0);
        assert_eq!(r.transmission_factor(&meter!(1.5, 1.0)), 1.0);
        assert_eq!(r.transmission_factor(&meter!(1.5, 2.0)), 1.0);
        assert_eq!(r.transmission_factor(&meter!(0.5, 2.0)), 1.0);
        assert_eq!(r.transmission_factor(&meter!(0.5, 0.0)), 1.0);
        assert_eq!(r.transmission_factor(&meter!(0.0, 0.0)), 0.0);
        assert_eq!(r.transmission_factor(&meter!(1.0, 2.1)), 0.0);
    }
}
