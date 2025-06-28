//! Uniform energy distribution

use crate::joule;
use nalgebra::Point2;
use num::ToPrimitive;
use serde::{Deserialize, Serialize};
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
};

use super::EnergyDistribution;
use crate::error::{OpmResult, OpossumError};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]
pub struct UniformDist {
    total_energy: Energy,
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
    ///
    /// # Example
    /// ```
    /// let dist = UniformDist::new(Energy::from_joules(1.0))?;
    /// ```
    pub fn new(total_energy: Energy) -> OpmResult<Self> {
        if !total_energy.get::<joule>().is_normal()
            || total_energy.get::<joule>().is_sign_negative()
        {
            return Err(OpossumError::Other(
                "Energy must be greater than zero finite!".into(),
            ));
        }
        Ok(Self { total_energy })
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
    ///
    /// # Example
    /// ```
    /// let mut dist = UniformDist::new(Energy::from_joules(1.0))?;
    /// dist.set_energy(Energy::from_joules(2.0))?;
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

    /// Returns the total energy stored in this distribution.
    ///
    /// # Returns
    /// The current total [`Energy`] value of the distribution.
    ///
    /// # Example
    /// ```
    /// let energy = dist.energy();
    /// ```
    #[must_use]
    pub fn energy(&self) -> Energy {
        self.total_energy
    }
}

impl Default for UniformDist {
    fn default() -> Self {
        Self {
            total_energy: joule!(0.1),
        }
    }
}

impl EnergyDistribution for UniformDist {
    fn apply(&self, input: &[Point2<Length>]) -> Vec<Energy> {
        let input_len = input.len();
        let energy_per_point = self.total_energy / input_len.to_f64().unwrap();
        vec![energy_per_point; input_len]
    }

    fn get_total_energy(&self) -> Energy {
        self.total_energy
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
    use crate::joule;
    #[test]
    fn new_uniform_energy() {
        assert!(UniformDist::new(joule!(0.)).is_err());
        assert!(UniformDist::new(joule!(f64::NAN)).is_err());
        assert!(UniformDist::new(joule!(f64::INFINITY)).is_err());
        assert!(UniformDist::new(joule!(f64::NEG_INFINITY)).is_err());
        assert!(UniformDist::new(joule!(-1.)).is_err());
        assert!(UniformDist::new(joule!(1.)).is_ok());
    }
}
