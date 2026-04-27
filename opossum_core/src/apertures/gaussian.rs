use super::Shape;
use crate::{error::{OpmResult, OpossumError}, millimeter, types::validated_type_definitions::{ValidatedCenter, ValidatedSideLengths}};
use nalgebra::Point2;
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, ratio::ratio};
use utoipa::ToSchema;
use crate::{generic_validators::ValidateTrait};

/// Configuration data for a Gaussian aperture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema, EnsureValidated, Default)]
pub struct GaussianShape {
    #[schema(value_type = Object)]
    sigma: ValidatedSideLengths,
    #[schema(value_type = Object)]
    center: ValidatedCenter,
}

impl GaussianShape {
    /// Create a Gaussian aperture configurartion given by `(sigma_x, sigma_y)` as well as the center point.
    ///
    /// # Errors
    ///
    /// This function will return an error if the given waists are negative and / or the center point is indefinite.
    pub fn new(sigma: (Length, Length), center: Point2<Length>) -> OpmResult<Self> {
        let sigma_validated = ValidatedSideLengths::try_new(Point2::new(sigma.0, sigma.1))?;
        let center_validated = ValidatedCenter::try_new(center)?;
        Ok(Self { sigma: sigma_validated, center: center_validated })
    }
    /// Returns the sigma of this [`GaussianShape`].
    #[must_use]
    pub fn sigma(&self) -> (Length, Length) {
        let sigma = self.sigma.get();
        (sigma.x, sigma.y)
    }
    /// Returns the center of this [`GaussianShape`].
    #[must_use]
    pub fn center(&self) -> Point2<Length> {
        *self.center.get()
    }
}
impl Shape for GaussianShape {
    fn transmission_factor(&self, point: &Point2<Length>) -> f64 {
        let x_c = self.center.get().x;
        let y_c = self.center.get().y;
        let x = point.x;
        let y = point.y;
        (-0.5
            * (((x - x_c) / self.sigma.get().x).get::<ratio>().mul_add(
                ((x - x_c) / self.sigma.get().x).get::<ratio>(),
                ((y - y_c) / self.sigma.get().y).get::<ratio>().powi(2),
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
        let p = meter!(0.0, 0.0);
        assert!(GaussianShape::new((meter!(1.0), meter!(1.0)), p).is_ok());
        assert!(GaussianShape::new((meter!(0.0), meter!(1.0)), p).is_err());
        assert!(GaussianShape::new((meter!(-1.0), meter!(1.0)), p).is_err());
        assert!(GaussianShape::new((meter!(1.0), meter!(0.0)), p).is_err());
        assert!(GaussianShape::new((meter!(1.0), meter!(-1.0)), p).is_err());
        assert!(GaussianShape::new((meter!(f64::NAN), meter!(1.0)), p).is_err());
        assert!(GaussianShape::new((meter!(f64::INFINITY), meter!(1.0)), p).is_err());
        assert!(GaussianShape::new((meter!(1.0), meter!(f64::NAN)), p).is_err());
        assert!(GaussianShape::new((meter!(1.0), meter!(f64::INFINITY)), p).is_err());
        let p = meter!(f64::NAN, 0.0);
        assert!(GaussianShape::new((meter!(1.0), meter!(1.0)), p).is_err());
        let p = meter!(f64::INFINITY, 0.0);
        assert!(GaussianShape::new((meter!(1.0), meter!(1.0)), p).is_err());
    }
    #[test]
    fn getters() {
        let g = GaussianShape::new((meter!(1.0), meter!(2.0)), meter!(3.0, 4.0)).unwrap();
        assert_eq!(g.sigma(), (meter!(1.0), meter!(2.0)));
        assert_eq!(g.center().x, meter!(3.0));
        assert_eq!(g.center().y, meter!(4.0));
    }
    #[test]
    fn transmission_factor() {
        let g = GaussianShape::new((meter!(1.0), meter!(1.0)), meter!(1.0, 1.0)).unwrap();
        assert_eq!(g.transmission_factor(&meter!(1.0, 1.0)), 1.0);
        assert_eq!(
            g.transmission_factor(&meter!(0.0, 0.0)),
            1.0 / 1.0_f64.exp()
        );
    }
    #[test]
    fn test_sigma_decay() {
        let sigma_x = meter!(1.0);
        let sigma_y = meter!(2.0);
        let g = GaussianShape::new((sigma_x, sigma_y), meter!(0.0, 0.0)).unwrap();

        // Center (0 sigma)
        assert_eq!(g.transmission_factor(&meter!(0.0, 0.0)), 1.0);

        // 1 sigma in X: exp(-0.5 * (1/1)^2) = exp(-0.5)
        let t_1sigma_x = g.transmission_factor(&meter!(1.0, 0.0));
        assert_abs_diff_eq!(t_1sigma_x, (-0.5_f64).exp(), epsilon = 1e-12);

        // 1 sigma in Y: exp(-0.5 * (2/2)^2) = exp(-0.5)
        let t_1sigma_y = g.transmission_factor(&meter!(0.0, 2.0));
        assert_abs_diff_eq!(t_1sigma_y, (-0.5_f64).exp(), epsilon = 1e-12);
    }
}
