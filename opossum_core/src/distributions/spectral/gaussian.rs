use serde::{Deserialize, Serialize};
use uom::si::f64::Length;

use super::SpectralDistribution;
use crate::error::OpmResult;
use crate::utils::griddata::linspace;
use crate::utils::math_distribution_functions::gaussian;
use crate::validated;
use crate::{
    generic_validators::{AllNormal, AllNotZero, AllPositive, SecondLarger},
    meter, nanometer, validated_type,
};
use kahan::KahanSummator;
use opm_macros_lib::EnsureValidated;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnsureValidated)]
pub struct Gaussian {
    wvl_range: validated_type!((Length, Length), SecondLarger && AllPositive && AllNormal),
    num_points: validated_type!(usize, AllNotZero),
    mu: validated_type!(Length, AllPositive && AllNormal),
    fwhm: validated_type!(Length, AllPositive && AllNormal),
    power: validated_type!(f64, AllPositive && AllNormal),
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
        let mut spec_gaussian = Self::default();
        spec_gaussian.set_fwhm(fwhm)?;
        spec_gaussian.set_mu(mu)?;
        spec_gaussian.set_num_points(num_points)?;
        spec_gaussian.set_power(power)?;
        spec_gaussian.set_wvl_start(wvl_range.0)?;
        spec_gaussian.set_wvl_end(wvl_range.1)?;
        Ok(spec_gaussian)
    }

    /// Returns the start wavelength of the distribution range.
    ///
    /// This corresponds to the lower bound of the wavelength interval.
    ///
    /// # Returns
    /// A [`Length`] value representing the start of the wavelength range.
    #[must_use]
    pub fn wvl_start(&self) -> Length {
        self.wvl_range.get().0
    }

    /// Sets the start wavelength of the distribution range.
    ///
    /// # Parameters
    /// - `start`: A [`Length`] representing the new lower bound of the wavelength range.
    ///
    /// # Errors
    /// Returns an error on validation fail
    pub fn set_wvl_start(&mut self, start: Length) -> OpmResult<()> {
        self.wvl_range.set((start, self.wvl_end()))?;
        Ok(())
    }

    /// Returns the end wavelength of the distribution range.
    ///
    /// This corresponds to the upper bound of the wavelength interval.
    ///
    /// # Returns
    /// A [`Length`] value representing the end of the wavelength range.
    #[must_use]
    pub fn wvl_end(&self) -> Length {
        self.wvl_range.get().1
    }

    /// Sets the end wavelength of the distribution range.
    ///
    /// # Parameters
    /// - `end`: A [`Length`] representing the new upper bound of the wavelength range.
    ///
    /// # Errors
    /// Returns an error on validation fail
    pub fn set_wvl_end(&mut self, end: Length) -> OpmResult<()> {
        self.wvl_range.set((self.wvl_start(), end))?;
        Ok(())
    }

    /// Returns the number of discrete wavelength points used in the distribution.
    ///
    /// # Returns
    /// A `usize` indicating how many spectral samples are generated.
    #[must_use]
    pub const fn num_points(&self) -> usize {
        *self.num_points.get()
    }

    /// Sets the number of discrete wavelength points in the distribution.
    ///
    /// # Parameters
    /// - `num_points`: The number of spectral samples to generate.
    ///
    /// # Errors
    /// Returns an error on validation fail
    pub fn set_num_points(&mut self, num_points: usize) -> OpmResult<()> {
        self.num_points.set(num_points)?;
        Ok(())
    }

    /// Returns the full width at half maximum (FWHM) of the Gaussian distribution.
    ///
    /// This controls the width of the spectral peak.
    ///
    /// # Returns
    /// A [`Length`] value representing the FWHM.
    #[must_use]
    pub const fn fwhm(&self) -> Length {
        *self.fwhm.get()
    }

    /// Sets the full width at half maximum (FWHM) of the Gaussian distribution.
    ///
    /// # Parameters
    /// - `fwhm`: A [`Length`] specifying the width of the spectral peak.
    ///
    /// # Errors
    /// Returns an error on validation fail
    pub fn set_fwhm(&mut self, fwhm: Length) -> OpmResult<()> {
        self.fwhm.set(fwhm)?;
        Ok(())
    }

    /// Returns the mean (center wavelength) of the Gaussian distribution.
    ///
    /// # Returns
    /// A [`Length`] value representing the center wavelength (`μ`).
    #[must_use]
    pub const fn mu(&self) -> Length {
        *self.mu.get()
    }

    /// Sets the mean (center wavelength) of the Gaussian distribution.
    ///
    /// # Parameters
    /// - `mu`: A [`Length`] representing the new center wavelength.
    ///
    /// # Errors
    /// Returns an error on validation fail
    pub fn set_mu(&mut self, mu: Length) -> OpmResult<()> {
        self.mu.set(mu)?;
        Ok(())
    }

    /// Returns the total power of the spectral distribution.
    ///
    /// # Returns
    /// A `f64` value representing the power (intensity scaling factor).
    #[must_use]
    pub const fn power(&self) -> f64 {
        *self.power.get()
    }

    /// Sets the total power of the spectral distribution.
    ///
    /// # Parameters
    /// - `power`: A `f64` value representing the new power level.
    ///
    /// # Errors
    /// Returns an error on validation fail
    pub fn set_power(&mut self, power: f64) -> OpmResult<()> {
        self.power.set(power)?;
        Ok(())
    }
}

