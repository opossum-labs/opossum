use super::Shape;
use crate::error::{OpmResult, OpossumError};
use nalgebra::{Isometry2, Point2};
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::meter};

/// Configuration data for a circular aperture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CircleShape {
    radius: Length,
    center: Point2<Length>,
}
impl CircleShape {
    /// Create a new [`CircleShape`] from a given radius and a center point.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given radius of negative, NaN or Infinity.
    pub fn new(radius: Length, center: Point2<Length>) -> OpmResult<Self> {
        if radius.is_normal() && radius.is_sign_positive() {
            Ok(Self { radius, center })
        } else {
            Err(OpossumError::Other("radius must be positive".into()))
        }
    }
    /// Returns the radius of this [`CircleShape`]
    #[must_use]
    pub fn radius(&self) -> &Length {
        &self.radius
    }
    /// Returns the center of this [`CircleShape`].
    #[must_use]
    pub fn center(&self) -> Point2<Length> {
        self.center
    }
}
impl Shape for CircleShape {
    fn transmission_factor(&self, point: &Point2<Length>) -> f64 {
        let translation = Isometry2::translation(
            self.center.coords[0].get::<meter>(),
            self.center.coords[1].get::<meter>(),
        );

        let point_meter = Point2::<f64>::new(point.x.get::<meter>(), point.y.get::<meter>());
        let point_transformed = translation.inverse_transform_point(&point_meter);
        if point_transformed
            .y
            .mul_add(point_transformed.y, point_transformed.x.powi(2))
            <= self.radius.get::<meter>().powi(2)
        {
            1.0
        } else {
            0.0
        }
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::meter;

    #[test]
    fn new() {
        let center = meter!(0.0, 0.0);
        assert!(CircleShape::new(meter!(1.0), center).is_ok());
        assert!(CircleShape::new(meter!(0.0), center).is_err());
        assert!(CircleShape::new(meter!(-1.0), center).is_err());
        assert!(CircleShape::new(meter!(f64::NAN), center).is_err());
        assert!(CircleShape::new(meter!(f64::INFINITY), center).is_err());
    }
    #[test]
    fn transmission_factor() {
        let c = CircleShape::new(meter!(1.0), meter!(1.0, 1.0)).unwrap();
        assert_eq!(c.transmission_factor(&meter!(1.0, 1.0)), 1.0);
        assert_eq!(c.transmission_factor(&meter!(1.0, 0.0)), 1.0);
        assert_eq!(c.transmission_factor(&meter!(1.0, 2.0)), 1.0);
        assert_eq!(c.transmission_factor(&meter!(2.0, 1.0)), 1.0);
        assert_eq!(c.transmission_factor(&meter!(0.0, 1.0)), 1.0);
        assert_eq!(c.transmission_factor(&meter!(0.0, 0.0)), 0.0);
        assert_eq!(c.transmission_factor(&meter!(2.0, 2.0)), 0.0);
    }
}
