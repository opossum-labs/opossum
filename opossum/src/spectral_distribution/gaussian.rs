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
    /// # Attributes
    /// - `mx`: the mean value  -> Shifts the distribution n to be centered at `mu`
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
        if !mu.is_finite() {
            return Err(OpossumError::Other("Mean value must be finite!".into()));
        };
        if !fwhm.is_normal() || fwhm.is_sign_negative() {
            return Err(OpossumError::Other(
                "fwhm must be greater than zero and finite!".into(),
            ));
        };
        if !power.is_finite() {
            return Err(OpossumError::Other(
                "Power of the distribution must be positive and finite!".into(),
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
    fn generate(&self) -> OpmResult<(Vec<f64>, Vec<Length>)> {
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

        Ok((
            spectral_distribution.iter().map(|x| *x / sum).collect_vec(),
            wvls.iter().map(|w| meter!(*w)).collect_vec(),
        ))
    }
}
