//! Generalized 2D Gaussian distribution
use super::EnergyDistribution;
use crate::{
    error::OpmResult,
    generic_validators::{AllFinite, AllNotZero, AllPositive},
    joule, millimeter,
    types::validated_type_definitions::{
        ValidatedAngle1D, ValidatedCenter2D, ValidatedGaussianPower, ValidatedSideLengths2D,
    },
    utils::super_gaussian::SuperGaussianShape,
    validated, validated_type,
};
use kahan::KahanSummator;
use nalgebra::Point2;
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::f64::{Angle, Energy, Length};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy, EnsureValidated)]
pub struct General2DGaussian {
    total_energy: validated_type!(Energy, AllNotZero && AllFinite && AllPositive),
    // The four parameters below are the shape rather than the energy, and they are typed by the
    // very same aliases [`SuperGaussianShape`] uses. That is what makes [`General2DGaussian::shape`]
    // infallible: there is one definition of what a valid center, width, power and rotation is, not
    // one per user of it. `Validated` is `#[serde(transparent)]`, so which validator guards a field
    // never reaches the file.
    mu_xy: ValidatedCenter2D,
    sigma_xy: ValidatedSideLengths2D,
    power: ValidatedGaussianPower,
    theta: ValidatedAngle1D,
    #[validate(skip)]
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
        let mut gaussian = Self::default();
        gaussian.set_energy(total_energy)?;
        gaussian.set_center_x(mu_xy.x)?;
        gaussian.set_center_y(mu_xy.y)?;
        gaussian.set_sigma_x(sigma_xy.x)?;
        gaussian.set_sigma_y(sigma_xy.y)?;
        gaussian.set_power(power)?;
        gaussian.set_theta(theta)?;
        gaussian.set_rectangular(rectangular);

        Ok(gaussian)
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
    /// Returns an error if validation fails
    pub fn set_energy(&mut self, energy: Energy) -> OpmResult<()> {
        self.total_energy.set(energy)?;
        Ok(())
    }

    /// Returns the total energy of the distribution.
    ///
    /// # Returns
    /// An [`Energy`] value representing the total integrated energy of the distribution.
    #[must_use]
    pub const fn energy(&self) -> Energy {
        *self.total_energy.get()
    }

    /// Returns the center of the 2D Gaussian in the x-y plane.
    ///
    /// # Returns
    /// A [`Point2<Length>`] representing the mean (μₓ, μᵧ) of the distribution.
    #[must_use]
    pub const fn center(&self) -> Point2<Length> {
        *self.mu_xy.get()
    }

    /// Returns the x-coordinate of the center of the 2D Gaussian.
    ///
    /// # Returns
    /// The X-coordinate as `Length` representing the mean (μₓ) of the distribution.
    #[must_use]
    pub fn center_x(&self) -> Length {
        self.mu_xy.get().x
    }

    /// Returns the y-coordinate of the center of the 2D Gaussian.
    ///
    /// # Returns
    /// The Y-coordinate as `Length` representing the mean (μᵧ) of the distribution.
    #[must_use]
    pub fn center_y(&self) -> Length {
        self.mu_xy.get().y
    }

    /// Sets the x-coordinate of the center of the 2D Gaussian distribution.
    ///
    /// # Parameters
    /// - `x`: A [`Length`] value for the new μₓ (horizontal center).
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_center_x(&mut self, x: Length) -> OpmResult<()> {
        self.mu_xy.set(Point2::new(x, self.center_y()))?;
        Ok(())
    }

    /// Sets the y-coordinate of the center of the 2D Gaussian distribution.
    ///
    /// # Parameters
    /// - `y`: A [`Length`] value for the new μᵧ (vertical center).
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_center_y(&mut self, y: Length) -> OpmResult<()> {
        self.mu_xy.set(Point2::new(self.center_x(), y))?;
        Ok(())
    }

    /// Returns the standard deviation along the x and y axes.
    ///
    /// # Returns
    /// A [`Point2<Length>`] containing the standard deviations (σₓ, σᵧ).
    #[must_use]
    pub const fn sigma(&self) -> Point2<Length> {
        *self.sigma_xy.get()
    }

