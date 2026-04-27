use super::Shape;
use crate::{error::OpmResult, millimeter, generic_validators::ValidateTrait, types::validated_type_definitions::{ValidatedCenter, ValidatedRadius}};
use nalgebra::{Isometry2, Point2};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::meter};
use utoipa::ToSchema;

/// Configuration data for a circular aperture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema, EnsureValidated, Default)]
pub struct CircleShape {
    #[schema(value_type = f64)]
    radius: ValidatedRadius,
    #[schema(value_type = Object)]
    center: ValidatedCenter
}

impl CircleShape {
    /// Create a new [`CircleShape`] from a given radius and a center point.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given radius of negative, `NaN` or `Infinity`.
    pub fn new(radius: Length, center: Point2<Length>) -> OpmResult<Self> {
        let validated_radius = ValidatedRadius::try_new(radius)?;
        let validated_center = ValidatedCenter::try_new(center)?;
        Ok(Self { radius: validated_radius, center: validated_center })
    }
    
    /// Returns the radius of this [`CircleShape`]
    #[must_use]
    pub fn radius(&self) -> Length {
        *self.radius.get()
    }
    /// Returns the center of this [`CircleShape`].
    #[must_use]
    pub fn center(&self) -> Point2<Length> {
        *self.center.get()
    }
}
impl Shape for CircleShape {
    fn transmission_factor(&self, point: &Point2<Length>) -> f64 {
        let translation = Isometry2::translation(
            self.center().x.get::<meter>(),
            self.center().y.get::<meter>(),
        );

        let point_meter = Point2::<f64>::new(point.x.get::<meter>(), point.y.get::<meter>());
        let point_transformed = translation.inverse_transform_point(&point_meter);
        if point_transformed
            .y
            .mul_add(point_transformed.y, point_transformed.x.powi(2))
            <= self.radius().get::<meter>().powi(2)
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
    fn getters() {
        let c = CircleShape::new(meter!(2.0), meter!(3.0, 4.0)).unwrap();
        assert_eq!(c.radius(), meter!(2.0));
        assert_eq!(c.center().x, meter!(3.0));
        assert_eq!(c.center().y, meter!(4.0));
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
    #[test]
    fn test_boundary_conditions() {
        let radius = meter!(1.0);
        let c = CircleShape::new(radius, meter!(0.0, 0.0)).unwrap();

        // Point exactly on the boundary (x^2 + y^2 == r^2)
        // Floating point precision might be tricky, but 1.0^2 is exact.
        assert_eq!(c.transmission_factor(&meter!(1.0, 0.0)), 1.0);
        assert_eq!(c.transmission_factor(&meter!(0.0, 1.0)), 1.0);

        // Slightly outside
        assert_eq!(c.transmission_factor(&meter!(1.000001, 0.0)), 0.0);
    }
}
