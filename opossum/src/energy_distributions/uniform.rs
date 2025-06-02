//! Uniform energy distribution

use nalgebra::Point2;
use num::ToPrimitive;
use serde::{Deserialize, Serialize};
use uom::si::{
    energy::joule,
    f64::{Energy, Length},
};
use crate::joule;

use super::EnergyDistribution;
use crate::error::{OpmResult, OpossumError};
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UniformDist {
    total_energy: Energy,
}

impl UniformDist {
    /// Create a new uniform energy-distribution generator [`General2DGaussian`](crate::energy_distributions::General2DGaussian).
    /// # Attributes
    /// - `total_energy`: total energy to distribute within the construction points
    /// # Errors
    /// This function will return an error if
    ///   - the energy is non-finite, zero or below zero
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

impl Default for UniformDist{
    fn default() -> Self {
        Self {
            total_energy:joule!(0.1),
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
