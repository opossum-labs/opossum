use super::Shape;
use crate::{
    error::OpmResult, generic_validators::ValidateTrait, prelude::ApertureShape,
    types::validated_type_definitions::ValidatedRadius,
};
use nalgebra::Point3;
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, length::meter};
use utoipa::ToSchema;

/// Configuration data for a circular aperture.
#[derive(
    Copy, Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema, EnsureValidated, Default,
)]
pub struct CircleShape {
    #[schema(value_type = f64)]
    radius: ValidatedRadius,
}

impl From<CircleShape> for ApertureShape {
    fn from(value: CircleShape) -> Self {
        Self::BinaryCircle(value)
    }
}

impl CircleShape {
    /// Create a new [`CircleShape`] from a given radius and a center point.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given radius of negative, `NaN` or `Infinity`.
    pub fn new(radius: Length) -> OpmResult<Self> {
        let validated_radius = ValidatedRadius::try_new(radius)?;
        Ok(Self {
            radius: validated_radius,
        })
    }

    /// Returns the radius of this [`CircleShape`]
    #[must_use]
    pub fn radius(&self) -> Length {
        *self.radius.get()
    }

    /// set the radius of this [`CircleShape`]
    /// # Errors
    /// This function will return an error if the given radius of negative, `NaN`
    pub fn set_radius(&mut self, radius: Length) -> OpmResult<()> {
        self.radius.set(radius)?;
        Ok(())
    }
}
impl Shape for CircleShape {
    fn transmission_factor(&self, point: &Point3<Length>) -> f64 {
        if point.y.value.mul_add(point.y.value, point.x.value.powi(2))
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
    use crate::{meter, millimeter};

    #[test]
    fn new() {
        assert!(CircleShape::new(meter!(1.0)).is_ok());
        assert!(CircleShape::new(meter!(0.0)).is_ok());
        assert!(CircleShape::new(meter!(-1.0)).is_err());
        assert!(CircleShape::new(meter!(f64::NAN)).is_err());
        assert!(CircleShape::new(meter!(f64::INFINITY)).is_err());
    }
    #[test]
    fn getters() -> OpmResult<()> {
        let c = CircleShape::new(meter!(2.0))?;
        assert_eq!(c.radius(), meter!(2.0));
        Ok(())
    }
    #[test]
    fn transmission_factor() -> OpmResult<()> {
        let c = CircleShape::new(meter!(1.0))?;
        assert_eq!(c.transmission_factor(&meter!(0.0, 0.0, 0.0)), 1.0);
        assert_eq!(c.transmission_factor(&meter!(0.0, -1.0, 0.0)), 1.0);
        assert_eq!(c.transmission_factor(&meter!(0.0, 1.0, 0.0)), 1.0);
        assert_eq!(c.transmission_factor(&meter!(1.0, 0.0, 0.0)), 1.0);
        assert_eq!(c.transmission_factor(&meter!(-1.0, 0.0, 0.0)), 1.0);
        assert_eq!(c.transmission_factor(&meter!(-1.0, -1.0, 0.0)), 0.0);
        assert_eq!(c.transmission_factor(&meter!(1.0, 1.0, 0.0)), 0.0);
        Ok(())
    }
    #[test]
    fn test_boundary_conditions() -> OpmResult<()> {
        let radius = meter!(1.0);
        let c = CircleShape::new(radius)?;

        // Point exactly on the boundary (x^2 + y^2 == r^2)
        // Floating point precision might be tricky, but 1.0^2 is exact.
        assert_eq!(c.transmission_factor(&meter!(1.0, 0.0, 0.0)), 1.0);
        assert_eq!(c.transmission_factor(&meter!(0.0, 1.0, 0.0)), 1.0);

        // Slightly outside
        assert_eq!(c.transmission_factor(&meter!(1.000001, 0.0, 0.0)), 0.0);
        Ok(())
    }
    #[test]
    fn from() {
        let cs = CircleShape::default();
        let aps: ApertureShape = cs.into();
        assert!(matches!(aps, ApertureShape::BinaryCircle(_)))
    }
    #[test]
    fn set_radius() -> OpmResult<()> {
        let mut cs = CircleShape::default();
        assert!(cs.set_radius(millimeter!(1.23)).is_ok());
        assert_eq!(cs.radius.get(), &millimeter!(1.23));
        assert!(cs.set_radius(millimeter!(-0.1)).is_err());
        assert!(cs.set_radius(millimeter!(f64::NAN)).is_err());
        assert!(cs.set_radius(millimeter!(f64::INFINITY)).is_err());
        assert!(cs.set_radius(millimeter!(f64::NEG_INFINITY)).is_err());
        assert!(cs.set_radius(millimeter!(0.0)).is_ok());
        Ok(())
    }
}
