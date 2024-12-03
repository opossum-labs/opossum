use uom::si::f64::Length;

use super::SpectralDistribution;
use crate::error::{OpmResult, OpossumError};
use crate::meter;
use crate::utils::griddata::linspace;
use crate::utils::math_distribution_functions::gaussian;
use itertools::Itertools;
use kahan::KahanSummator;

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

    /// # Errors

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
        };
        if !wvl_range.1.is_normal() || wvl_range.1.is_sign_negative() {
            return Err(OpossumError::Other(
                "range end must be positive and finite".into(),
            ));
        }
        if !mu.is_normal() || mu.is_sign_negative() {
            return Err(OpossumError::Other(
                "mean value must be positive and finite!".into(),
            ));
        };
        if !fwhm.is_normal() || fwhm.is_sign_negative() {
            return Err(OpossumError::Other(
                "fwhm must be greater than zero and finite!".into(),
            ));
        };
        if !power.is_normal() || power.is_sign_negative() {
            return Err(OpossumError::Other(
                "power of the distribution must be positive and finite!".into(),
            ));
        };

        Ok(Self {
            wvl_range,
            num_points,
            mu,
            fwhm,
            power,
        })
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
#[cfg(test)]
mod test {
    use crate::{
        nanometer,
        spectral_distribution::{Gaussian, SpectralDistribution},
    };
    use core::f64;
    use approx::assert_abs_diff_eq;
    use uom::si::f64::Length;
    #[test]
    fn new() {
        assert!(Gaussian::new(
            (nanometer!(1000.0), nanometer!(2000.0)),
            10,
            nanometer!(1500.0),
            nanometer!(100.0),
            1.0
        )
        .is_ok());
        let test_values = vec![0.0, -0.1, f64::INFINITY, f64::NAN, f64::NEG_INFINITY];
        for value in &test_values {
            assert!(Gaussian::new(
                (nanometer!(1000.0), nanometer!(2000.0)),
                10,
                nanometer!(1500.0),
                nanometer!(100.0),
                *value
            )
            .is_err());
        }
        let wvl_values: Vec<Length> = test_values.iter().map(|v| nanometer!(*v)).collect();
        for value in &wvl_values {
            assert!(Gaussian::new(
                (*value, nanometer!(2000.0)),
                10,
                nanometer!(1500.0),
                nanometer!(100.0),
                1.0
            )
            .is_err());
            assert!(Gaussian::new(
                (nanometer!(2000.0), *value),
                10,
                nanometer!(1500.0),
                nanometer!(100.0),
                1.0
            )
            .is_err());
            assert!(Gaussian::new(
                (nanometer!(1000.0), nanometer!(2000.0)),
                10,
                *value,
                nanometer!(100.0),
                1.0
            )
            .is_err());
            assert!(Gaussian::new(
                (nanometer!(1000.0), nanometer!(2000.0)),
                10,
                nanometer!(1500.0),
                *value,
                1.0
            )
            .is_err());
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
        let v_sum:f64=values.iter().map(|v| v.1).sum();
        assert_abs_diff_eq!(v_sum, 1.0);
    }
}
