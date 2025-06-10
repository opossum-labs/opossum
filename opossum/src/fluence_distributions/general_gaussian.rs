//! Generalized 2D Gaussian distribution
use std::f64::consts::PI;

use super::FluenceDistribution;
use crate::{
    error::{OpmResult, OpossumError},
    nodes::fluence_detector::Fluence,
    utils::math_distribution_functions::quantity_2d_gaussian_point,
};
use nalgebra::Point2;
use uom::si::{
    energy::joule,
    f64::{Angle, Energy, Length},
};
pub struct General2DGaussian {
    total_energy: Energy,
    mu_xy: Point2<Length>,
    sigma_xy: Point2<Length>,
    theta: Angle,
}
impl General2DGaussian {
    /// Create a new generalized 2-dimension Gaussian energy-distribution generator [`General2DGaussian`].
    /// # Attributes
    /// - `total_energy`: total energy to distribute within the construction points
    /// - `mu_xy`: the mean value in x, y direction as Point2
    /// - `sigma_xy`: the standard deviation value in x, y direction as Point2
    /// - `theta`: rotation angle of the distribution. Counter-clockwise rotation for positive theta
    /// # Errors
    /// This function will return an error if
    ///   - the energy is non-finite, zero or below zero
    ///   - the mean values are non-finite
    ///   - the standard deviations are non-finite, zero or below zero
    ///   - the power are non-finite, zero or below zero
    ///   - the Angle is non-finite
    pub fn new(
        total_energy: Energy,
        mu_xy: Point2<Length>,
        sigma_xy: Point2<Length>,
        theta: Angle,
    ) -> OpmResult<Self> {
        if !total_energy.get::<joule>().is_normal()
            || total_energy.get::<joule>().is_sign_negative()
        {
            return Err(OpossumError::Other(
                "Energy must be greater than zero finite!".into(),
            ));
        }
        if !mu_xy.x.is_finite() || !mu_xy.y.is_finite() {
            return Err(OpossumError::Other("Mean values must be finite!".into()));
        }
        if !sigma_xy.x.is_normal()
            || !sigma_xy.y.is_normal()
            || sigma_xy.x.is_sign_negative()
            || sigma_xy.y.is_sign_negative()
        {
            return Err(OpossumError::Other(
                "Standard deviations must be greater than zero and finite!".into(),
            ));
        }
        if !theta.is_finite() {
            return Err(OpossumError::Other(
                "Angle the distribution must be finite!".into(),
            ));
        }
        Ok(Self {
            total_energy,
            mu_xy,
            sigma_xy,
            theta,
        })
    }
}

impl FluenceDistribution for General2DGaussian {
    fn apply(&self, input: &[Point2<Length>]) -> Vec<Fluence> {
        let mut fluence_distribution = Vec::<Fluence>::with_capacity(input.len());
        let peak_val = self.total_energy / (2. * PI * self.sigma_xy.x * self.sigma_xy.y);
        for p in input {
            fluence_distribution.push(quantity_2d_gaussian_point(
                peak_val,
                p,
                self.mu_xy,
                self.sigma_xy,
                self.theta,
            ));
        }
        fluence_distribution
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{joule, meter, radian};
    #[test]
    fn new_gaussian_sigma() {
        assert!(
            General2DGaussian::new(joule!(1.), meter!(0., 0.), meter!(0., 1.), radian!(0.),)
                .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(f64::NAN, 1.),
                radian!(0.),
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(f64::INFINITY, 1.),
                radian!(0.),
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(f64::NEG_INFINITY, 1.),
                radian!(0.),
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(joule!(1.), meter!(0., 0.), meter!(-1., 1.), radian!(0.),)
                .is_err()
        );

        assert!(
            General2DGaussian::new(joule!(1.), meter!(0., 0.), meter!(1., 0.), radian!(0.),)
                .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., f64::NAN),
                radian!(0.),
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., f64::INFINITY),
                radian!(0.)
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., f64::NEG_INFINITY),
                radian!(0.),
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(joule!(1.), meter!(0., 0.), meter!(1., -1.), radian!(0.),)
                .is_err()
        );
    }
    #[test]
    fn new_gaussian_energy() {
        assert!(
            General2DGaussian::new(
                joule!(f64::NAN),
                meter!(0., 0.),
                meter!(1., 1.),
                radian!(0.),
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(f64::INFINITY),
                meter!(0., 0.),
                meter!(1., 1.),
                radian!(0.)
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(f64::NEG_INFINITY),
                meter!(0., 0.),
                meter!(1., 1.),
                radian!(0.)
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(joule!(-1.), meter!(0., 0.), meter!(1., 1.), radian!(0.))
                .is_err()
        );
        assert!(
            General2DGaussian::new(joule!(0.), meter!(0., 0.), meter!(1., 1.), radian!(0.))
                .is_err()
        );
        assert!(
            General2DGaussian::new(joule!(1.), meter!(0., 0.), meter!(1., 1.), radian!(0.)).is_ok()
        );
    }
    #[test]
    fn new_gaussian_mean() {
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(f64::NAN, 0.),
                meter!(1., 1.),
                radian!(0.)
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(f64::INFINITY, 0.),
                meter!(1., 1.),
                radian!(0.)
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(f64::NEG_INFINITY, 0.),
                meter!(1., 1.),
                radian!(0.)
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(joule!(1.), meter!(-10., 0.), meter!(1., 1.), radian!(0.))
                .is_ok()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., f64::NAN),
                meter!(1., 1.),
                radian!(0.)
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., f64::INFINITY),
                meter!(1., 1.),
                radian!(0.)
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., f64::NEG_INFINITY),
                meter!(1., 1.),
                radian!(0.)
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(joule!(1.), meter!(0., -10.), meter!(1., 1.), radian!(0.))
                .is_ok()
        );
    }

    #[test]
    fn new_gaussian_angle() {
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., 1.),
                radian!(f64::NAN),
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., 1.),
                radian!(f64::INFINITY),
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., 1.),
                radian!(f64::NEG_INFINITY),
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(joule!(1.), meter!(0., 0.), meter!(1., 1.), radian!(-10.),)
                .is_ok()
        );
    }
}
