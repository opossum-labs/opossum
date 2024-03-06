//! Uniform energy distribution

use nalgebra::Point2;
use num::ToPrimitive;
use uom::si::{energy::joule, f64::Energy};

use crate::error::{OpmResult, OpossumError};

use super::EnergyDistribution;
pub struct UniformDist {
    total_energy: Energy,
}

impl UniformDist {
    /// Create a new uniform energy-distribution generator [`General2DGaussian`].
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
        };

        Ok(Self { total_energy })
    }
}
impl EnergyDistribution for UniformDist {
    fn apply(&self, input: &[Point2<f64>]) -> Vec<Energy> {
        let input_len = input.len();
        let energy_per_point = self.total_energy / input_len.to_f64().unwrap();
        vec![energy_per_point; input_len]
    }

    fn get_total_energy(&self) -> Energy {
        self.total_energy
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn new_uniform_energy() {
        assert!(UniformDist::new(Energy::new::<joule>(0.)).is_err());
        assert!(UniformDist::new(Energy::new::<joule>(f64::NAN)).is_err());
        assert!(UniformDist::new(Energy::new::<joule>(f64::INFINITY)).is_err());
        assert!(UniformDist::new(Energy::new::<joule>(f64::NEG_INFINITY)).is_err());
        assert!(UniformDist::new(Energy::new::<joule>(-1.)).is_err());
        assert!(UniformDist::new(Energy::new::<joule>(1.)).is_ok());
    }
}