impl Default for Gaussian {
    fn default() -> Self {
        Self {
            wvl_range: validated!(
                (nanometer!(1000.), nanometer!(1100.)),
                SecondLarger && AllPositive && AllNormal
            )
            .unwrap(),
            num_points: validated!(50_usize, AllNotZero).unwrap(),
            mu: validated!(nanometer!(1054.), AllPositive && AllNormal).unwrap(),
            fwhm: validated!(nanometer!(10.), AllPositive && AllNormal).unwrap(),
            power: validated!(1., AllPositive && AllNormal).unwrap(),
        }
    }
}

impl SpectralDistribution for Gaussian {
    fn generate(&self) -> OpmResult<Vec<(Length, f64)>> {
        let wvls = linspace(
            self.wvl_start().value,
            self.wvl_end().value,
            self.num_points(),
        )?;
        let spectral_distribution = gaussian(
            wvls.data.as_slice(),
            self.mu().value,
            self.fwhm().value,
            self.power(),
        );
        let sum: f64 = spectral_distribution.iter().kahan_sum().sum();
        Ok(spectral_distribution
            .iter()
            .zip(wvls.iter())
            .map(|v| (meter!(*v.1), *v.0 / sum))
            .collect::<Vec<(Length, f64)>>())
    }
}

impl From<Gaussian> for super::SpecDistType {
    fn from(g: Gaussian) -> Self {
        Self::Gaussian(g)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{distributions::spectral::SpecDistType, nanometer};
    use approx::assert_abs_diff_eq;
    use core::f64;
    use uom::si::{f64::Length, length::nanometer};

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
        // invalid wavelength range
        assert!(
            Gaussian::new(
                (nanometer!(500.0), nanometer!(500.0)),
                500,
                nanometer!(750.0),
                nanometer!(10.0),
                1.0
            )
            .is_err()
        );
        assert!(
            Gaussian::new(
                (nanometer!(1000.0), nanometer!(999.0)),
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
                0,
                nanometer!(1500.0),
                nanometer!(100.0),
                1.0
            )
            .is_err()
        );
    }
    #[test]
    fn generate() -> OpmResult<()> {
        let gauss = Gaussian::new(
            (nanometer!(1000.0), nanometer!(2000.0)),
            11,
            nanometer!(1500.0),
            nanometer!(500.0),
            1.0,
        )?;
        let values = gauss.generate()?;
        assert_eq!(values.len(), 11);
        assert_abs_diff_eq!(values[5].0.value, nanometer!(1500.0).value);
        let v_sum: f64 = values.iter().map(|v| v.1).sum();
        assert_abs_diff_eq!(v_sum, 1.0);
        Ok(())
    }
    #[test]
    fn test_default() {
        let g = Gaussian::default();
        assert_abs_diff_eq!(g.wvl_start().get::<nanometer>(), 1000.0);
        assert_abs_diff_eq!(g.wvl_end().get::<nanometer>(), 1100.0);
        assert_eq!(g.num_points(), 50);
        assert_abs_diff_eq!(g.mu().get::<nanometer>(), 1054.0, epsilon = 1e-12);
        assert_abs_diff_eq!(g.fwhm().get::<nanometer>(), 10.0, epsilon = 1e-12);
        assert_abs_diff_eq!(g.power(), 1.0);
    }

    #[test]
    fn test_getters_setters() {
        let mut g = Gaussian::default();

        // Test Wavelength Range
        assert!(g.set_wvl_start(nanometer!(900.0)).is_ok());
        assert_abs_diff_eq!(g.wvl_start().get::<nanometer>(), 900.0);
        assert!(g.set_wvl_end(nanometer!(1200.0)).is_ok());
        assert_abs_diff_eq!(g.wvl_end().get::<nanometer>(), 1200.0);

        // Validation check: start >= end must fail
        assert!(g.set_wvl_start(nanometer!(1300.0)).is_err());

        // Test Num Points
        assert!(g.set_num_points(100).is_ok());
        assert_eq!(g.num_points(), 100);
        assert!(g.set_num_points(0).is_err());

        // Test Mu
        assert!(g.set_mu(nanometer!(1000.0)).is_ok());
        assert_abs_diff_eq!(g.mu().get::<nanometer>(), 1000.0);
        assert!(g.set_mu(nanometer!(-1.0)).is_err());

        // Test FWHM
        assert!(g.set_fwhm(nanometer!(20.0)).is_ok());
        assert_abs_diff_eq!(g.fwhm().get::<nanometer>(), 20.0, epsilon = 1e-12);
        assert!(g.set_fwhm(nanometer!(0.0)).is_err());

        // Test Power
        assert!(g.set_power(2.0).is_ok());
        assert_abs_diff_eq!(g.power(), 2.0);
        assert!(g.set_power(-0.5).is_err());
    }

    #[test]
    fn test_conversions() {
        let g = Gaussian::default();
        let spec_type: SpecDistType = g.clone().into();

        if let SpecDistType::Gaussian(converted) = spec_type {
            assert_abs_diff_eq!(converted.mu().value, g.mu().value);
        } else {
            panic!("Conversion to SpecDistType::Gaussian failed");
        }
    }

    #[test]
    fn test_generate_symmetry() -> OpmResult<()> {
        // A Gaussian centered in the range should be symmetric
        let mu = nanometer!(1500.0);
        let g = Gaussian::new(
            (nanometer!(1000.0), nanometer!(2000.0)),
            21, // odd number to have a center point
            mu,
            nanometer!(100.0),
            1.0,
        )?;
        let values = g.generate()?;

        // Check symmetry around index 10 (the 11th point at 1500nm)
        for i in 0..10 {
            assert_abs_diff_eq!(values[i].1, values[20 - i].1, epsilon = 1e-12);
        }
        Ok(())
    }
}
