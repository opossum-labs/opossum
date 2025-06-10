//! Generalized 2D Gaussian distribution
use super::EnergyDistribution;
use crate::{
    degree,
    error::{OpmResult, OpossumError},
    joule, millimeter,
    utils::math_distribution_functions::{
        general_2d_super_gaussian_point_elliptical, general_2d_super_gaussian_point_rectangular,
    },
};
use kahan::KahanSummator;
use nalgebra::Point2;
use serde::{Deserialize, Serialize};
use uom::si::{
    angle::radian,
    energy::joule,
    f64::{Angle, Energy, Length},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct General2DGaussian {
    total_energy: Energy,
    mu_xy: Point2<Length>,
    sigma_xy: Point2<Length>,
    power: f64,
    theta: Angle,
    rectangular: bool,
}
impl General2DGaussian {
    /// Create a new generalized 2-dimension Gaussian energy-distribution generator [`General2DGaussian`].
    /// # Attributes
    /// - `total_energy`: total energy to distribute within the construction points
    /// - `mu_x`: the mean value in x direction -> Shifts the distribution in x direction to be centered at `mu_x`
    /// - `mu_y`: the mean value in y direction -> Shifts the distribution in y direction to be centered at `mu_y`
    /// - `sigma_x`: the standard deviation value in x direction
    /// - `sigma_y`: the standard deviation value in y direction
    /// - `power`: the power of the distribution. A standard Gaussian distribution has a power of 1. Larger powers are so called super-Gaussians
    /// - `theta`: rotation angle of the distribution. Counter-clockwise rotation for positive theta
    /// - `rect_flag`: defines if the distribution will be shaped elliptically or rectangularly. Difference between these modes vanishes for power = 1
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
        power: f64,
        theta: Angle,
        rectangular: bool,
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
        if !power.is_finite() {
            return Err(OpossumError::Other(
                "Power of the distribution must be positive and finite!".into(),
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
            power,
            theta,
            rectangular,
        })
    }

    pub fn set_energy(&mut self, energy: Energy) -> OpmResult<()> {
        if !energy.get::<joule>().is_normal() || energy.get::<joule>().is_sign_negative() {
            return Err(OpossumError::Other(
                "Energy must be greater than zero finite!".into(),
            ));
        }
        self.total_energy = energy;
        Ok(())
    }
}

impl Default for General2DGaussian {
    fn default() -> Self {
        Self {
            total_energy: joule!(0.1),
            mu_xy: millimeter!(0., 0.),
            sigma_xy: millimeter!(5., 5.),
            power: 1.,
            theta: degree!(0.),
            rectangular: false,
        }
    }
}

impl EnergyDistribution for General2DGaussian {
    fn apply(&self, input: &[Point2<Length>]) -> Vec<Energy> {
        let mut energy_distribution = Vec::<f64>::with_capacity(input.len());
        let (sin_theta, cos_theta) = self.theta.get::<radian>().sin_cos();
        let mu_xy = Point2::new(self.mu_xy.x.value, self.mu_xy.y.value);
        let sigma_xy = Point2::new(self.sigma_xy.x.value, self.sigma_xy.y.value);
        if self.rectangular {
            for p in input {
                let p_m = Point2::new(p.x.value, p.y.value);
                energy_distribution.push(general_2d_super_gaussian_point_rectangular(
                    &p_m, mu_xy, sigma_xy, self.power, sin_theta, cos_theta,
                ));
            }
        } else {
            for p in input {
                let p_m = Point2::new(p.x.value, p.y.value);
                energy_distribution.push(general_2d_super_gaussian_point_elliptical(
                    &p_m, mu_xy, sigma_xy, self.power, sin_theta, cos_theta,
                ));
            }
        }

        let current_energy: f64 = energy_distribution.iter().kahan_sum().sum();

        energy_distribution
            .iter()
            .map(|x| self.total_energy * *x / current_energy)
            .collect::<Vec<Energy>>()
    }

    fn get_total_energy(&self) -> Energy {
        self.total_energy
    }
}
impl From<General2DGaussian> for super::EnergyDistType {
    fn from(g: General2DGaussian) -> Self {
        Self::General2DGaussian(g)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{joule, meter, radian};
    #[test]
    fn new_gaussian_sigma() {
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(0., 1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(f64::NAN, 1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(f64::INFINITY, 1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(f64::NEG_INFINITY, 1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(-1., 1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );

        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., 0.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., f64::NAN),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., f64::INFINITY),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., f64::NEG_INFINITY),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., -1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
    }
    #[test]
    fn new_gaussian_power() {
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., 1.),
                0.,
                radian!(0.),
                true
            )
            .is_ok()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., 1.),
                -1.,
                radian!(0.),
                true
            )
            .is_ok()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., 1.),
                0.5,
                radian!(0.),
                true
            )
            .is_ok()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., 1.),
                f64::NAN,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., 1.),
                f64::INFINITY,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., 1.),
                f64::NEG_INFINITY,
                radian!(0.),
                true
            )
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
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(f64::INFINITY),
                meter!(0., 0.),
                meter!(1., 1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(f64::NEG_INFINITY),
                meter!(0., 0.),
                meter!(1., 1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(-1.),
                meter!(0., 0.),
                meter!(1., 1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(0.),
                meter!(0., 0.),
                meter!(1., 1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., 1.),
                1.,
                radian!(0.),
                true
            )
            .is_ok()
        );
    }
    #[test]
    fn new_gaussian_mean() {
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(f64::NAN, 0.),
                meter!(1., 1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(f64::INFINITY, 0.),
                meter!(1., 1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(f64::NEG_INFINITY, 0.),
                meter!(1., 1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(-10., 0.),
                meter!(1., 1.),
                1.,
                radian!(0.),
                true
            )
            .is_ok()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., f64::NAN),
                meter!(1., 1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., f64::INFINITY),
                meter!(1., 1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., f64::NEG_INFINITY),
                meter!(1., 1.),
                1.,
                radian!(0.),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., -10.),
                meter!(1., 1.),
                1.,
                radian!(0.),
                true
            )
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
                1.,
                radian!(f64::NAN),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., 1.),
                1.,
                radian!(f64::INFINITY),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., 1.),
                1.,
                radian!(f64::NEG_INFINITY),
                true
            )
            .is_err()
        );
        assert!(
            General2DGaussian::new(
                joule!(1.),
                meter!(0., 0.),
                meter!(1., 1.),
                1.,
                radian!(-10.),
                true
            )
            .is_ok()
        );
    }
}