    /// Returns the x standard deviation of the 2D Gaussian.
    ///
    /// # Returns
    /// A `Length` representing the standard deviation (σₓ) of the distribution.
    #[must_use]
    pub fn sigma_x(&self) -> Length {
        self.sigma_xy.get().x
    }

    /// Returns the y standard deviation of the 2D Gaussian.
    ///
    /// # Returns
    /// A `Length` representing the standard deviation (σᵧ) of the distribution.
    #[must_use]
    pub fn sigma_y(&self) -> Length {
        self.sigma_xy.get().y
    }

    /// Sets the standard deviation σₓ of the 2D Gaussian distribution.
    ///
    /// # Parameters
    /// - `x`: A [`Length`] value for the horizontal spread.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_sigma_x(&mut self, x: Length) -> OpmResult<()> {
        self.sigma_xy.set(Point2::new(x, self.sigma_y()))?;
        Ok(())
    }

    /// Sets the standard deviation σᵧ of the 2D Gaussian distribution.
    ///
    /// # Parameters
    /// - `y`: A [`Length`] value for the vertical spread.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_sigma_y(&mut self, y: Length) -> OpmResult<()> {
        self.sigma_xy.set(Point2::new(self.sigma_x(), y))?;
        Ok(())
    }

    /// Returns the normalized power scaling factor of the distribution.
    ///
    /// This can be used to modulate the intensity without affecting the shape.
    ///
    /// # Returns
    /// A `f64` value representing the power multiplier.
    #[must_use]
    pub const fn power(&self) -> f64 {
        *self.power.get()
    }

    /// Sets the normalized power scaling factor of the distribution.
    ///
    /// # Parameters
    /// - `power`: A `f64` value for the new intensity multiplier.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_power(&mut self, power: f64) -> OpmResult<()> {
        self.power.set(power)?;
        Ok(())
    }

    /// Returns the rotation angle θ of the distribution in the x-y plane.
    ///
    /// The rotation is measured counterclockwise from the x-axis.
    ///
    /// # Returns
    /// An [`Angle`] representing the orientation of the Gaussian ellipse.
    #[must_use]
    pub const fn theta(&self) -> Angle {
        *self.theta.get()
    }

    /// Sets the rotation angle θ of the distribution in the x-y plane.
    ///
    /// # Parameters
    /// - `angle`: An [`Angle`] specifying the orientation.
    ///
    /// # Errors
    /// Returns an error if validation fails
    pub fn set_theta(&mut self, angle: Angle) -> OpmResult<()> {
        self.theta.set(angle)?;
        Ok(())
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
    /// Returns the shape of this distribution, without the energy it spreads over it.
    ///
    /// Everything but [`General2DGaussian::energy`] describes a shape rather than a distribution,
    /// and that description is not particular to spreading energy — a pumped medium is given a
    /// transversal profile by the very same one. It therefore lives in [`SuperGaussianShape`], and
    /// this is where the two meet.
    ///
    /// # Returns
    ///
    /// The peak-normalised shape this distribution has, which
    /// [`EnergyDistribution::apply`] then scales to the total energy.
    ///
    /// # Panics
    ///
    /// Panics if the parameters of this distribution are rejected by [`SuperGaussianShape`], which
    /// cannot happen: both hold them in the very same validated types, so a value that got into one
    /// is accepted by the other by construction.
    #[must_use]
    pub fn shape(&self) -> SuperGaussianShape {
        SuperGaussianShape::new(
            self.center(),
            self.sigma(),
            self.power(),
            self.theta(),
            self.rectangular(),
        )
        .expect("a validated General2DGaussian is a validated SuperGaussianShape")
    }
}
impl Default for General2DGaussian {
    fn default() -> Self {
        Self {
            total_energy: validated!(joule!(0.1), AllNotZero && AllFinite && AllPositive).unwrap(),
            mu_xy: ValidatedCenter2D::default(),
            sigma_xy: ValidatedSideLengths2D::try_new(millimeter!(5., 5.)).unwrap(),
            power: ValidatedGaussianPower::default(),
            theta: ValidatedAngle1D::default(),
            rectangular: false,
        }
    }
}

