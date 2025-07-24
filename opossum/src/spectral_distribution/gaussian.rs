use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use super::SpectralDistribution;
use crate::error::{OpmResult, OpossumError};
use crate::utils::griddata::linspace;
use crate::utils::math_distribution_functions::gaussian;
use crate::{meter, nanometer};
use itertools::Itertools;
use kahan::KahanSummator;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Copy)]
pub struct Gaussian {
    wvl_range: (Length, Length),
    num_points: usize,
    mu: Length,
    fwhm: Length,
    power: f64,
}

impl Gaussian {
    /// Create a new Gaussian distribution generator
    ///
    /// # Attributes
    ///
    /// - `mu`: the mean value  -> Shifts the distribution n to be centered at `mu`
    /// - `fwhm`: the full-with at half maximum of the gaussian
    /// - `power`: the power of the distribution. A standard Gaussian distribution has a power of 1. Larger powers are so called super-Gaussians
    ///
    /// # Errors
    ///
    /// This function will return an error if
    ///   - the mean value are non-finite
    ///   - the fwhm are non-finite, zero or below zero
    ///   - the power are non-finite, zero or below zero
    pub fn new(
        wvl_range: (Length, Length),
        num_points: usize,
        mu: Length,
        fwhm: Length,
        power: f64,
    ) -> OpmResult<Self> {
        if !wvl_range.0.is_normal() || wvl_range.0.is_sign_negative() {
            return Err(OpossumError::Other(
                "range start must be positive and finite".into(),
            ));
        }
        if !wvl_range.1.is_normal() || wvl_range.1.is_sign_negative() {
            return Err(OpossumError::Other(
                "range end must be positive and finite".into(),
            ));
        }
        if !mu.is_normal() || mu.is_sign_negative() {
            return Err(OpossumError::Other(
                "mean value must be positive and finite!".into(),
            ));
        }
        if !fwhm.is_normal() || fwhm.is_sign_negative() {
            return Err(OpossumError::Other(
                "fwhm must be greater than zero and finite!".into(),
            ));
        }
        if !power.is_normal() || power.is_sign_negative() {
            return Err(OpossumError::Other(
                "power of the distribution must be positive and finite!".into(),
            ));
        }
        Ok(Self {
            wvl_range,
            num_points,
            mu,
            fwhm,
            power,
        })
    }

    /// Returns the start wavelength of the distribution range.
    ///
    /// This corresponds to the lower bound of the wavelength interval.
    ///
    /// # Returns
    /// A [`Length`] value representing the start of the wavelength range.
    #[must_use]
    pub const fn wvl_start(&self) -> Length {
        self.wvl_range.0
    }

    /// Sets the start wavelength of the distribution range.
    ///
    /// # Parameters
    /// - `start`: A [`Length`] representing the new lower bound of the wavelength range.
    pub fn set_wvl_start(&mut self, start: Length) {
        self.wvl_range.0 = start;
    }

    /// Returns the end wavelength of the distribution range.
    ///
    /// This corresponds to the upper bound of the wavelength interval.
    ///
    /// # Returns
    /// A [`Length`] value representing the end of the wavelength range.
    #[must_use]
    pub const fn wvl_end(&self) -> Length {
        self.wvl_range.1
    }

    /// Sets the end wavelength of the distribution range.
    ///
    /// # Parameters
    /// - `end`: A [`Length`] representing the new upper bound of the wavelength range.
    pub fn set_wvl_end(&mut self, end: Length) {
        self.wvl_range.1 = end;
    }

    /// Returns the number of discrete wavelength points used in the distribution.
    ///
    /// # Returns
    /// A `usize` indicating how many spectral samples are generated.
    #[must_use]
    pub const fn num_points(&self) -> usize {
        self.num_points
    }

    /// Sets the number of discrete wavelength points in the distribution.
    ///
    /// # Parameters
    /// - `num_points`: The number of spectral samples to generate.
    pub const fn set_num_points(&mut self, num_points: usize) {
        self.num_points = num_points;
    }

    /// Returns the full width at half maximum (FWHM) of the Gaussian distribution.
    ///
    /// This controls the width of the spectral peak.
    ///
    /// # Returns
    /// A [`Length`] value representing the FWHM.
    #[must_use]
    pub const fn fwhm(&self) -> Length {
        self.fwhm
    }

    /// Sets the full width at half maximum (FWHM) of the Gaussian distribution.
    ///
    /// # Parameters
    /// - `fwhm`: A [`Length`] specifying the width of the spectral peak.
    pub fn set_fwhm(&mut self, fwhm: Length) {
        self.fwhm = fwhm;
    }

