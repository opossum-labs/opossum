use kahan::KahanSummator;
use nalgebra::Point2;
use uom::si::f64::{Angle, Energy};
use crate::{error::{OpmResult, OpossumError}, utils::math_distribution_functions::general_2d_gaussian};
use super::EnergyDistribution;
pub struct General2DGaussian {
    mu_x: f64,
    mu_y: f64,
    sigma_x: f64,
    sigma_y: f64,
    power: f64,
    theta: Angle,
    rectangular: bool,
}
impl General2DGaussian {
    /// Create a new generalized 2-dimension Gaussian enery-distribution generator [`General2DGaussian`].
    /// # Attributes
    /// - `mu_x`: the mean value in x direction -> Shifts the distribution in x direction to be centered at `mu_x`
    /// - `mu_y`: the mean value in y direction -> Shifts the distribution in y direction to be centered at `mu_y`
    /// - `sigma_x`: the standard deviation value in x direction
    /// - `sigma_y`: the standard deviation value in y direction
    /// - `power`: the power of the distribution. A standard Gaussian distribution has a power of 1. Larger powers are so called super-Gaussians
    /// - `theta`: rotation angle of the distribution. Counter-clockwise rotation for positive theta
    /// - `rect_flag`: defines if the distribution will be shaped elliptically or rectangularly. Difference between these modes vanishes for power = 1
    /// # Errors
    /// This function will return an error if
    ///   - the mean values are non-finite
    ///   - the standard deviations are non-finite, zero or below zero
    ///   - the power are non-finite, zero or below zero
    ///   - the Angle is non-finite
    pub fn new(
        mu_x: f64,
        mu_y: f64,
        sigma_x: f64,
        sigma_y: f64,
        power: f64,
        theta: Angle,
        rectangular: bool,
    ) -> OpmResult<Self> {
        if !mu_x.is_finite() || !mu_y.is_finite() {
            return Err(OpossumError::Other(
                "Mean values must be finite!".into(),
            ));
        };
        if !sigma_x.is_normal() || !sigma_y.is_normal() || sigma_x.is_sign_negative() || sigma_y.is_sign_negative() {
            return Err(OpossumError::Other(
                "Standard deviations must be greater than zero and finite!".into(),
            ));
        };
        if !power.is_finite(){
            return Err(OpossumError::Other(
                "Power of the distribution must be positive and finite!".into(),
            ));
        };

        if !theta.is_finite() {
            return Err(OpossumError::Other(
                "Angle the distribution must be finite!".into(),
            ));
        };

        Ok(Self {
            mu_x,
            mu_y,
            sigma_x,
            sigma_y,
            power,
            theta,
            rectangular,
        })
    }
}
impl EnergyDistribution for General2DGaussian {
    fn apply(&self, total_energy: Energy, input: Vec<Point2<f64>>) -> Vec<Energy> {
        let energy_distribution = general_2d_gaussian(input, self.mu_x, self.mu_y, self.sigma_x, self.sigma_y, self.power, self.theta, self.rectangular);
        let current_energy: f64 = energy_distribution.iter().kahan_sum().sum();

        energy_distribution.iter().map(|x| total_energy**x/current_energy).collect::<Vec<Energy>>()
    }
}

#[cfg(test)]
mod test {
    use uom::si::angle::radian;

    use super::*;
    #[test]
    fn new_gaussian_sigma(){
        assert!(General2DGaussian::new(0., 0., 0., 1., 1., Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new(0., 0., f64::NAN, 1., 1., Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new(0., 0., f64::INFINITY, 1., 1., Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new(0., 0., f64::NEG_INFINITY, 1., 1., Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new(0., 0., -1., 1., 1., Angle::new::<radian>(0.), true).is_err());

        assert!(General2DGaussian::new(0., 0., 1., 0., 1., Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new(0., 0., 1., f64::NAN, 1., Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new(0., 0., 1., f64::INFINITY, 1., Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new(0., 0., 1., f64::NEG_INFINITY, 1., Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new(0., 0., 1., -1., 1., Angle::new::<radian>(0.), true).is_err());
    }
    #[test]
    fn new_gaussian_power(){
        assert!(General2DGaussian::new(0., 0., 1., 1., 0., Angle::new::<radian>(0.), true).is_ok());
        assert!(General2DGaussian::new(0., 0., 1., 1., -1., Angle::new::<radian>(0.), true).is_ok());
        assert!(General2DGaussian::new(0., 0., 1., 1., 0.5, Angle::new::<radian>(0.), true).is_ok());
        assert!(General2DGaussian::new(0., 0., 1., 1., f64::NAN, Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new(0., 0., 1., 1., f64::INFINITY, Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new(0., 0., 1., 1., f64::NEG_INFINITY, Angle::new::<radian>(0.), true).is_err());
    }

    #[test]
    fn new_gaussian_mean(){
        assert!(General2DGaussian::new(f64::NAN,          0., 1., 1., 1., Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new(f64::INFINITY,     0., 1., 1., 1., Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new(f64::NEG_INFINITY , 0., 1., 1., 1., Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new(-10.              , 0., 1., 1., 1., Angle::new::<radian>(0.), true).is_ok());
        assert!(General2DGaussian::new( 0.,f64::NAN,          1., 1., 1., Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new( 0.,f64::INFINITY,     1., 1., 1., Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new( 0.,f64::NEG_INFINITY , 1., 1., 1., Angle::new::<radian>(0.), true).is_err());
        assert!(General2DGaussian::new( 0.,-10.              , 1., 1., 1., Angle::new::<radian>(0.), true).is_ok());
    }

    #[test]
    fn new_gaussian_anlge(){
        assert!(General2DGaussian::new(0., 0., 1., 1., 1.,  Angle::new::<radian>(f64::NAN,        ), true).is_err());
        assert!(General2DGaussian::new(0., 0., 1., 1., 1.,  Angle::new::<radian>(f64::INFINITY,   ), true).is_err());
        assert!(General2DGaussian::new(0. , 0., 1., 1., 1., Angle::new::<radian>(f64::NEG_INFINITY), true).is_err());
        assert!(General2DGaussian::new(0. , 0., 1., 1., 1., Angle::new::<radian>(-10.             ), true).is_ok());
    }
}