impl EnergyDistribution for General2DGaussian {
    fn apply(&self, input: &[Point2<Length>]) -> Vec<Energy> {
        // The shape is peak-normalised, so what comes out here is the *relative* weight of each
        // point. Turning those weights into energies is this distribution's own job, and the only
        // part of it that is about energy at all.
        let shape = self.shape();
        let energy_distribution = input
            .iter()
            .map(|point| shape.value_at(point))
            .collect::<Vec<f64>>();

        let current_energy: f64 = energy_distribution.iter().kahan_sum().sum();

        energy_distribution
            .iter()
            .map(|x| self.energy() * *x / current_energy)
            .collect::<Vec<Energy>>()
    }

    fn get_total_energy(&self) -> Energy {
        self.energy()
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
    use crate::{degree, joule, meter, radian};
    use uom::si::energy::joule;
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
            .is_err()
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
            .is_err()
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
    /// The shape parameters sit directly in this struct, not nested in a shape of their own.
    ///
    /// They are *typed* by the same validated aliases [`SuperGaussianShape`] uses, and those are
    /// `#[serde(transparent)]`, so which validator guards a field never reaches the file. Pinned
    /// here because `.opm` files in the wild carry this type through a source node's light data,
    /// and the golden roundtrip fixture happens not to contain one.
    #[test]
    fn serialization_stays_flat() -> OpmResult<()> {
        let gaussian = General2DGaussian::default();
        let serialized = ron::to_string(&gaussian)
            .map_err(|e| crate::error::OpossumError::Other(e.to_string()))?;
        assert_eq!(
            serialized,
            "(total_energy:0.1,mu_xy:(0.0,0.0),sigma_xy:(0.005,0.005),power:1.0,theta:0.0,\
             rectangular:false)"
        );
        let deserialized: General2DGaussian = ron::from_str(&serialized)
            .map_err(|e| crate::error::OpossumError::Other(e.to_string()))?;
        assert_eq!(gaussian, deserialized);
        Ok(())
    }
    #[test]
    fn power_parameter_influence() -> OpmResult<()> {
        let center = Point2::new(millimeter!(0.0), millimeter!(0.0));
        let sigma = Point2::new(millimeter!(1.0), millimeter!(1.0));

        // Wir brauchen mindestens zwei Punkte, damit die Normalisierung
        // die relative Verteilung beibehält.
        let points = vec![
            Point2::new(millimeter!(0.0), millimeter!(0.0)),
            Point2::new(millimeter!(0.5), millimeter!(0.5)),
        ];

        let dist_1 = General2DGaussian::new(joule!(1.0), center, sigma, 1.0, degree!(0.0), false)?;
        let dist_2 = General2DGaussian::new(joule!(1.0), center, sigma, 2.0, degree!(0.0), false)?;

        let e1 = dist_1.apply(&points);
        let e2 = dist_2.apply(&points);

        // Vergleiche die Energie am zweiten Punkt (0.5, 0.5)
        // Bei power=2 (Super-Gauß) fällt die Flanke steiler ab oder ist flacher
        // (je nach Definition), was die Anteile verschiebt.
        assert!(
            (e1[1].get::<joule>() - e2[1].get::<joule>()).abs() > 1e-5,
            "Power parameter should influence relative distribution between points"
        );
        Ok(())
    }

    #[test]
    fn rectangular_flag_influence() -> OpmResult<()> {
        let center = Point2::new(millimeter!(0.0), millimeter!(0.0));
        let sigma = Point2::new(millimeter!(1.0), millimeter!(0.5));

        // Auch hier: Zwei Punkte verwenden
        let points = vec![
            Point2::new(millimeter!(0.0), millimeter!(0.0)),
            Point2::new(millimeter!(0.8), millimeter!(0.4)),
        ];

        let dist_ellip =
            General2DGaussian::new(joule!(1.0), center, sigma, 2.0, degree!(0.0), false)?;
        let dist_rect =
            General2DGaussian::new(joule!(1.0), center, sigma, 2.0, degree!(0.0), true)?;

        let e_ellip = dist_ellip.apply(&points);
        let e_rect = dist_rect.apply(&points);

        assert!(
            (e_ellip[1].get::<joule>() - e_rect[1].get::<joule>()).abs() > 1e-5,
            "Rectangular flag should influence relative distribution for power != 1"
        );
        Ok(())
    }
}
