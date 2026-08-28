//! Uniform energy distribution

use crate::{
    generic_validators::{AllFinite, AllNotZero, AllPositive},
    joule, validated, validated_type,
};
use nalgebra::Point2;
use num::ToPrimitive;
use opm_macros_lib::EnsureValidated;
use serde::{Deserialize, Serialize};
use uom::si::f64::{Energy, Length};

use super::EnergyDistribution;
use crate::error::OpmResult;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy, EnsureValidated)]
pub struct UniformDist {
    total_energy: validated_type!(Energy, AllNotZero && AllFinite && AllPositive),
}

impl UniformDist {
    /// Creates a new uniform energy distribution.
    ///
    /// The uniform distribution assigns the same energy to all sampling points
    /// without any spatial weighting.
    ///
    /// # Parameters
    /// - `total_energy`: The total [`Energy`] to distribute across all rays or points.
    ///
    /// # Returns
    /// - `Ok(Self)` if the energy is valid (positive and finite).
    /// - `Err(OpossumError)` if the energy is non-finite, zero, or negative.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `total_energy` is not finite (NaN or infinite).
    /// - `total_energy` is zero or less than zero.
    pub fn new(total_energy: Energy) -> OpmResult<Self> {
        let mut uniform = Self::default();
        uniform.set_energy(total_energy)?;
        Ok(uniform)
    }

    /// Sets the total energy of this uniform distribution.
    ///
    /// This replaces the previously set energy value with a new one.
    ///
    /// # Parameters
    /// - `energy`: The new [`Energy`] value to set.
    ///
    /// # Returns
    /// - `Ok(())` on success.
    /// - `Err(OpossumError)` if the energy is invalid.
    ///
    /// # Errors
    /// Returns an error if:
    /// - `energy` is not finite (NaN or infinite).
    /// - `energy` is zero or negative.
    pub fn set_energy(&mut self, energy: Energy) -> OpmResult<()> {
        self.total_energy.set(energy)?;
        Ok(())
    }

    /// Returns the total energy stored in this distribution.
    ///
    /// # Returns
    /// The current total [`Energy`] value of the distribution.
    #[must_use]
    pub fn energy(&self) -> Energy {
        *self.total_energy.get()
    }
}

impl Default for UniformDist {
    fn default() -> Self {
        Self {
            total_energy: validated!(joule!(0.1), AllNotZero && AllFinite && AllPositive).unwrap(),
        }
    }
}

impl EnergyDistribution for UniformDist {
    fn apply(&self, input: &[Point2<Length>]) -> Vec<Energy> {
        let input_len = input.len();
        let energy_per_point = self.energy() / input_len.to_f64().unwrap();
        vec![energy_per_point; input_len]
    }

    fn get_total_energy(&self) -> Energy {
        self.energy()
    }
}
impl From<UniformDist> for super::EnergyDistType {
    fn from(ud: UniformDist) -> Self {
        Self::Uniform(ud)
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::{joule, millimeter};
    use approx::assert_abs_diff_eq;
    use nalgebra::Point2;
    use uom::si::energy::joule;
    #[test]
    fn new_uniform_energy() {
        assert!(UniformDist::new(joule!(0.)).is_err());
        assert!(UniformDist::new(joule!(f64::NAN)).is_err());
        assert!(UniformDist::new(joule!(f64::INFINITY)).is_err());
        assert!(UniformDist::new(joule!(f64::NEG_INFINITY)).is_err());
        assert!(UniformDist::new(joule!(-1.)).is_err());
        assert!(UniformDist::new(joule!(1.)).is_ok());
    }
    #[test]
    fn uniform_renormalization_integration() -> OpmResult<()> {
        let total = joule!(1.0);
        let dist = UniformDist::new(total)?;
        let points = vec![Point2::new(millimeter!(0.0), millimeter!(0.0)); 10];
        let mut energies = dist.apply(&points);
        energies[0] = joule!(0.0); // Simulate energy loss: set one element to zero
        dist.renormalize(&mut energies);
        let sum: f64 = energies.iter().map(|e| e.get::<joule>()).sum();
        assert_abs_diff_eq!(sum, 1.0, epsilon = 1e-12);
        Ok(())
    }
}
