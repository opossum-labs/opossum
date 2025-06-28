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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]
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

    /// Sets the total energy of this [`General2DGaussian`] distribution.
    ///
    /// This function updates the total energy assigned to the distribution,
    /// which determines how much energy is spread across the 2D Gaussian profile.
    ///
    /// The [`General2DGaussian`] distribution represents a two-dimensional Gaussian
    /// distribution with parameters like center, width (σ), rotation, and aspect ratio.
    ///
    /// # Parameters
    /// - `energy`: The new total [`Energy`] to assign to the distribution.
    ///
    /// # Returns
    /// - `Ok(())` if the provided energy is valid (positive and finite).
    /// - `Err(OpossumError)` if the energy is invalid.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The energy is not finite (e.g., NaN or infinite).
    /// - The energy is zero or negative.
    ///
    /// # Example
    /// ```
    /// let mut dist = General2DGaussian::default();
    /// dist.set_energy(Energy::from_joules(1.0))?;
    /// ```
    pub fn set_energy(&mut self, energy: Energy) -> OpmResult<()> {
        if !energy.get::<joule>().is_normal() || energy.get::<joule>().is_sign_negative() {
            return Err(OpossumError::Other(
                "Energy must be greater than zero finite!".into(),
            ));
        }
        self.total_energy = energy;
        Ok(())
    }

    /// Returns the total energy of the distribution.
    ///
    /// # Returns
    /// An [`Energy`] value representing the total integrated energy of the distribution.
    #[must_use]
    pub const fn energy(&self) -> Energy {
        self.total_energy
    }

    /// Returns the center of the 2D Gaussian in the x-y plane.
    ///
    /// # Returns
    /// A [`Point2<Length>`] representing the mean (μₓ, μᵧ) of the distribution.
    #[must_use]
    pub const fn center(&self) -> Point2<Length> {
        self.mu_xy
    }

    /// Sets the x-coordinate of the center of the 2D Gaussian distribution.
    ///
    /// # Parameters
    /// - `x`: A [`Length`] value for the new μₓ (horizontal center).
    pub fn set_center_x(&mut self, x: Length) {
        self.mu_xy.x = x;
    }

    /// Sets the y-coordinate of the center of the 2D Gaussian distribution.
    ///
    /// # Parameters
    /// - `y`: A [`Length`] value for the new μᵧ (vertical center).
    pub fn set_center_y(&mut self, y: Length) {
        self.mu_xy.y = y;
    }

    /// Returns the standard deviation along the x and y axes.
    ///
    /// # Returns
    /// A [`Point2<Length>`] containing the standard deviations (σₓ, σᵧ).
    #[must_use]
    pub const fn sigma(&self) -> Point2<Length> {
        self.sigma_xy
    }

    /// Sets the standard deviation σₓ of the 2D Gaussian distribution.
    ///
    /// # Parameters
    /// - `x`: A [`Length`] value for the horizontal spread.
    pub fn set_sigma_x(&mut self, x: Length) {
        self.sigma_xy.x = x;
    }

    /// Sets the standard deviation σᵧ of the 2D Gaussian distribution.
    ///
    /// # Parameters
    /// - `y`: A [`Length`] value for the vertical spread.
    pub fn set_sigma_y(&mut self, y: Length) {
        self.sigma_xy.y = y;
    }

    /// Returns the normalized power scaling factor of the distribution.
    ///
    /// This can be used to modulate the intensity without affecting the shape.
    ///
    /// # Returns
    /// A `f64` value representing the power multiplier.
    #[must_use]
    pub const fn power(&self) -> f64 {
        self.power
    }

    /// Sets the normalized power scaling factor of the distribution.
    ///
    /// # Parameters
    /// - `power`: A `f64` value for the new intensity multiplier.
    pub const fn set_power(&mut self, power: f64) {
        self.power = power;
    }

    /// Returns the rotation angle θ of the distribution in the x-y plane.
    ///
    /// The rotation is measured counterclockwise from the x-axis.
    ///
    /// # Returns
    /// An [`Angle`] representing the orientation of the Gaussian ellipse.
    #[must_use]
    pub const fn theta(&self) -> Angle {
        self.theta
    }

    /// Sets the rotation angle θ of the distribution in the x-y plane.
    ///
    /// # Parameters
    /// - `angle`: An [`Angle`] specifying the orientation.
    pub fn set_theta(&mut self, angle: Angle) {
        self.theta = angle;
    }

    /// Returns whether the distribution has rectangular or elliptical shape.
    ///
    /// This affects how the Gaussian profile is shaped.
    ///
    /// # Returns
    /// A `bool` indicating if rectangular mode is active.
    #[must_use]
    pub const fn rectangular(&self) -> bool {
        self.rectangular
    }

    /// Enables or disables rectangular shaping for the distribution.
    ///
    /// # Parameters
    /// - `rectangular`: A `bool` indicating whether to use rectangular mode.
    pub const fn set_rectangular(&mut self, rectangular: bool) {
        self.rectangular = rectangular;
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