    /// Returns the mean (center wavelength) of the Gaussian distribution.
    ///
    /// # Returns
    /// A [`Length`] value representing the center wavelength (`μ`).
    #[must_use]
    pub const fn mu(&self) -> Length {
        self.mu
    }

    /// Sets the mean (center wavelength) of the Gaussian distribution.
    ///
    /// # Parameters
    /// - `mu`: A [`Length`] representing the new center wavelength.
    pub fn set_mu(&mut self, mu: Length) {
        self.mu = mu;
    }

    /// Returns the total power of the spectral distribution.
    ///
    /// # Returns
    /// A `f64` value representing the power (intensity scaling factor).
    #[must_use]
    pub const fn power(&self) -> f64 {
        self.power
    }

    /// Sets the total power of the spectral distribution.
    ///
    /// # Parameters
    /// - `power`: A `f64` value representing the new power level.
    pub const fn set_power(&mut self, power: f64) {
        self.power = power;
    }
}

impl Default for Gaussian {
    fn default() -> Self {
        Self {
            wvl_range: (nanometer!(1000.), nanometer!(1100.)),
            num_points: 50,
            mu: nanometer!(1054.),
            fwhm: nanometer!(10.),
            power: 1.,
        }
    }
}

impl SpectralDistribution for Gaussian {
    fn generate(&self) -> OpmResult<Vec<(Length, f64)>> {
        let wvls = linspace(
            self.wvl_range.0.value,
            self.wvl_range.1.value,
            self.num_points,
        )?;
        let spectral_distribution = gaussian(
            wvls.data.as_slice(),
            self.mu.value,
            self.fwhm.value,
            self.power,
        );
        let sum: f64 = spectral_distribution.iter().kahan_sum().sum();
        Ok(spectral_distribution
            .iter()
            .zip(wvls.iter())
            .map(|v| (meter!(*v.1), *v.0 / sum))
            .collect_vec())
    }
}

impl From<Gaussian> for super::SpecDistType {
    fn from(g: Gaussian) -> Self {
        Self::Gaussian(g)
    }
}
#[cfg(test)]
mod test {
    use crate::{
        nanometer,
        spectral_distribution::{Gaussian, SpectralDistribution},
    };
    use approx::assert_abs_diff_eq;
    use core::f64;
    use uom::si::f64::Length;
    #[test]
    fn new() {
        assert!(
            Gaussian::new(
                (nanometer!(1000.0), nanometer!(2000.0)),
                10,
                nanometer!(1500.0),
                nanometer!(100.0),
                1.0
            )
            .is_ok()
        );
        let test_values = vec![0.0, -0.1, f64::INFINITY, f64::NAN, f64::NEG_INFINITY];
        for value in &test_values {
            assert!(
                Gaussian::new(
                    (nanometer!(1000.0), nanometer!(2000.0)),
                    10,
                    nanometer!(1500.0),
                    nanometer!(100.0),
                    *value
                )
                .is_err()
            );
        }
        let wvl_values: Vec<Length> = test_values.iter().map(|v| nanometer!(*v)).collect();
        for value in &wvl_values {
            assert!(
                Gaussian::new(
                    (*value, nanometer!(2000.0)),
                    10,
                    nanometer!(1500.0),
                    nanometer!(100.0),
                    1.0
                )
                .is_err()
            );
            assert!(
                Gaussian::new(
                    (nanometer!(2000.0), *value),
                    10,
                    nanometer!(1500.0),
                    nanometer!(100.0),
                    1.0
                )
                .is_err()
            );
            assert!(
                Gaussian::new(
                    (nanometer!(1000.0), nanometer!(2000.0)),
                    10,
                    *value,
                    nanometer!(100.0),
                    1.0
                )
                .is_err()
            );
            assert!(
                Gaussian::new(
                    (nanometer!(1000.0), nanometer!(2000.0)),
                    10,
                    nanometer!(1500.0),
                    *value,
                    1.0
                )
                .is_err()
            );
        }
    }
    #[test]
    fn generate() {
        let gauss = Gaussian::new(
            (nanometer!(1000.0), nanometer!(2000.0)),
            11,
            nanometer!(1500.0),
            nanometer!(500.0),
            1.0,
        )
        .unwrap();
        let values = gauss.generate().unwrap();
        assert_eq!(values.len(), 11);
        assert_abs_diff_eq!(values[5].0.value, nanometer!(1500.0).value);
        let v_sum: f64 = values.iter().map(|v| v.1).sum();
        assert_abs_diff_eq!(v_sum, 1.0);
    }
}
