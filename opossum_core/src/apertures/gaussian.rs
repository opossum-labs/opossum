use super::Shape;
use crate::generic_validators::ValidateTrait;
use crate::prelude::ApertureShape;
use crate::{error::OpmResult, types::validated_type_definitions::ValidatedSideLengths2D};
use nalgebra::{Point2, Point3};
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, ratio::ratio};
use utoipa::ToSchema;

/// Configuration data for a Gaussian aperture.
#[derive(
    Copy, Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema, EnsureValidated, Default,
)]
pub struct GaussianShape {
    #[schema(value_type = Object)]
    sigma: ValidatedSideLengths2D,
}
impl From<GaussianShape> for ApertureShape {
    fn from(value: GaussianShape) -> Self {
        Self::Gaussian(value)
    }
}
impl GaussianShape {
    /// Create a Gaussian aperture configurartion given by `(sigma_x, sigma_y)` as well as the center point.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given waists are negative and / or the center point is indefinite.
    pub fn new(sigma: (Length, Length)) -> OpmResult<Self> {
        let sigma_validated = ValidatedSideLengths2D::try_new(Point2::new(sigma.0, sigma.1))?;
        Ok(Self {
            sigma: sigma_validated,
        })
    }
    /// Returns the sigma of this [`GaussianShape`].
    #[must_use]
    pub fn sigma(&self) -> (Length, Length) {
        let sigma = self.sigma.get();
        (sigma.x, sigma.y)
    }

    /// Set the x-sigma value of this [`GaussianShape`].
    /// # Errors
    /// This function will return an error if the given x-sigma is negative or indefinite.
    pub fn set_sigma_x(&mut self, sigma_x: Length) -> OpmResult<()> {
        self.sigma.set(Point2::new(sigma_x, self.sigma().1))?;
        Ok(())
    }

    /// Set the y-sigma value of this [`GaussianShape`].
    /// # Errors
    /// This function will return an error if the given y-sigma is negative or indefinite.
    pub fn set_sigma_y(&mut self, sigma_y: Length) -> OpmResult<()> {
        self.sigma.set(Point2::new(self.sigma().0, sigma_y))?;
        Ok(())
    }
}
impl Shape for GaussianShape {
    fn transmission_factor(&self, point: &Point3<Length>) -> f64 {
        let x = point.x;
        let y = point.y;
        (-0.5
            * ((x / self.sigma.get().x).get::<ratio>().mul_add(
                (x / self.sigma.get().x).get::<ratio>(),
                (y / self.sigma.get().y).get::<ratio>().powi(2),
            )))
        .exp()
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::meter;
    use approx::assert_abs_diff_eq;

    #[test]
    fn new() {
        assert!(GaussianShape::new((meter!(1.0), meter!(1.0))).is_ok());
        assert!(GaussianShape::new((meter!(0.0), meter!(1.0))).is_err());
        assert!(GaussianShape::new((meter!(-1.0), meter!(1.0))).is_err());
        assert!(GaussianShape::new((meter!(1.0), meter!(0.0))).is_err());
        assert!(GaussianShape::new((meter!(1.0), meter!(-1.0))).is_err());
        assert!(GaussianShape::new((meter!(f64::NAN), meter!(1.0))).is_err());
        assert!(GaussianShape::new((meter!(f64::INFINITY), meter!(1.0))).is_err());
        assert!(GaussianShape::new((meter!(1.0), meter!(f64::NAN))).is_err());
        assert!(GaussianShape::new((meter!(1.0), meter!(f64::INFINITY))).is_err());
    }
    #[test]
    fn getters() -> OpmResult<()> {
        let g = GaussianShape::new((meter!(1.0), meter!(2.0)))?;
        assert_eq!(g.sigma(), (meter!(1.0), meter!(2.0)));
        Ok(())
    }
    #[test]
    fn transmission_factor() -> OpmResult<()> {
        let g = GaussianShape::new((meter!(1.0), meter!(1.0)))?;
        assert_eq!(g.transmission_factor(&meter!(0.0, 0.0, 0.0)), 1.0);
        assert_eq!(
            g.transmission_factor(&meter!(-1.0, -1.0, 0.0)),
            1.0 / 1.0_f64.exp()
        );
        Ok(())
    }
    #[test]
    fn test_sigma_decay() -> OpmResult<()> {
        let sigma_x = meter!(1.0);
        let sigma_y = meter!(2.0);
        let g = GaussianShape::new((sigma_x, sigma_y))?;

        // Center (0 sigma)
        assert_eq!(g.transmission_factor(&meter!(0.0, 0.0, 0.0)), 1.0);

        // 1 sigma in X: exp(-0.5 * (1/1)^2) = exp(-0.5)
        let t_1sigma_x = g.transmission_factor(&meter!(1.0, 0.0, 0.0));
        assert_abs_diff_eq!(t_1sigma_x, (-0.5_f64).exp(), epsilon = 1e-12);

        // 1 sigma in Y: exp(-0.5 * (2/2)^2) = exp(-0.5)
        let t_1sigma_y = g.transmission_factor(&meter!(0.0, 2.0, 0.0));
        assert_abs_diff_eq!(t_1sigma_y, (-0.5_f64).exp(), epsilon = 1e-12);
        Ok(())
    }
}
