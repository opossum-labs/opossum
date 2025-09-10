use super::{ApertureType, Apodize};
use crate::{
    apertures::Shape,
    error::{OpmResult, OpossumError},
};
use nalgebra::Point2;
use serde::{Deserialize, Serialize};
use uom::si::{f64::Length, ratio::ratio};

/// Configuration data for a Gaussian aperture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GaussianShape {
    sigma: (Length, Length),
    center: Point2<Length>,
    aperture_type: ApertureType,
}
impl GaussianShape {
    /// Create a Gaussian aperture configurartion given by `(sigma_x, sigma_y)` as well as the center point.
    ///
    /// By default the aperture has the aperture type [`ApertureType::Hole`].
    /// # Errors
    ///
    /// This function will return an error if the given waists are negative and / or the center point is indefinite.
    pub fn new(sigma: (Length, Length), center: Point2<Length>) -> OpmResult<Self> {
        if sigma.0.is_normal()
            && sigma.0.is_sign_positive()
            && sigma.1.is_normal()
            && sigma.1.is_sign_positive()
            && center.coords[0].is_finite()
            && center.coords[1].is_finite()
        {
            Ok(Self {
                sigma,
                center,
                aperture_type: ApertureType::default(),
            })
        } else {
            Err(OpossumError::Other("parameters out of range".into()))
        }
    }
    /// Returns the sigma of this [`GaussianConfig`].
    #[must_use]
    pub fn sigma(&self) -> (Length, Length) {
        self.sigma
    }
    /// Returns the center of this [`GaussianConfig`].
    #[must_use]
    pub fn center(&self) -> Point2<Length> {
        self.center
    }
}
impl Shape for GaussianShape {
    fn transmission_factor(&self, point: &Point2<Length>) -> f64 {
        let x_c = self.center.coords[0];
        let y_c = self.center.coords[1];
        let x = point.coords[0];
        let y = point.coords[1];
        (-0.5
            * (((x - x_c) / self.sigma.0).get::<ratio>().mul_add(
                ((x - x_c) / self.sigma.0).get::<ratio>(),
                ((y - y_c) / self.sigma.1).get::<ratio>().powi(2),
            )))
        .exp()
    }
}
impl Apodize for GaussianShape {
    fn set_aperture_type(&mut self, aperture_type: ApertureType) {
        self.aperture_type = aperture_type;
    }
    fn apodize(&self, point: &Point2<Length>) -> f64 {
        let x_c = self.center.coords[0];
        let y_c = self.center.coords[1];
        let x = point.coords[0];
        let y = point.coords[1];
        let mut transmission = (-0.5
            * (((x - x_c) / self.sigma.0).get::<ratio>().mul_add(
                ((x - x_c) / self.sigma.0).get::<ratio>(),
                ((y - y_c) / self.sigma.1).get::<ratio>().powi(2),
            )))
        .exp();
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
}
