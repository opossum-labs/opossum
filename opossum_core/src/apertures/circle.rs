use crate::{
    apertures::Shape,
    error::{OpmResult, OpossumError},
};
use nalgebra::{Isometry2, Point2};
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::meter};

use super::{ApertureType, Apodize};

/// Configuration data for a circular aperture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CircleShape {
    radius: Length,
    center: Point2<Length>,
    aperture_type: ApertureType,
}
impl CircleShape {
    /// Create a new [`CircleShape`] from a given radius and a center point.
    ///
    /// By default the aperture has the aperture type [`ApertureType::Hole`].
    ///
    /// # Errors
    ///
    /// This function will return an error if the given radius of negative, NaN or Infinity.
    pub fn new(radius: Length, center: Point2<Length>) -> OpmResult<Self> {
        if radius.is_normal() && radius.is_sign_positive() {
            Ok(Self {
                radius,
                center,
                aperture_type: ApertureType::default(),
            })
        } else {
            Err(OpossumError::Other("radius must be positive".into()))
        }
    }
    /// Returns the radius of this [`CircleConfig`]
    #[must_use]
    pub fn radius(&self) -> &Length {
        &self.radius
    }
    /// Returns the center of this [`CircleConfig`].
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
impl Apodize for CircleShape {
    fn set_aperture_type(&mut self, aperture_type: ApertureType) {
        self.aperture_type = aperture_type;
    }
    fn apodize(&self, point: &Point2<Length>) -> f64 {
        let translation = Isometry2::translation(
            self.center.coords[0].get::<meter>(),
            self.center.coords[1].get::<meter>(),
        );

        let point_meter = Point2::<f64>::new(point.x.get::<meter>(), point.y.get::<meter>());
        let point_transformed = translation.inverse_transform_point(&point_meter);
        let mut transmission = if point_transformed
            .y
            .mul_add(point_transformed.y, point_transformed.x.powi(2))
            <= self.radius.get::<meter>().powi(2)
        {
            1.0
        } else {
            0.0
        };

        if matches!(self.aperture_type, ApertureType::Obstruction) {
            transmission = 1.0 - transmission;
        }
        transmission
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
}
